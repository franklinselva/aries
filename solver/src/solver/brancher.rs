use crate::solver::stats::Stats;
use aries_backtrack::{Backtrack, DecLvl, Trail};
use aries_collections::heap::IdxHeap;
use aries_model::assignments::Assignment;
use env_param::EnvParam;

use aries_collections::ref_store::RefMap;
use aries_model::int_model::IntDomain;

use aries_model::bounds::Bound;
use aries_model::lang::{IntCst, VarRef};
use aries_model::Model;
use itertools::Itertools;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

pub static PREFER_MIN_VALUE: EnvParam<bool> = EnvParam::new("ARIES_SMT_PREFER_MIN_VALUE", "true");
pub static INITIALLY_ALLOWED_CONFLICTS: EnvParam<u64> = EnvParam::new("ARIES_SMT_INITIALLY_ALLOWED_CONFLICT", "100");
pub static INCREASE_RATIO_FOR_ALLOWED_CONFLICTS: EnvParam<f32> =
    EnvParam::new("ARIES_SMT_INCREASE_RATIO_FOR_ALLOWED_CONFLICTS", "1.5");

#[derive(Clone)]
pub struct BranchingParams {
    pub prefer_min_value: bool,
    pub allowed_conflicts: u64,
    pub increase_ratio_for_allowed_conflicts: f32,
}

impl Default for BranchingParams {
    fn default() -> Self {
        BranchingParams {
            prefer_min_value: PREFER_MIN_VALUE.get(),
            allowed_conflicts: INITIALLY_ALLOWED_CONFLICTS.get(),
            increase_ratio_for_allowed_conflicts: INCREASE_RATIO_FOR_ALLOWED_CONFLICTS.get(),
        }
    }
}

#[derive(Clone)]
pub struct Brancher {
    pub params: BranchingParams,
    heap: VarSelect,
    default_assignment: DefaultValues,
    conflicts_at_last_restart: u64,
    num_processed_var: usize,
    rng: StdRng,
}

#[derive(Clone, Default)]
struct DefaultValues {
    bools: RefMap<VarRef, IntCst>,
}

pub enum Decision {
    SetLiteral(Bound),
    Restart,
}

impl Brancher {
    pub fn new() -> Self {
        Brancher {
            params: Default::default(),
            heap: VarSelect::new(Default::default()),
            default_assignment: DefaultValues::default(),
            conflicts_at_last_restart: 0,
            num_processed_var: 0,
            rng: StdRng::seed_from_u64(0),
        }
    }

    pub fn set_seed(&mut self, seed: u64) {
        self.rng = StdRng::seed_from_u64(seed);
    }

    pub fn import_vars(&mut self, model: &Model) {
        let mut count = 0;
        for var in model.discrete.variables().dropping(self.num_processed_var) {
            debug_assert!(!self.heap.is_declared(var));
            let priority = if model.var_domain(var).size() <= 1 { 0 } else { 1 };
            let activity = self.rng.gen_range(0.98_f32..1.02_f32);
            self.heap.add_variable(var, priority, Some(activity));
            count += 1;
        }
        self.num_processed_var += count;
    }

