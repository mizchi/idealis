//! One model of structure (static relations), state (`var` relations) and
//! behavior (actions with an implicit frame), lowered independently to
//! Alloy 6 for ralloy's SAT and to TLA+ for rtlc (plan 0091).
//!
//! The declarations and check names are stable identifiers that a Pkl
//! specification links its rules to (plan 0092).
mod emit;
pub mod reference;
mod syntax;

pub use emit::{alloy_label, to_alloy, to_tla};
use std::fmt;
pub use syntax::{
    Action, Check, CheckKind, Compare, Expr, Formula, Int, IntCompare, Model, Mult, Quantifier,
    Relation, Scope, SetOp, Shape,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Syntax,
    Type,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}
impl Error {
    pub(crate) fn syntax(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Syntax,
            message: message.into(),
        }
    }
    pub(crate) fn kind(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Type,
            message: message.into(),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for Error {}

/// Parses and checks a model.
pub fn parse(source: &str) -> Result<Model, Error> {
    syntax::parse(source)
}
