pub mod collection;
mod core;
//use crate::collection::index_map::*;
use crate::collection::index_map::*;
use crate::core::all::*;
use std::ops::{Not, RangeInclusive};

use log::{debug, info, trace};

#[derive(Debug, Clone, Copy)]
pub enum Decision {
    True(BVar),
    False(BVar),
}
impl Not for Decision {
    type Output = Self;
    fn not(self) -> Self::Output {
        match self {
            Decision::True(v) => Decision::False(v),
            Decision::False(v) => Decision::True(v),
        }
    }
}

pub struct SearchParams {
    var_decay: f64,
    cla_decay: f64,
    init_nof_conflict: usize,
    init_learnt_ratio: f64,
    use_learning: bool,
}
impl Default for SearchParams {
    fn default() -> Self {
        SearchParams {
            var_decay: 0.95,
            cla_decay: 0.999,
            init_nof_conflict: 100,
            init_learnt_ratio: 1_f64 / 3_f64,
            use_learning: true,
        }
    }
}

pub struct Solver {
    num_vars: u32,
    assignments: Assignments,
    clauses: ClauseDB,
    watches: IndexMap<Lit, Vec<ClauseId>>,
    propagation_queue: Vec<Lit>,
    heuristic: Heur,
}

enum AddClauseRes {
    Inconsistent,
    Unit(Lit),
    Complete(ClauseId),
}

impl Solver {
    pub fn init(clauses: Vec<Box<[Lit]>>) -> Self {
        let mut biggest_var = 0;
        for cl in &clauses {
            for lit in &**cl {
                biggest_var = biggest_var.max(lit.variable().id.get())
            }
        }
        let db = ClauseDB::new();
        let watches = IndexMap::new_with(((biggest_var + 1) * 2) as usize, || Vec::new());

        println!("biggest var: {}", biggest_var);

        let mut solver = Solver {
            num_vars: biggest_var,
            assignments: Assignments::new(biggest_var),
            clauses: db,
            watches,
            propagation_queue: Vec::new(),
            heuristic: Heur::init(biggest_var, HeurParams::default()),
        };

        for cl in clauses {
            solver.add_clause(&*cl, false);
        }

        solver.check_invariants();
        solver
    }

    fn add_clause(&mut self, lits: &[Lit], learnt: bool) -> AddClauseRes {
        // TODO: normalize non learnt clauses

        if learnt {
            // invariant: at this point we should have undone the assignment to the first literal
            // and all others should still be violated
            debug_assert!(lits[1..]
                .iter()
                .all(|l| self.assignments.is_set(l.variable())));
        }

        match lits.len() {
            0 => AddClauseRes::Inconsistent,
            1 => {
                //                self.enqueue(lits[0], None);
                AddClauseRes::Unit(lits[0])
            }
            _ => {
                let mut cl = Clause::new(lits, learnt);
                if learnt {
                    // lits[0] is the first literal to watch
                    // select second literal to watch (the one with highest decision level)
                    // and move it to lits[1]
                    let lits = &mut cl.disjuncts;
                    let mut max_i = 1;
                    let mut max_lvl = self.assignments.level(lits[1].variable());
                    for i in 1..lits.len() {
                        let lvl_i = self.assignments.level(lits[i].variable());
                        if lvl_i > max_lvl {
                            max_i = i;
                            max_lvl = lvl_i;
                        }
                    }
                    lits.swap(1, max_i);

                    // adding a leanrt clause, we must bump the activity of all its variables
                    for l in lits {
                        self.heuristic.var_bump_activity(l.variable());
                    }
                }
                // the two literals to watch
                let lit0 = cl.disjuncts[0];
                let lit1 = cl.disjuncts[1];
                let cl_id = self.clauses.add_clause(cl);

                self.watches[!lit0].push(cl_id);
                self.watches[!lit1].push(cl_id);
                AddClauseRes::Complete(cl_id)
            }
        }
    }

    pub fn variables(&self) -> RangeInclusive<BVar> {
        RangeInclusive::new(BVar::from_bits(1), BVar::from_bits(self.num_vars))
    }

