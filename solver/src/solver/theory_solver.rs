use crate::solver::BindingResult;
use crate::{Contradiction, Theory};
use aries_backtrack::{Backtrack, DecLvl};
use aries_model::bounds::Lit;
use aries_model::lang::reification::Expr;
use aries_model::state::Domains;
use aries_model::Model;

// TODO: remove this useless layer
pub struct TheorySolver {
    pub theory: Box<dyn Theory>,
}

impl TheorySolver {
    pub fn new(theory: Box<dyn Theory>) -> TheorySolver {
        TheorySolver { theory }
    }

    pub fn bind(&mut self, lit: Lit, expr: &Expr, model: &mut Model) -> BindingResult {
        self.theory.bind(lit, expr, model)
    }

    pub fn process(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        self.theory.propagate(model)
    }

    pub fn print_stats(&self) {
        self.theory.print_stats()
    }
}

impl Clone for TheorySolver {
    fn clone(&self) -> Self {
        TheorySolver {
            theory: self.theory.clone_box(),
        }
    }
}

impl Backtrack for TheorySolver {
    fn save_state(&mut self) -> DecLvl {
        self.theory.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.theory.num_saved()
    }

    fn restore_last(&mut self) {
        self.theory.restore_last()
    }

    fn restore(&mut self, saved_id: DecLvl) {
        self.theory.restore(saved_id)
    }
}
