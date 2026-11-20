use crate::ir::{Branch, Expression, SafetyProgram, Scalar, Step};

pub(crate) struct PartialOrder {
    candidates: Vec<(usize, usize)>,
}

impl PartialOrder {
    pub fn prepare(program: &SafetyProgram) -> Self {
        if program.symmetry.is_some()
            || !program.properties.is_empty()
            || !program.fairness.is_empty()
            || !program.constraints.is_empty()
            || !program.action_constraints.is_empty()
            || !program.initial_properties.is_empty()
            || !program.action_properties.is_empty()
        {
            return Self {
                candidates: Vec::new(),
            };
        }
        let candidates = program
            .next
            .iter()
            .enumerate()
            .filter_map(|(index, branch)| {
                let slot = monotone_flag(branch, program.variables.len())?;
                if program
                    .invariants
                    .iter()
                    .any(|predicate| reads(&predicate.expression, slot) != Some(false))
                    || program
                        .next
                        .iter()
                        .enumerate()
                        .any(|(other, branch)| other != index && !independent(branch, slot))
                {
                    return None;
                }
                Some((index, slot))
            })
            .collect();
        Self { candidates }
    }
    pub fn selected(&self, state: &[Scalar]) -> Option<usize> {
        if self
            .candidates
            .iter()
            .any(|(_, slot)| !matches!(state[*slot], Scalar::Bool(_)))
        {
            return None;
        }
        self.candidates
            .iter()
            .find_map(|(branch, slot)| (state[*slot] == Scalar::Bool(false)).then_some(*branch))
    }
}

fn monotone_flag(branch: &Branch, width: usize) -> Option<usize> {
    let guards: Vec<_> = branch
        .steps
        .iter()
        .filter_map(|step| match step {
            Step::Guard { expression } => Some(expression),
            _ => None,
        })
        .collect();
    let [Expression::Not { argument }] = guards.as_slice() else {
        return None;
    };
    let Expression::Read {
        slot: flag,
        next: false,
    } = argument.as_ref()
    else {
        return None;
    };
    let mut assigned = vec![false; width];
    for step in &branch.steps {
        match step {
            Step::Guard { .. } => {}
            Step::Assign { slot, value } => {
                if assigned[*slot] {
                    return None;
                }
                assigned[*slot] = true;
                if *slot == *flag {
                    if !matches!(
                        value,
                        Expression::Literal {
                            value: Scalar::Bool(true)
                        }
                    ) {
                        return None;
                    }
                } else if !matches!(value, Expression::Read { slot: read, next: false } if read == slot)
                {
                    return None;
                }
            }
            _ => return None,
        }
    }
    assigned.iter().all(|assigned| *assigned).then_some(*flag)
}

fn independent(branch: &Branch, flag: usize) -> bool {
    branch.steps.iter().all(|step| match step {
        Step::Assign {
            slot,
            value:
                Expression::Read {
                    slot: read,
                    next: false,
                },
        } if slot == read => true,
        Step::Assign { slot, value } => *slot != flag && reads(value, flag) == Some(false),
        Step::Guard { expression } => reads(expression, flag) == Some(false),
        _ => false,
    })
}

fn reads(expression: &Expression, flag: usize) -> Option<bool> {
    let children = match expression {
        Expression::Literal { .. } | Expression::Data { .. } => return Some(false),
        Expression::Read { slot, next: false } => return Some(*slot == flag),
        Expression::Not { .. }
        | Expression::Neg { .. }
        | Expression::Add { .. }
        | Expression::Sub { .. }
        | Expression::Mul { .. }
        | Expression::Div { .. }
        | Expression::Mod { .. }
        | Expression::Pow { .. }
        | Expression::Eq { .. }
        | Expression::Ne { .. }
        | Expression::Lt { .. }
        | Expression::Le { .. }
        | Expression::Gt { .. }
        | Expression::Ge { .. }
        | Expression::And { .. }
        | Expression::Or { .. }
        | Expression::Implies { .. }
        | Expression::If { .. }
        | Expression::In { .. } => expression.children(),
        _ => return None,
    };
    children.into_iter().try_fold(false, |found, child| {
        reads(child, flag).map(|read| found || read)
    })
}
