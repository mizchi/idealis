//! Temporary unary sets over the ordinals of a single checked value sort.
use crate::{
    compile::{Environment, Predicate, Term},
    indexed_rows, sequences,
};
use rmodel::{Compare, SetOp};
use std::sync::Arc;

pub(crate) enum Expression {
    Singleton(Term),
    Elements(sequences::Encoding, Box<sequences::Expression>),
    Rows(Arc<indexed_rows::Layout>, Term, Term),
    Set(Operation, Box<Self>, Box<Self>),
    If(Box<Predicate>, Box<Self>, Box<Self>),
}

pub(crate) enum Operation {
    Union,
    Difference,
    Intersection,
}

impl Operation {
    pub(crate) fn from_op(op: SetOp) -> Option<Self> {
        match op {
            SetOp::Union | SetOp::SetUnion => Some(Self::Union),
            SetOp::Difference | SetOp::SetDifference => Some(Self::Difference),
            SetOp::Intersection => Some(Self::Intersection),
            _ => None,
        }
    }
}

impl Expression {
    pub(crate) fn reads_state(&self) -> bool {
        match self {
            Self::Singleton(value) => value.reads_state(),
            Self::Elements(_, seq) => seq.reads_state(),
            Self::Rows(_, rows, keys) => rows.reads_state() || keys.reads_state(),
            Self::Set(_, left, right) => left.reads_state() || right.reads_state(),
            Self::If(test, yes, no) => test.reads_state() || yes.reads_state() || no.reads_state(),
        }
    }

    /// Ascending, unique codes. Zero is the absent singleton, never an element.
    pub(crate) fn evaluate(&self, env: Environment<'_>) -> Vec<u64> {
        match self {
            Self::Singleton(value) => {
                let code = value.evaluate(env);
                (code != 0).then_some(code).into_iter().collect()
            }
            Self::Elements(layout, seq) => {
                let mut values = seq.evaluate(env, layout);
                values.sort_unstable();
                values.dedup();
                values
            }
            Self::Rows(layout, rows, keys) => {
                let keys = keys.evaluate(env);
                let mut values: Vec<_> = layout
                    .values(rows.evaluate(env))
                    .filter_map(|(key, value)| (key & keys != 0).then_some(value))
                    .collect();
                values.sort_unstable();
                values.dedup();
                values
            }
            Self::Set(op, left, right) => {
                let mut left = left.evaluate(env);
                let right = right.evaluate(env);
                match op {
                    Operation::Union => {
                        left.extend(right);
                        left.sort_unstable();
                        left.dedup();
                    }
                    Operation::Difference => left.retain(|code| right.binary_search(code).is_err()),
                    Operation::Intersection => {
                        left.retain(|code| right.binary_search(code).is_ok())
                    }
                }
                left
            }
            Self::If(test, yes, no) => {
                let branch = if test.evaluate(env) { yes } else { no };
                branch.evaluate(env)
            }
        }
    }

    pub(crate) fn compare(&self, op: Compare, right: &Self, env: Environment<'_>) -> bool {
        let left = self.evaluate(env);
        let right = right.evaluate(env);
        match op {
            Compare::Equal => left == right,
            Compare::NotEqual => left != right,
            Compare::In => left.iter().all(|code| right.binary_search(code).is_ok()),
            Compare::NotIn => left.iter().any(|code| right.binary_search(code).is_err()),
        }
    }
}
