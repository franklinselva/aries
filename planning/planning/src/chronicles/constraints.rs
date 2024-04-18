use super::*;
use aries::core::Lit;
use aries::model::extensions::AssignmentExt;
use aries::model::lang::expr::*;
use aries::model::lang::linear::LinearSum;
use aries::model::lang::Type;
use aries::model::Label;
use std::fmt::Debug;
use ConstraintType::*;

/// Generic representation of a constraint on a set of variables
#[derive(Debug, Clone)]
pub struct Constraint {
    pub variables: Vec<Atom>,
    pub tpe: ConstraintType,
    /// If set, this constraint should be reified so that it is always equal to value.
    pub value: Option<Lit>,
}

impl Constraint {
    pub fn atom(a: impl Into<Atom>) -> Constraint {
        Constraint {
            variables: vec![a.into()],
            tpe: Or,
            value: None,
        }
    }

    pub fn lt(a: impl Into<Atom>, b: impl Into<Atom>) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Lt,
            value: None,
        }
    }
    pub fn leq(a: impl Into<Atom>, b: impl Into<Atom>) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Leq,
            value: None,
        }
    }
    pub fn reified_lt(a: impl Into<Atom>, b: impl Into<Atom>, constraint_value: Lit) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Lt,
            value: Some(constraint_value),
        }
    }
    pub fn reified_leq(a: impl Into<Atom>, b: impl Into<Atom>, constraint_value: Lit) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Leq,
            value: Some(constraint_value),
        }
    }
    pub fn eq(a: impl Into<Atom>, b: impl Into<Atom>) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Eq,
            value: None,
        }
    }
    pub fn reified_eq(a: impl Into<Atom>, b: impl Into<Atom>, constraint_value: Lit) -> Constraint {
        if constraint_value == Lit::FALSE {
            Self::neq(a, b)
        } else if constraint_value == Lit::TRUE {
            Self::eq(a, b)
        } else {
            Constraint {
                variables: vec![a.into(), b.into()],
                tpe: Eq,
                value: Some(constraint_value),
            }
        }
    }
    pub fn neq(a: impl Into<Atom>, b: impl Into<Atom>) -> Constraint {
        Constraint {
            variables: vec![a.into(), b.into()],
            tpe: Neq,
            value: None,
        }
    }

    pub fn duration(dur: Duration) -> Constraint {
        Constraint {
            variables: vec![],
            tpe: ConstraintType::Duration(dur),
            value: None,
        }
    }

    /// Constrains the given linear sum to be equal to zero.
    pub fn linear_eq_zero(sum: LinearSum) -> Constraint {
        Constraint {
            variables: vec![],
            tpe: ConstraintType::LinearEq(sum),
            value: None,
        }
    }

    pub fn table(variables: Vec<Atom>, values: Arc<Table<DiscreteValue>>) -> Self {
        Constraint {
            variables,
            tpe: ConstraintType::InTable(values),
            value: None,
        }
    }

    // /// Returns true if the
    // pub fn is_tautological(self) -> bool {
    //     match self.tpe {
    //         ConstraintType::Lt => {
    //             if self.variables.len() == 2 && let Some(a) = self.variables[0]
    //         }
    //     }
    // }
}