    pub fn decide(&mut self, dec: Decision) {
        self.check_invariants();
        trace!("decision: {:?}", dec);
        self.assignments.add_backtrack_point(dec);
        self.assume(dec, None);
    }
    pub fn assume(&mut self, dec: Decision, reason: Option<ClauseId>) {
        self.check_invariants();
        match dec {
            Decision::True(var) => {
                self.assignments.set(var, true, reason);
                self.propagation_queue.push(var.lit(true));
            }
            Decision::False(var) => {
                self.assignments.set(var, false, reason);
                self.propagation_queue.push(var.lit(false));
            }
        }
        self.check_invariants();
    }

    /// Returns:
    ///   Some(i): in case of a conflict where i is the id of the violated clause
    ///   None if no conflict was detected during propagation
    pub fn propagate(&mut self) -> Option<ClauseId> {
        self.check_invariants();
        while !self.propagation_queue.is_empty() {
            let p = self.propagation_queue.pop().unwrap();

            let todo = self.watches[p].clone();
            self.watches[p].clear();
            let n = todo.len();
            for i in 0..n {
                if !self.propagate_clause(todo[i], p) {
                    // clause violated
                    // restore remaining watches
                    for j in i + 1..n {
                        self.watches[p].push(todo[j]);
                    }
                    self.propagation_queue.clear();
                    self.check_invariants();
                    return Some(todo[i]);
                }
            }
        }
        self.check_invariants();
        return None;
    }

    fn propagate_clause(&mut self, clause_id: ClauseId, p: Lit) -> bool {
        let lits = &mut self.clauses[clause_id].disjuncts;
        if lits[0] == !p {
            lits.swap(0, 1);
        }
        debug_assert!(lits[1] == !p);
        let lits = &self.clauses[clause_id].disjuncts;
        if self.is_set(lits[0]) {
            // clause satisfied, restore the watch and exit
            self.watches[p].push(clause_id);
            //            self.check_invariants();
            return true;
        }
        for i in 2..lits.len() {
            if !self.is_set(!lits[i]) {
                let lits = &mut self.clauses[clause_id].disjuncts;
                lits.swap(1, i);
                self.watches[!lits[1]].push(clause_id);
                //                self.check_invariants();
                return true;
            }
        }
        // no replacement found, clause is unit
        trace!("Unit clause {}: {}", clause_id, self.clauses[clause_id]);
        self.watches[p].push(clause_id);
        return self.enqueue(lits[0], Some(clause_id));
    }
    fn is_set(&self, lit: Lit) -> bool {
        match self.assignments.get(lit.variable()) {
            BVal::Undef => false,
            BVal::True => lit.is_positive(),
            BVal::False => lit.is_negative(),
        }
    }
    pub fn enqueue(&mut self, lit: Lit, reason: Option<ClauseId>) -> bool {
        if self.is_set(!lit) {
            // contradiction
            false
        } else if self.is_set(lit) {
            // already known
            true
        } else {
            trace!("enqueued: {}", lit);
            self.assignments
                .set(lit.variable(), lit.is_positive(), reason);
            self.propagation_queue.push(lit);
            //            self.check_invariants();
            true
        }
    }

    fn analyze(&self, original_clause: ClauseId) -> (Vec<Lit>, DecisionLevel) {
        // TODO: many allocations to optimize here
        let mut seen = vec![false; self.num_vars as usize + 1]; // todo: use a bitvector
        let mut counter = 0;
        let mut p = None;
        let mut p_reason = Vec::new();
        let mut out_learnt = Vec::new();
        let mut out_btlevel = GROUND_LEVEL;

        let mut clause = Some(original_clause);
        let mut simulated_undone = 0;

        out_learnt.push(Lit::dummy());

        let mut first = true;
        while first || counter > 0 {
            first = false;
            p_reason.clear();
            debug_assert!(clause.is_some(), "Analyzed clause is empty.");
            self.calc_reason(clause.unwrap(), p, &mut p_reason);

            for &q in &p_reason {
                let qvar = q.variable();
                if !seen[q.variable().to_index()] {
                    seen[q.variable().to_index()] = true;
                    if self.assignments.level(qvar) == self.assignments.decision_level() {
                        counter += 1;
                    } else if self.assignments.level(qvar) > GROUND_LEVEL {
                        out_learnt.push(!q);
                        out_btlevel = out_btlevel.max(self.assignments.level(qvar));
                    }
                }
            }

            while {
                // do
                let l = self.assignments.last_assignment(simulated_undone);
                debug_assert!(
                    self.assignments.level(l.variable()) == self.assignments.decision_level()
                );
                p = Some(l);
                clause = self.assignments.reason(l.variable());

                simulated_undone += 1;

                // while
                !seen[l.variable().to_index()]
            } {}
            counter -= 1;
        }
        debug_assert!(out_learnt[0] == Lit::dummy());
        out_learnt[0] = !p.unwrap();

        (out_learnt, out_btlevel)
    }

