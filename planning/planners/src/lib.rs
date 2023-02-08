use aries_planning::chronicles::VarLabel;

pub mod encode;
pub mod encoding;
pub mod fmt;
pub mod forward_search;
pub mod solver;

pub type Model = aries_model::Model<VarLabel>;
pub type Solver = aries_solver::solver::Solver<VarLabel>;
pub type ParSolver = aries_solver::parallel_solver::ParSolver<VarLabel>;