    /// Select the next decision to make while maintaining the invariant that every non bound variable remains in the queue.
    ///
    /// This invariant allows to invoke this function at the decision level preceding the one of the decision that will be returned.
    /// A nice side-effects is that any variable that is bound and remove from the queue will only be added back if backtracking
    /// to the level preceding the decision to be made.
    ///
    /// Returns `None` if no decision is left to be made.
    pub fn next_decision(&mut self, stats: &Stats, model: &Model) -> Option<Decision> {
        self.import_vars(model);

        let mut popper = self.heap.extractor();

        // extract the highest priority variable that is not set yet.
        let next_unset = loop {
            // we are only allowed to remove from the queue variables that are bound.
            // so peek at the next one an only remove it if it was
            match popper.peek() {
                Some(v) => {
                    if model.discrete.domains.is_bound(v) || model.discrete.domains.present(v) == Some(false) {
                        // already bound or absent, drop the peeked variable before proceeding to next
                        popper.pop().unwrap();
                    } else {
                        // not set, select for decision
                        break Some(v);
                    }
                }
                None => {
                    // no variables left in queue
                    break None;
                }
            }
        };
        if let Some(v) = next_unset {
            if stats.num_conflicts - self.conflicts_at_last_restart >= self.params.allowed_conflicts {
                // we have exceeded the number of allowed conflict, time for a restart
                self.conflicts_at_last_restart = stats.num_conflicts;
                // increase the number of allowed conflicts
                self.params.allowed_conflicts =
                    (self.params.allowed_conflicts as f32 * self.params.increase_ratio_for_allowed_conflicts) as u64;

                Some(Decision::Restart)
            } else {
                // determine value for literal:
                // - first from per-variable preferred assignments
                // - otherwise from the preferred value for boolean variables
                let IntDomain { lb, ub } = model.var_domain(v);
                debug_assert!(lb < ub);

                let value = self
                    .default_assignment
                    .bools
                    .get(v)
                    .copied()
                    .unwrap_or(if self.params.prefer_min_value { lb } else { ub });

                let literal = if value < lb || value > ub {
                    if self.params.prefer_min_value {
                        Bound::leq(v, lb)
                    } else {
                        Bound::geq(v, ub)
                    }
                } else if ub > value && self.params.prefer_min_value {
                    Bound::leq(v, value)
                } else if lb < value {
                    Bound::geq(v, value)
                } else {
                    debug_assert!(ub > value);
                    Bound::leq(v, value)
                };

                Some(Decision::SetLiteral(literal))
            }
        } else {
            // all variables are set, no decision left
            None
        }
    }

    pub fn set_default_value(&mut self, var: VarRef, val: IntCst) {
        self.default_assignment.bools.insert(var, val);
    }

    pub fn set_default_values_from(&mut self, assignment: &Model) {
        self.import_vars(assignment);
        for (var, val) in assignment.discrete.bound_variables() {
            self.set_default_value(var, val);
        }
    }

    /// Increase the activity of the variable and perform an reordering in the queue.
    /// The activity is then used to select the next variable.
    pub fn bump_activity(&mut self, bvar: VarRef) {
        self.heap.var_bump_activity(bvar);
    }
}

impl Default for Brancher {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct BoolHeuristicParams {
    pub var_inc: f32,
    pub var_decay: f32,
}
impl Default for BoolHeuristicParams {
    fn default() -> Self {
        BoolHeuristicParams {
            var_inc: 1_f32,
            var_decay: 0.95_f32,
        }
    }
}

/// Heuristic value associated to a variable.
#[derive(Copy, Clone, PartialEq, PartialOrd)]
struct BoolVarHeuristicValue {
    activity: f32,
}

type Heap = IdxHeap<VarRef, BoolVarHeuristicValue>;

/// Changes that need to be undone.
/// The only change that we need to undo is the removal from the queue.
/// When extracting a variable from the queue, it will be checked whether the variable
/// should be returned to the caller. Thus it is correct to have a variable in the queue
/// that will never be send to a caller.
#[derive(Copy, Clone)]
enum HeapEvent {
    Removal(VarRef, u8),
}

#[derive(Clone)]
pub struct VarSelect {
    params: BoolHeuristicParams,
    /// One heap for each decision stage.
    heaps: Vec<Heap>,
    /// Stage in which each variable appears.
    stages: RefMap<VarRef, u8>,
    trail: Trail<HeapEvent>,
}

impl VarSelect {
    pub fn new(params: BoolHeuristicParams) -> Self {
        VarSelect {
            params,
            heaps: Vec::new(),
            stages: Default::default(),
            trail: Trail::default(),
        }
    }