    fn calc_reason(&self, clause: ClauseId, op: Option<Lit>, out_reason: &mut Vec<Lit>) {
        let cl = &self.clauses[clause];
        debug_assert!(out_reason.is_empty());
        debug_assert!(
            op.iter().all(|&p| cl.disjuncts[0] == p),
            "{} -- {}",
            cl,
            op.unwrap()
        );
        let first = match op {
            Some(_) => 1,
            None => 0,
        };
        for &l in &cl.disjuncts[first..] {
            out_reason.push(!l);
        }
        // TODO : bump activity if learnt
    }

    fn backtrack(&mut self) -> Option<Decision> {
        let h = &mut self.heuristic;
        self.assignments.backtrack(&mut |v| h.var_insert(v))
    }

    fn backtrack_to(&mut self, lvl: DecisionLevel) -> Option<Decision> {
        let h = &mut self.heuristic;
        self.assignments.backtrack_to(lvl, &mut |v| h.var_insert(v))
    }

    /// Return None if no solution was found within the conflict limit.
    ///
    fn search(
        &mut self,
        nof_conflicts: usize,
        nof_learnt: usize,
        params: &SearchParams,
        stats: &mut Stats,
    ) -> Option<bool> {
        debug_assert!(self.assignments.decision_level() == self.assignments.root_level());

        let var_decay = 1_f64 / params.var_decay;
        let cla_decay = 1_f64 / params.cla_decay;

        let mut conflict_count: usize = 0;

        loop {
            match self.propagate() {
                Some(conflict) => {
                    stats.conflicts += 1;
                    conflict_count += 1;

                    if self.assignments.decision_level() == self.assignments.root_level() {
                        return Some(false);
                    } else {
                        if params.use_learning {
                            let (learnt_clause, backtrack_level) = self.analyze(conflict);
                            match self.backtrack_to(backtrack_level) {
                                Some(dec) => trace!("backtracking: {:?}", !dec),
                                None => return Some(false), // no decision left to undo
                            }
                            let added_clause = self.add_clause(&learnt_clause[..], true);

                            match added_clause {
                                AddClauseRes::Inconsistent => return Some(false),
                                AddClauseRes::Unit(l) => {
                                    debug_assert!(learnt_clause[0] == l);
                                    self.enqueue(l, None);
                                }
                                AddClauseRes::Complete(cl_id) => {
                                    debug_assert!(
                                        learnt_clause[0] == self.clauses[cl_id].disjuncts[0]
                                    );
                                    self.enqueue(learnt_clause[0], Some(cl_id));
                                }
                            }
                        // cancel until
                        // record clause
                        // decay activities
                        } else {
                            match self.backtrack() {
                                Some(dec) => {
                                    trace!("backtracking: {:?}", !dec);
                                    self.assume(!dec, None);
                                }
                                None => {
                                    return Some(false); // no decision left to undo
                                }
                            }
                        }
                    }
                }
                None => {
                    if self.assignments.decision_level() == GROUND_LEVEL {
                        // TODO: simplify db
                    }
                    if self.num_learnt() as i64 - self.assignments.num_assigned() as i64
                        >= nof_learnt as i64
                    {
                        // TODO: reduce learnt set
                    }

                    if self.num_vars() as usize == self.assignments.num_assigned() {
                        // model found
                        debug_assert!(self.is_model_valid());
                        return Some(true);
                    } else if conflict_count > nof_conflicts {
                        // reached bound on number of conflicts
                        // cancel until root level
                        self.backtrack_to(self.assignments.root_level());
                        return None;
                    } else {
                        let next: BVar = loop {
                            match self.heuristic.next_var() {
                                Some(v) if !self.assignments.is_set(v) => break v, // // not set, select for decision
                                Some(_) => continue, // var already set, proceed to next
                                None => panic!("No unbound value in the heap."),
                            }
                        };

                        self.decide(Decision::True(next));
                        stats.decisions += 1;
                    }
                }
            }
        }
    }
    fn num_vars(&self) -> u32 {
        self.num_vars
    }
    fn num_learnt(&self) -> usize {
        //TODO
        0
    }

