use crate::Kind;
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mult {
    Set,
    One,
    Lone,
    Some,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Parent {
    /// `extends`: a disjoint part of the parent's atoms.
    Extends,
    /// `in`: any subset of the parent's atoms.
    In,
}
pub(crate) struct Signature {
    pub name: String,
    pub mult: Mult,
    pub abstract_: bool,
    pub parent: Option<(Parent, String)>,
}
/// `name: mult A` declares a binary field; `name: A m -> n B` and
/// `name: A -> B -> C` declare fields of arity three and four (plan 0079).
pub(crate) struct Field {
    pub name: String,
    pub owner: String,
    /// Signatures of the columns after the owner.
    pub columns: Vec<String>,
    /// Multiplicity of a single-column field.
    pub mult: Mult,
    /// Multiplicities around the arrow of a two-column field.
    pub arrow: Option<(Mult, Mult)>,
    /// `var`: the field changes from state to state (plan 0090).
    pub var: bool,
}
#[derive(Clone, Copy)]
pub(crate) enum Unary {
    Not,
    No,
    Some,
    One,
    Lone,
    Transpose,
    Closure,
    Reflexive,
    /// `#e`, an integer (plan 0082).
    Cardinality,
    /// Temporal formulas over a lasso trace and the postfix prime `e'`,
    /// the value of `e` in the next state (plan 0090).
    Always,
    Eventually,
    After,
    Prime,
}
#[derive(Clone, Copy)]
pub(crate) enum Binary {
    And,
    Or,
    Implies,
    Iff,
    Equal,
    NotEqual,
    In,
    NotIn,
    Union,
    Difference,
    Intersection,
    Product,
    Join,
    Domain,
    Range,
    /// Integer comparisons (plan 0082).
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}
#[derive(Clone, Copy)]
pub(crate) enum Quantifier {
    All,
    No,
    Some,
    One,
    Lone,
}
#[derive(Clone)]
pub(crate) struct Declaration {
    pub names: Vec<String>,
    pub domain: Expr,
    pub disjoint: bool,
}
#[derive(Clone)]
pub(crate) enum Expr {
    Name(String),
    /// `name[arguments]`: a pred/fun call, resolved after parsing; other
    /// names become box joins (plan 0081).
    Apply(String, Vec<Self>),
    /// An integer literal (plan 0082).
    Number(i64),
    Block(Vec<Self>),
    Unary(Unary, Box<Self>),
    Binary(Binary, Box<Self>, Box<Self>),
    Conditional(Box<Self>, Box<Self>, Box<Self>),
    Quantified(Quantifier, Vec<Declaration>, Box<Self>),
}
pub(crate) struct RawScope {
    pub overall: Option<usize>,
    pub entries: Vec<(String, usize, bool)>,
    pub bitwidth: usize,
    /// `N steps`: the trace length (plan 0090).
    pub steps: Option<usize>,
}
pub(crate) struct RawCommand {
    pub label: String,
    pub kind: Kind,
    pub body: Expr,
    pub scope: RawScope,
}
/// A pred (formula body) or fun (relation body) with its parameters, each
/// of the default `one` multiplicity.
pub(crate) struct Predicate {
    pub params: Vec<Declaration>,
    pub body: Expr,
    pub function: bool,
}
/// Whether an expression uses a temporal operator.
pub(crate) fn uses_temporal(expression: &Expr) -> bool {
    match expression {
        Expr::Name(_) | Expr::Number(_) => false,
        Expr::Apply(_, arguments) => arguments.iter().any(uses_temporal),
        Expr::Block(expressions) => expressions.iter().any(uses_temporal),
        Expr::Unary(operator, child) => {
            matches!(
                operator,
                Unary::Always | Unary::Eventually | Unary::After | Unary::Prime
            ) || uses_temporal(child)
        }
        Expr::Binary(_, left, right) => uses_temporal(left) || uses_temporal(right),
        Expr::Conditional(test, yes, no) => {
            [test, yes, no].into_iter().any(|part| uses_temporal(part))
        }
        Expr::Quantified(_, declarations, body) => {
            declarations
                .iter()
                .any(|declaration| uses_temporal(&declaration.domain))
                || uses_temporal(body)
        }
    }
}
impl Module {
    /// Temporal for every command: a var field, or a temporal fact or
    /// predicate (plan 0090).
    fn temporal_everywhere(&self) -> bool {
        self.fields.iter().any(|field| field.var)
            || self.facts.iter().any(uses_temporal)
            || self.predicates.values().any(|predicate| {
                uses_temporal(&predicate.body)
                    || predicate
                        .params
                        .iter()
                        .any(|param| uses_temporal(&param.domain))
            })
    }
    /// Whether the command with this (resolved) body is temporal; a static
    /// command beside a temporal one stays static (review of #102).
    pub fn temporal_for(&self, body: &Expr) -> bool {
        self.temporal_everywhere() || uses_temporal(body)
    }
}
impl Predicate {
    pub fn arity(&self) -> usize {
        self.params.iter().map(|param| param.names.len()).sum()
    }
}
#[derive(Default)]
pub(crate) struct Module {
    pub signatures: Vec<Signature>,
    pub fields: Vec<Field>,
    pub facts: Vec<Expr>,
    /// Bodies of signature facts until `parse` turns them into facts.
    pub signature_facts: Vec<(String, Expr)>,
    /// Preds and funs by name.
    pub predicates: BTreeMap<String, Predicate>,
    pub assertions: BTreeMap<String, Expr>,
    pub commands: Vec<RawCommand>,
    /// Signature indices, parents first; set once by `Model::parse`.
    pub hierarchy: Vec<usize>,
}