impl Substitute for Constraint {
    fn substitute(&self, substitution: &impl Substitution) -> Self {
        Constraint {
            variables: self.variables.iter().map(|i| substitution.sub(*i)).collect(),
            tpe: self.tpe.substitute(substitution),
            value: self.value.map(|v| substitution.sub_lit(v)),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ConstraintType {
    /// Variables should take a value as one of the tuples in the corresponding table.
    InTable(Arc<Table<DiscreteValue>>),
    Lt,
    Leq,
    Eq,
    Neq,
    Duration(Duration),
    Or,
    /// A linear sum that must equals zero
    LinearEq(LinearSum),
}

impl Substitute for ConstraintType {
    fn substitute(&self, substitution: &impl Substitution) -> Self {
        match self {
            Duration(Duration::Fixed(f)) => ConstraintType::Duration(Duration::Fixed(substitution.sub_linear_sum(f))),
            Duration(Duration::Bounded { lb, ub }) => ConstraintType::Duration(Duration::Bounded {
                lb: substitution.sub_linear_sum(lb),
                ub: substitution.sub_linear_sum(ub),
            }),
            LinearEq(sum) => LinearEq(substitution.sub_linear_sum(sum)),
            InTable(_) | Lt | Leq | Eq | Neq | Or => self.clone(), // no variables in those variants
        }
    }
}

/// A set of tuples, representing the allowed values in a table constraint.
#[derive(Clone)]
#[allow(dead_code)]
pub struct Table<E> {
    /// A human readable name to describe the table's content (typically the name of the property)
    pub name: String,
    /// Number of elements in the tuple
    line_size: usize,
    /// Type of the values in the tuples (length = line_size)
    types: Vec<Type>,
    /// linear representation of a matrix (each line occurs right after the previous one)
    inner: Vec<E>,
}

impl<E> Debug for Table<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "table({})", self.name)
    }
}

impl<E: Clone> Table<E> {
    pub fn new(name: String, types: Vec<Type>) -> Table<E> {
        Table {
            name,
            line_size: types.len(),
            types,
            inner: Vec::new(),
        }
    }

    pub fn push(&mut self, line: &[E]) {
        assert_eq!(line.len(), self.line_size);
        self.inner.extend_from_slice(line);
    }

    pub fn lines(&self) -> impl Iterator<Item = &[E]> {
        self.inner.chunks(self.line_size)
    }
}

/// Constraint that restricts the allowed durations of a chronicle
#[derive(Clone, Debug)]
pub enum Duration {
    /// The chronicle has a fixed the duration.
    Fixed(LinearSum),
    /// The duration must be between the lower and the upper bound (inclusive)
    Bounded { lb: LinearSum, ub: LinearSum },
}

