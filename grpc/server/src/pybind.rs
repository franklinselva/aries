use pyo3::exceptions::PyException;
use pyo3::prelude::*;

use aries_grpc_api::{Answer, Problem};

mod lib;
use lib::solver::solve;

// The following function a python binding based on rust
// The module name is aries and holds the below functions:-
// solver() <- Planner function based on python binding
// help() <- Helper text for the planner
// doc() <- Documentation brief for ARIES
#[pymodule]
fn aries(_py: Python, m: &PyModule) -> PyResult<()> {
    // m.add_function(wrap_pyfunction!(solver, m)?)?;
    m.add_function(wrap_pyfunction!(doc, m)?)?;
    m.add_function(wrap_pyfunction!(help, m)?)?;
    Ok(())
}

#[pyclass]
struct PyAnswer {
    answer: Answer,
}

#[pyclass]
#[derive(Debug, Clone)]
struct PyProblem {
    problem: Problem,
}

#[pyfunction]
fn solver(problem: PyProblem) -> PyResult<PyAnswer> {
    let answer = solve(problem.problem);
    if let Ok(answer) = answer {
        Ok(PyAnswer { answer })
    } else {
        Err(PyErr::new::<PyException, _>(answer.unwrap_err().to_string()))
    }
}

#[pyfunction]
fn help() -> String {
    // TODO: Add help text for helper functions
    // like solvable operations
    let help = r#"
    Solve a problem using the Aries Planner.
    Usage:
        solver(problem)
    Arguments:
        problem: Problem
    Returns:
        Answer
    "#;
    help.to_string()
}

#[pyfunction]
fn doc() -> String {
    // TODO: Add documentation
    let doc = r#"
    Solve a problem using the Aries Planner.
    Usage:
        solver(problem)
    Arguments:
        problem: Problem
    Returns:
        Answer
    "#;
    doc.to_string()
}
