use malachite::Rational;

use crate::traits::interpreter::Interpreter;

use super::action::DurativeAction;

/*******************************************************************/

#[derive(Clone, Debug, PartialEq, Eq)]
/// Kinds of timepoints.
pub enum TimepointKind {
    /// Start of the problem.
    GlobalStart,
    /// End of the problem.
    GlobalEnd,
    /// Start of the container.
    Start,
    /// End of the container.
    End,
}

/*******************************************************************/

#[derive(Clone, Debug, PartialEq, Eq)]
/// Reference to an absolute time.
pub struct Timepoint {
    kind: TimepointKind,
    delay: Rational,
}

impl Timepoint {
    pub fn new(kind: TimepointKind, delay: Rational) -> Self {
        Self { kind, delay }
    }

    /// Builds a fixed timepoint
    pub fn fixed(instant: Rational) -> Self {
        Self {
            kind: TimepointKind::GlobalStart,
            delay: instant,
        }
    }

    /// Builds a timepoint representing the PDDL `at-start`.
    pub fn at_start() -> Self {
        Self {
            kind: TimepointKind::Start,
            delay: 0.into(),
        }
    }

    /// Builds a timepoint representing the PDDL `at-end`.
    pub fn at_end() -> Self {
        Self {
            kind: TimepointKind::End,
            delay: 0.into(),
        }
    }
}

impl Default for Timepoint {
    fn default() -> Self {
        Self {
            kind: TimepointKind::GlobalStart,
            delay: 0.into(),
        }
    }
}

impl Timepoint {
    /// Evaluates the value of the timepoint for the given container.
    pub fn eval<E: Interpreter>(&self, container: Option<&DurativeAction<E>>, global_end: &Rational) -> Rational {
        let b = match self.kind {
            TimepointKind::GlobalStart => 0.into(),
            TimepointKind::GlobalEnd => global_end.clone(),
            TimepointKind::Start => {
                if let Some(c) = container {
                    c.start().eval::<E>(None, global_end)
                } else {
                    0.into()
                }
            }
            TimepointKind::End => {
                if let Some(c) = container {
                    c.end().eval::<E>(None, global_end)
                } else {
                    global_end.clone()
                }
            }
        };
        b + self.delay.clone()
    }
}

/*******************************************************************/

#[derive(Clone, Debug, PartialEq, Eq)]
/// Temporal interval [start, end] which can be opened or closed with abstract timepoints.
pub struct TemporalInterval {
    /// The lower bound of the interval.
    start: Timepoint,
    /// The upper bound of the interval.
    end: Timepoint,
    /// Whether or not the lower bound is open.
    is_start_open: bool,
    /// Whether or not the upper bound is open.
    is_end_open: bool,
}

impl TemporalInterval {
    pub fn new(start: Timepoint, end: Timepoint, is_start_open: bool, is_end_open: bool) -> Self {
        Self {
            start,
            end,
            is_start_open,
            is_end_open,
        }
    }

    /// Builds a temporal interval [at-start, at-start].
    pub fn at_start() -> Self {
        Self::new(Timepoint::at_start(), Timepoint::at_start(), false, false)
    }

    /// Returns whether or not the timepoint is in the interval for the given container.
    pub fn contains<E: Interpreter>(
        &self,
        timepoint: &Rational,
        container: Option<&DurativeAction<E>>,
        global_end: &Rational,
    ) -> bool {
        let start = &self.start.eval(container, global_end);
        let end = &self.end.eval(container, global_end);
        if (start == timepoint && self.is_start_open) || (end == timepoint && self.is_end_open) {
            false
        } else {
            start <= timepoint && timepoint <= end
        }
    }

    /// Returns the lower bound of the interval.
    pub fn start(&self) -> &Timepoint {
        &self.start
    }

    /// Returns the upper bound of the interval.
    pub fn end(&self) -> &Timepoint {
        &self.end
    }

    /// Returns whether or not the lower is open.
    pub fn is_start_open(&self) -> bool {
        self.is_start_open
    }

    /// Returns whether or not the upper is open.
    pub fn is_end_open(&self) -> bool {
        self.is_end_open
    }
}

/*******************************************************************/

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::models::{action::DurativeAction, env::Env, value::Value};

    use super::*;

    #[derive(Clone)]
    struct MockExpr(Value);
    impl Default for MockExpr {
        fn default() -> Self {
            Self(true.into())
        }
    }
    impl Interpreter for MockExpr {
        fn eval(&self, _: &Env<Self>) -> Result<Value> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn eval() {
        let a = DurativeAction::<MockExpr>::new(
            "d".into(),
            vec![],
            vec![],
            Timepoint::fixed(5.into()),
            Timepoint::fixed(10.into()),
        );

        let kinds = [
            TimepointKind::GlobalStart,
            TimepointKind::GlobalEnd,
            TimepointKind::Start,
            TimepointKind::End,
        ];
        let delays = [0, 2, -2];
        let expected = [0, 30, 5, 10, 2, 32, 7, 12, -2, 28, 3, 8];
        for i in 0..delays.len() {
            for j in 0..kinds.len() {
                let delay = delays[i];
                let kind = kinds[j].clone();
                let expect = expected[i * kinds.len() + j];
                assert_eq!(
                    Timepoint::new(kind, delay.into()).eval(Some(&a.clone().into()), &30.into()),
                    Rational::from(expect)
                );
            }
        }
    }

    #[test]
    fn contains() {
        let start = Timepoint::fixed(5.into());
        let end = Timepoint::fixed(10.into());
        let timepoints = [Rational::from(5), Rational::from_signeds(15, 2), Rational::from(10)];
        let global_end = &Rational::from(30);

        for is_start_open in [true, false] {
            for is_end_open in [true, false] {
                let i = TemporalInterval::new(start.clone(), end.clone(), is_start_open, is_end_open);
                for timepoint in timepoints.iter() {
                    let expected = timepoint == &timepoints[1]
                        || (!is_start_open && timepoint == &timepoints[0])
                        || (!is_end_open && timepoint == &timepoints[2]);
                    assert_eq!(i.contains::<MockExpr>(timepoint, None, global_end), expected);
                }
            }
        }
    }
}