/// Update the given model to enforce the constraints.
/// Context is given through the presence, start and end
/// of the chronicle in which the constraint appears.
pub fn encode_constraint<L: Label>(
    model: &mut Model<L>,
    constraint: &Constraint,
    presence: Lit,
    start: Time,
    end: Time,
) {
    let value = match constraint.value {
        // work around some dubious encoding of chronicle. The given value should have the appropriate scope
        Some(Lit::TRUE) | None => model.get_tautology_of_scope(presence),
        Some(Lit::FALSE) => !model.get_tautology_of_scope(presence),
        Some(l) => l,
    };
    match &constraint.tpe {
        ConstraintType::InTable(table) => {
            let mut supported_by_a_line: Vec<Lit> = Vec::with_capacity(256);

            let vars = &constraint.variables;
            for values in table.lines() {
                assert_eq!(vars.len(), values.len());
                let mut supported_by_this_line = Vec::with_capacity(16);
                for (&var, &val) in vars.iter().zip(values.iter()) {
                    match var {
                        Atom::Sym(s) => {
                            let DiscreteValue::Sym(val) = val else { panic!() };
                            supported_by_this_line.push(model.reify(eq(s, val)));
                        }
                        Atom::Int(var) => {
                            let DiscreteValue::Int(val) = val else { panic!() };
                            supported_by_this_line.push(model.reify(leq(var, val)));
                            supported_by_this_line.push(model.reify(geq(var, val)));
                        }
                        Atom::Bool(l) => {
                            let DiscreteValue::Bool(val) = val else { panic!() };
                            if val {
                                supported_by_this_line.push(l);
                            } else {
                                supported_by_this_line.push(!l);
                            }
                        }
                        Atom::Fixed(_) => unimplemented!(),
                    }
                }
                supported_by_a_line.push(model.reify(and(supported_by_this_line)));
            }
            assert!(model.entails(value)); // tricky to determine the appropriate validity scope, only support enforcing
            model.enforce(or(supported_by_a_line), [presence]);
        }
        ConstraintType::Lt => match constraint.variables.as_slice() {
            &[a, b] => match (a, b) {
                (Atom::Int(a), Atom::Int(b)) => model.bind(lt(a, b), value),
                (Atom::Fixed(a), Atom::Fixed(b)) if a.denom == b.denom => model.bind(f_lt(a, b), value),
                (Atom::Fixed(a), Atom::Int(b)) => {
                    let a = LinearSum::from(a + FAtom::EPSILON);
                    let b = LinearSum::from(b);
                    model.bind(a.leq(b), value);
                }
                (Atom::Int(a), Atom::Fixed(b)) => {
                    let a = LinearSum::from(a);
                    let b = LinearSum::from(b - FAtom::EPSILON);
                    model.bind(a.leq(b), value);
                }
                _ => panic!("Invalid LT operands: {a:?}  {b:?}"),
            },
            x => panic!("Invalid variable pattern for LT constraint: {:?}", x),
        },
        ConstraintType::Leq => match constraint.variables.as_slice() {
            &[a, b] => match (a, b) {
                (Atom::Int(a), Atom::Int(b)) => model.bind(leq(a, b), value),
                (Atom::Fixed(a), Atom::Fixed(b)) if a.denom == b.denom => model.bind(f_leq(a, b), value),
                (Atom::Fixed(a), Atom::Int(b)) => {
                    let a = LinearSum::from(a);
                    let b = LinearSum::from(b);
                    model.bind(a.leq(b), value);
                }
                (Atom::Int(a), Atom::Fixed(b)) => {
                    let a = LinearSum::from(a);
                    let b = LinearSum::from(b);
                    model.bind(a.leq(b), value);
                }
                _ => panic!("Invalid LEQ operands: {a:?}  {b:?}"),
            },
            x => panic!("Invalid variable pattern for LEQ constraint: {:?}", x),
        },
        ConstraintType::Eq => {
            assert_eq!(
                constraint.variables.len(),
                2,
                "Wrong number of parameters to equality constraint: {}",
                constraint.variables.len()
            );
            model.bind(eq(constraint.variables[0], constraint.variables[1]), value);
        }
        ConstraintType::Neq => {
            assert_eq!(
                constraint.variables.len(),
                2,
                "Wrong number of parameters to inequality constraint: {}",
                constraint.variables.len()
            );

            model.bind(neq(constraint.variables[0], constraint.variables[1]), value);
        }
        ConstraintType::Duration(dur) => {
            let build_sum = |s: LinearSum, e: LinearSum, d: &LinearSum| LinearSum::of(vec![-s, e]) - d.clone();

            let start_linear = LinearSum::from(start);
            let end_linear = LinearSum::from(end);

            match dur {
                Duration::Fixed(d) => {
                    let sum = build_sum(start_linear, end_linear, d);
                    model.bind(sum.clone().leq(LinearSum::zero()), value);
                    model.bind(sum.geq(LinearSum::zero()), value);
                }
                Duration::Bounded { lb, ub } => {
                    let lb_sum = build_sum(start_linear.clone(), end_linear.clone(), lb);
                    let ub_sum = build_sum(start_linear, end_linear, ub);
                    model.bind(lb_sum.geq(LinearSum::zero()), value);
                    model.bind(ub_sum.leq(LinearSum::zero()), value);
                }
            };
            // Redundant constraint to enforce the precedence between start and end.
            // This form ensures that the precedence in posted in the STN.
            model.enforce(f_leq(start, end), [presence])
        }
        ConstraintType::Or => {
            let mut disjuncts = Vec::with_capacity(constraint.variables.len());
            for v in &constraint.variables {
                let disjunct: Lit = Lit::try_from(*v).expect("Malformed or constraint");
                disjuncts.push(disjunct);
            }
            model.bind(or(disjuncts), value)
        }
        ConstraintType::LinearEq(sum) => {
            model.enforce(sum.clone().leq(LinearSum::zero()), [presence]);
            model.enforce(sum.clone().geq(LinearSum::zero()), [presence]);
        }
    }
}
