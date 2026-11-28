//! Syntax, source locations and checked models for the Idealis language.
mod numeric;
mod refinement;
mod structured;
mod syntax;

pub use numeric::validate as validate_integer_bounds;
pub use refinement::{Refinement, RefinementOperand};
use std::fmt;
pub use structured::{Case, Sequence, ValueDomain, ValueType, Variant};
pub use syntax::vocabulary::{
    LocalizedText, Vocabulary, VocabularyRole, VocabularyTerm, parse_vocabulary,
};
pub use syntax::{
    Action, Arith, BOOL, Check, CheckKind, Compare, DataKind, Expr, Fairness, FairnessKind,
    Formula, INT, Int, IntCompare, Model, Mult, NAT, Phase, Quantifier, Record, Relation, Scope,
    SetOp, Shape, TemporalCheck, TransitionCheck,
};
pub use syntax::{atoms as atom_count, transition_predicate, valid_term_id};

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
    /// Create a type or model diagnostic.
    pub fn kind(message: impl Into<String>) -> Self {
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

/// A source diagnostic. A location is a zero-based Unicode character offset.
/// Type errors can point to their enclosing declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseDiagnostic {
    pub error: Error,
    pub offset: Option<usize>,
}

impl fmt::Display for ParseDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for ParseDiagnostic {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Parses and checks a model.
pub fn parse(source: &str) -> Result<Model, Error> {
    parse_diagnostic(source).map_err(|diagnostic| diagnostic.error)
}

/// A model whose declarations and expressions have passed type checking.
#[derive(Clone, Debug)]
pub struct CheckedModel(Model);

impl CheckedModel {
    pub fn into_model(self) -> Model {
        self.0
    }
}

impl std::ops::Deref for CheckedModel {
    type Target = Model;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Parses and checks a model, retaining the source location of an error when available.
pub fn parse_diagnostic(source: &str) -> Result<Model, ParseDiagnostic> {
    syntax::parse(source, false).map(|located| located.model)
}

/// A checked model with source offsets in characters.
pub struct LocatedModel<M = Model> {
    pub model: M,
    pub declarations: std::collections::BTreeMap<String, usize>,
    pub term_locations: std::collections::BTreeMap<String, usize>,
    /// Mutable state read by each declaration, including expanded derived expressions.
    pub state_reads: std::collections::BTreeMap<usize, std::collections::BTreeSet<String>>,
    /// Source-level update groups and value branches, in source order per action.
    pub update_branches: std::collections::BTreeMap<String, Vec<UpdateBranch>>,
}

/// One source-level `if` in an update, located before lowering.
pub struct UpdateBranch {
    pub offset: usize,
    assignee: String,
    ancestors: Vec<bool>,
    sites: Vec<(String, Vec<usize>)>,
    binding: bool,
    value_conditional: bool,
    unreachable_reasons: [Option<String>; 2],
}

impl UpdateBranch {
    /// Assignment and enclosing arm choices used to find the condition after lowering.
    pub fn selector(&self) -> (&str, &[bool]) {
        (&self.assignee, &self.ancestors)
    }

    /// Restore a selector carried alongside the lowered model. The search
    /// resolves it against the checked action before evaluating it.
    pub fn from_selector(offset: usize, assignee: String, ancestors: Vec<bool>) -> Self {
        Self {
            offset,
            assignee,
            ancestors,
            sites: Vec::new(),
            binding: false,
            value_conditional: false,
            unreachable_reasons: [None, None],
        }
    }

    pub fn is_value_conditional(&self) -> bool {
        self.value_conditional
    }

    /// Assignment and child paths for a conditional inside an expression.
    pub fn value_sites(&self) -> &[(String, Vec<usize>)] {
        &self.sites
    }

    pub fn binding_name(&self) -> Option<&str> {
        self.binding.then_some(&self.assignee)
    }

    pub fn with_value_sites(mut self, sites: Vec<(String, Vec<usize>)>, binding: bool) -> Self {
        self.sites = sites;
        self.binding = binding;
        self
    }

    pub fn with_value_conditional(mut self, value_conditional: bool) -> Self {
        self.value_conditional = value_conditional;
        self
    }

    /// Coverage expectations for the then and else arms, in source order.
    pub fn unreachable_reasons(&self) -> &[Option<String>; 2] {
        &self.unreachable_reasons
    }

    pub fn with_unreachable_reasons(mut self, reasons: [Option<String>; 2]) -> Self {
        self.unreachable_reasons = reasons;
        self
    }
}

pub fn parse_located(source: &str) -> Result<LocatedModel, ParseDiagnostic> {
    syntax::parse(source, true)
}

/// Retain locations and the type-checking boundary for a source model.
pub fn parse_located_checked(source: &str) -> Result<LocatedModel<CheckedModel>, ParseDiagnostic> {
    let LocatedModel {
        model,
        declarations,
        term_locations,
        state_reads,
        update_branches,
    } = parse_located(source)?;
    Ok(LocatedModel {
        model: CheckedModel(model),
        declarations,
        term_locations,
        state_reads,
        update_branches,
    })
}

/// The byte spans of unquoted `@name` references in component source.
pub fn component_slots(
    source: &str,
) -> Result<Vec<(std::ops::Range<usize>, String)>, ParseDiagnostic> {
    syntax::component_slots(source)
}

/// Type-checks a syntax tree that did not come from the parser, such as
/// one read from the JSON IR (plan 0108), and returns it ready for the
/// checkers (arithmetic rewritten, Bool added when mentioned).
pub fn validate(model: Model) -> Result<Model, Error> {
    syntax::validate(model)
}

/// Check a model constructed outside the source parser.
pub fn validate_checked(model: Model) -> Result<CheckedModel, Error> {
    validate(model).map(CheckedModel)
}

/// The name of the built-in check that no transition breaks a var's
/// multiplicity (plan 0099).
pub const DECLARATIONS: &str = "Declarations";
/// The built-in check that every state reached keeps its integers within
/// the scope's `Int = lo..hi` (plan 0103). When it fails, the search is not
/// complete and a check without a witness is unknown.
pub const SCOPE: &str = "Scope";
