//! Compatibility API for Idealis model parsing, evaluation and adapters.

pub mod bindings;
mod emit;
mod print;

pub use emit::{
    GroundAction, TemporalWitnessTla, alloy_label, alloy_steps, alloy_supports,
    alloy_transition_supports, tla_supports, to_alloy, to_tla, to_tla_temporal_witness,
};
pub use idealis_semantics::reference;
pub use idealis_syntax::{
    Action, Arith, BOOL, Case, Check, CheckKind, CheckedModel, Compare, DECLARATIONS, DataKind,
    Error, ErrorKind, Expr, Fairness, FairnessKind, Formula, INT, Int, IntCompare, LocalizedText,
    LocatedModel, Model, Mult, NAT, ParseDiagnostic, Phase, Quantifier, Record, Refinement,
    RefinementOperand, Relation, SCOPE, Scope, Sequence, SetOp, Shape, TemporalCheck,
    TransitionCheck, UpdateBranch, ValueType, Variant, Vocabulary, VocabularyRole, VocabularyTerm,
    component_slots, parse, parse_diagnostic, parse_located, parse_located_checked,
    parse_vocabulary, valid_term_id, validate, validate_checked,
};
pub use print::quint_like;
