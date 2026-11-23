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
    Quantified(Quantifier, Vec<Declaration>, Box<Self>),
}
pub(crate) struct RawScope {
    pub overall: Option<usize>,
    pub entries: Vec<(String, usize, bool)>,
    pub bitwidth: usize,
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
}
