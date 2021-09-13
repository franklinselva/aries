use anyhow::*;
use aries_model::extensions::SavedAssignment;
use aries_planners::encode::{encode, populate_with_task_network, populate_with_template_instances};
use aries_planners::fmt::{format_hddl_plan, format_partial_plan, format_pddl_plan};
use aries_planning::chronicles::*;
use aries_planning::parsing::pddl::{find_domain_of, parse_pddl_domain, parse_pddl_problem, PddlFeature};
use aries_planning::parsing::pddl_to_chronicles;
use aries_solver::solver::Solver;
use aries_tnet::theory::{StnConfig, StnTheory, TheoryPropagationLevel};
use aries_utils::input::Input;

use aries_planners::forward_search::ForwardSearcher;
use aries_solver::parallel_solver::ParSolver;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use structopt::StructOpt;

/// An automated planner for PDDL and HDDL problems.
#[derive(Debug, StructOpt)]
#[structopt(name = "lcp", rename_all = "kebab-case")]
struct Opt {
    /// path to the domain file (if not provided, the solver will try to find it based on naming conventions).
    #[structopt(long, short)]
    domain: Option<PathBuf>,
    /// Path to the problem file.
    problem: PathBuf,
    /// If set, a machine readable plan will be written to the file.
    #[structopt(long = "output", short = "o")]
    plan_out_file: Option<PathBuf>,
    /// Minimum depth of the instantiation. (depth of HTN tree or number of standalone actions with the same name).
    #[structopt(long, default_value = "0")]
    min_depth: u32,
    /// Maximum depth of instantiation
    #[structopt(long)]
    max_depth: Option<u32>,
    /// If set, the solver will attempt to minimize the makespan of the plan.
    #[structopt(long = "optimize")]
    optimize_makespan: bool,
    /// If true, then the problem will be constructed, a full propagation will be made and the resulting
    /// partial plan will be displayed.
    #[structopt(long = "no-search")]
    no_search: bool,
}

fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();

    let problem_file = &opt.problem;
    ensure!(
        problem_file.exists(),
        "Problem file {} does not exist",
        problem_file.display()
    );

    let problem_file = problem_file.canonicalize().unwrap();
    let domain_file = match opt.domain {
        Some(name) => name,
        None => find_domain_of(&problem_file).context("Consider specifying the domain with the option -d/--domain")?,
    };

    let dom = Input::from_file(&domain_file)?;
    let prob = Input::from_file(&problem_file)?;

    let dom = parse_pddl_domain(dom)?;
    let prob = parse_pddl_problem(prob)?;

    // true if we are doing HTN planning, false otherwise
    let htn_mode = dom.features.contains(&PddlFeature::Hierarchy);

    let mut spec = pddl_to_chronicles(&dom, &prob)?;

    println!("===== Preprocessing ======");
    aries_planning::chronicles::preprocessing::preprocess(&mut spec);
    println!("==========================");

    for n in opt.min_depth..opt.max_depth.unwrap_or(u32::MAX) {
        println!("{} Solving with {} actions", n, n);
        let start = Instant::now();
        let mut pb = FiniteProblem {
            model: spec.context.model.clone(),
            origin: spec.context.origin(),
            horizon: spec.context.horizon(),
            chronicles: spec.chronicles.clone(),
            tables: spec.context.tables.clone(),
        };
        if htn_mode {
            populate_with_task_network(&mut pb, &spec, n)?;
        } else {
            populate_with_template_instances(&mut pb, &spec, |_| Some(n))?;
        }
        println!("  [{:.3}s] Populated", start.elapsed().as_secs_f32());
        let start = Instant::now();
        if opt.no_search {
            propagate_and_print(&pb);
            break;
        } else {
            let result = solve(&pb, opt.optimize_makespan, htn_mode);
            println!("  [{:.3}s] solved", start.elapsed().as_secs_f32());
            if let Some(x) = result {
                // println!("{}", format_partial_plan(&pb, &x)?);
                println!("  Solution found");
                let plan = if htn_mode {
                    format_hddl_plan(&pb, &x)?
                } else {
                    format_pddl_plan(&pb, &x)?
                };
                println!("{}", plan);
                if let Some(plan_out_file) = opt.plan_out_file {
                    let mut file = File::create(plan_out_file)?;
                    file.write_all(plan.as_bytes())?;
                }
                break;
            }
        }
    }

    Ok(())
}

fn init_solver(pb: &FiniteProblem) -> Box<Solver> {
    let model = encode(pb).unwrap(); // TODO: report error
    let stn_config = StnConfig {
        theory_propagation: TheoryPropagationLevel::Full,
        ..Default::default()
    };

    let mut solver = Box::new(aries_solver::solver::Solver::new(model));
    solver.add_theory(|tok| StnTheory::new(tok, stn_config));
    solver
}

fn solve(pb: &FiniteProblem, optimize_makespan: bool, htn_mode: bool) -> Option<std::sync::Arc<SavedAssignment>> {
    let solver = init_solver(pb);
    let mut solver = if htn_mode {
        aries_solver::parallel_solver::ParSolver::new(solver, 2, |id, s| match id {
            0 => {}                                                          // default brancher
            1 => s.set_brancher(ForwardSearcher::new(Arc::new(pb.clone()))), // brancher based on forward search.
            _ => unreachable!(),
        })
    } else {
        ParSolver::new(solver, 1, |_, _| {})
    };

    let found_plan = if optimize_makespan {
        let res = solver.minimize(pb.horizon).unwrap();
        res.map(|tup| tup.1)
    } else {
        solver.solve().unwrap()
    };

    if let Some(solution) = found_plan {
        solver.print_stats();
        Some(solution)
    } else {
        None
    }
}

fn propagate_and_print(pb: &FiniteProblem) {
    let mut solver = init_solver(pb);
    if solver.propagate_and_backtrack_to_consistent() {
        let str = format_partial_plan(pb, &solver.model).unwrap();
        println!("{}", str);
    } else {
        panic!("Invalid problem");
    }
}