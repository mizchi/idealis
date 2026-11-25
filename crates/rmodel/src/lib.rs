//! One model of structure (static relations), state (`var` relations) and
//! behavior (actions with an implicit frame), lowered independently to
//! Alloy 6 for ralloy's SAT and to TLA+ for rtlc (plan 0091).
//!
//! The declarations and check names are stable identifiers that a Pkl
//! specification links its rules to (plan 0092).
pub mod bindings;
mod emit;
mod numeric;
mod print;
pub mod reference;
mod structured;
mod syntax;
pub use structured::{Case, Sequence, Variant};

pub use emit::{DECLARATIONS, SCOPE, alloy_label, alloy_supports, tla_supports, to_alloy, to_tla};
pub use print::quint_like;
use std::fmt;
pub use syntax::{
    Action, Arith, BOOL, Check, CheckKind, Compare, Expr, Fairness, FairnessKind, Formula, INT,
    Int, IntCompare, Model, Mult, NAT, Phase, Quantifier, Record, Relation, Scope, SetOp, Shape,
    TemporalCheck, TransitionCheck,
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

/// Type-checks a syntax tree that did not come from the parser, such as
/// one read from the JSON IR (plan 0108), and returns it ready for the
/// checkers (arithmetic rewritten, Bool added when mentioned).
pub fn validate(model: Model) -> Result<Model, Error> {
    syntax::validate(model)
}
