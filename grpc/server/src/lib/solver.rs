#![allow(dead_code)] // TODO: remove once we exploit the code

use anyhow::{Error, Result};

use aries_planners::{Option, Planner};

use super::chronicles::{problem_to_chronicles, translate_answer};

// Aries solver based on the problem defined by Unified Planning Framework
pub fn solve(problem: aries_grpc_api::Problem) -> Result<aries_grpc_api::Answer, Error> {
    //TODO: Get the options from the problem
    let opt = Option::default();
    //TODO: Check if the options are valid for the planner
    let mut planner = Planner::new(opt.clone());

    // println!("{:?}", problem);
    let _spec = problem_to_chronicles(&problem)?;
    planner.solve(_spec, &opt)?;
    let answer = planner.get_answer();
    planner.format_plan(&answer)?;
    let answer = translate_answer(&problem, &planner.problem.unwrap(), &answer).unwrap();

    Ok(answer)
}