    pub fn is_declared(&self, v: VarRef) -> bool {
        self.stages.contains(v)
    }

    /// Declares a new variable. The variable is NOT added to the queue.
    /// THe stage parameters define at which stage of the search the variable will be selected.
    /// Variables with the lowest stage are considered first.
    pub fn add_variable(&mut self, v: VarRef, stage: u8, initial_activity: Option<f32>) {
        debug_assert!(!self.is_declared(v));
        let hvalue = BoolVarHeuristicValue {
            activity: initial_activity.unwrap_or(self.params.var_inc),
        };
        let priority = stage as usize;
        while priority >= self.heaps.len() {
            self.heaps.push(IdxHeap::new());
        }
        self.heaps[priority].declare_element(v, hvalue);
        self.heaps[priority].enqueue(v);
        self.stages.insert(v, priority as u8);
    }

    fn stage_of(&self, v: VarRef) -> u8 {
        self.stages[v]
    }

    fn heap_of(&mut self, v: VarRef) -> &mut Heap {
        let heap_index = self.stage_of(v) as usize;
        &mut self.heaps[heap_index]
    }

    /// Add the value to the queue, the variable must have been previously declared.
    pub fn enqueue_variable(&mut self, var: VarRef) {
        self.heap_of(var).enqueue(var)
    }

    /// Provides an iterator over variables in the heap.
    /// Variables are provided by increasing priority.
    pub fn extractor(&mut self) -> Popper {
        let mut heaps = self.heaps.iter_mut();
        let current_heap = heaps.next();
        Popper {
            heaps,
            current_heap,
            stage: 0,
            trail: &mut self.trail,
        }
    }

    pub fn var_bump_activity(&mut self, var: VarRef) {
        let var_inc = self.params.var_inc;
        let heap = self.heap_of(var);
        heap.change_priority(var, |p| p.activity += var_inc);
        if heap.priority(var).activity > 1e30_f32 {
            self.var_rescale_activity()
        }
    }

    pub fn decay_activities(&mut self) {
        self.params.var_inc /= self.params.var_decay;
    }

    fn var_rescale_activity(&mut self) {
        // here we scale the activity of all variables, to avoid overflowing
        // this can not change the relative order in the heap, since activities are scaled by the same amount.
        for heap in &mut self.heaps {
            heap.change_all_priorities_in_place(|p| p.activity *= 1e-30_f32);
        }

        self.params.var_inc *= 1e-30_f32;
    }
}
impl Backtrack for VarSelect {
    fn save_state(&mut self) -> DecLvl {
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        let heaps = &mut self.heaps;
        self.trail.restore_last_with(|HeapEvent::Removal(var, prio)| {
            heaps[prio as usize].enqueue(var);
        })
    }
}

pub struct Popper<'a> {
    heaps: std::slice::IterMut<'a, Heap>,
    current_heap: Option<&'a mut Heap>,
    stage: u8,
    trail: &'a mut Trail<HeapEvent>,
}

impl<'a> Popper<'a> {
    pub fn peek(&mut self) -> Option<VarRef> {
        while let Some(curr) = &self.current_heap {
            if let Some(var) = curr.peek().copied() {
                return Some(var);
            } else {
                self.current_heap = self.heaps.next();
                self.stage += 1;
            }
        }
        None
    }

    pub fn pop(&mut self) -> Option<VarRef> {
        while let Some(curr) = &mut self.current_heap {
            if let Some(var) = curr.pop() {
                self.trail.push(HeapEvent::Removal(var, self.stage as u8));
                return Some(var);
            } else {
                self.current_heap = self.heaps.next();
                self.stage += 1;
            }
        }
        None
    }
}

impl Backtrack for Brancher {
    fn save_state(&mut self) -> DecLvl {
        self.heap.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.heap.num_saved()
    }

    fn restore_last(&mut self) {
        self.heap.restore_last()
    }
}