    pub fn solve(&mut self, params: &SearchParams) -> bool {
        let mut stats = Stats::default();
        let init_time = time::precise_time_s();

        let mut nof_conflicts = params.init_nof_conflict as f64;
        let mut nof_learnt = self.clauses.num_clauses() as f64 / params.init_learnt_ratio;

        loop {
            match self.search(
                nof_conflicts as usize,
                nof_learnt as usize,
                params,
                &mut stats,
            ) {
                Some(is_sat) => {
                    let runtime = time::precise_time_s() - init_time;
                    print_stats(&stats, runtime);

                    return is_sat;
                }
                None => {
                    // no decision made within bounds
                    nof_conflicts *= 1.5;
                    nof_learnt *= 1.1;
                    stats.restarts += 1;
                }
            }
        }
    }

    fn is_model_valid(&self) -> bool {
        self.check_invariants();
        for cl_id in self.clauses.all_clauses() {
            let mut is_sat = false;
            for lit in &self.clauses[cl_id].disjuncts {
                if self.is_set(*lit) {
                    is_sat = true;
                }
            }
            if !is_sat {
                trace!("Invalid clause: {}: {}", cl_id, self.clauses[cl_id]);
                return false;
            }
        }
        true
    }

    #[cfg(not(feature = "full_check"))]
    fn check_invariants(&self) {}

    #[cfg(feature = "full_check")]
    fn check_invariants(&self) {
        let mut watch_count = IndexMap::new(self.clauses.num_clauses(), 0);
        for watches_for_lit in &self.watches.values[1..] {
            for watcher in watches_for_lit {
                watch_count[*watcher] += 1;
            }
        }
        assert!(watch_count.values.iter().all(|&n| n == 2))
    }
}

use crate::core::clause::{Clause, ClauseDB, ClauseId};
use crate::core::heuristic::{Heur, HeurParams};
use crate::core::stats::{print_stats, Stats};
use env_logger::Target;
use log::LevelFilter;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opt {
    file: String,
    #[structopt(long = "sat")]
    expected_satifiability: Option<bool>,
    #[structopt(short = "v")]
    verbose: bool,
}

fn main() {
    let opt = Opt::from_args();
    env_logger::builder()
        .filter_level(if opt.verbose {
            LevelFilter::Trace
        } else {
            LevelFilter::Info
        })
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .target(Target::Stdout)
        .init();

    log::debug!("Options: {:?}", opt);

    let filecontent = fs::read_to_string(opt.file).expect("Cannot read file");

    let clauses = core::cnf::CNF::parse(&filecontent).clauses;

    let mut solver = Solver::init(clauses);
    let vars = solver.variables();
    let sat = solver.solve(&SearchParams::default());
    match sat {
        true => {
            assert!(solver.is_model_valid());

            info!("==== Model found ====");

            let mut v = *vars.start();
            while v <= *vars.end() {
                debug!("{} <- {:?}", v.to_index(), solver.assignments.get(v));
                v = v.next();
            }
            if opt.expected_satifiability == Some(false) {
                eprintln!("Error: expected UNSAT but got SAT");
                std::process::exit(1);
            }
        }
        false => {
            info!("Unsat");

            if opt.expected_satifiability == Some(true) {
                eprintln!("Error: expected SAT but got UNSAT");
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let a = BVar::from_bits(1);
        let at = a.true_lit();
        assert_eq!(at.id.get(), 1 * 2 + 1);
        let af = a.false_lit();
        assert_eq!(af.id.get(), 1 * 2);
        assert_eq!(a, at.variable());
        assert_eq!(a, af.variable());
        assert_ne!(at, af);
    }
    #[test]
    #[should_panic]
    fn test_invalid_zero() {
        BVar::from_bits(0);
    }
    #[test]
    #[should_panic]
    fn test_invalid_too_big() {
        BVar::from_bits(std::u32::MAX);
    }
}
