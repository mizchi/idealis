//! The model language (plan 0091): tokens, syntax tree, parser and the
//! type and form checks every lowering relies on.
use crate::{Case, Error, ParseDiagnostic, Sequence, Variant};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mult {
    Set,
    Lone,
    One,
    Some,
}
/// A relation's shape: a set of atoms of one sig, or a binary relation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    Unary {
        sig: String,
        mult: Mult,
    },
    Binary {
        from: String,
        to: String,
        mult: Mult,
    },
}
#[derive(Clone, Debug)]
pub struct Relation {
    pub name: String,
    /// `var`: part of the state; otherwise part of the configuration.
    pub var: bool,
    pub shape: Shape,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetOp {
    Union,
    SetUnion,
    Override,
    WithoutKeys,
    Difference,
    SetDifference,
    Intersection,
    Product,
    Join,
    /// `*`: only between integers; the checker rewrites it to arithmetic.
    Multiply,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arith {
    Add,
    Sub,
    Mul,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Before,
    After,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    /// A sig, a relation, or a bound atom variable.
    Name(String),
    None,
    Binary(SetOp, Box<Expr>, Box<Expr>),
    If(Box<Formula>, Box<Expr>, Box<Expr>),
    /// The atoms of a finite sort that satisfy a predicate.
    Select(String, String, Box<Formula>),
    /// An integer literal, the set of one integer (plan 0103).
    Literal(i64),
    /// `#e`, the set of one integer.
    Count(Box<Expr>),
    /// Arithmetic on two expressions of one integer each. The parser reads
    /// `+` and `-` as set operations; the checker rewrites them when both
    /// sides are integers.
    Arith(Arith, Box<Expr>, Box<Expr>),
    /// `Money { amount: e, currency: c }`, the set of one record, fields in
    /// declaration order.
    Record(String, Vec<Expr>),
    /// A record field resolved from the receiver type by the checker.
    Field(Box<Expr>, String),
    Sequence(String, Vec<Expr>),
    Append(Box<Expr>, Box<Expr>),
    Tail(Box<Expr>),
    Take(Box<Expr>, Box<Expr>),
    RemoveAt(Box<Expr>, Box<Expr>),
    RemoveAll(Box<Expr>, Box<Expr>),
    ReachableFrom(Box<Expr>, Box<Expr>),
    /// The set of first components of a binary relation.
    Keys(Box<Expr>),
    /// Ordered pairs connected by paths of positive length.
    Closure(Box<Expr>),
    Length(Box<Expr>),
    Elements(Box<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Variant(String, Option<Box<Expr>>),
    Payload(Box<Expr>, String),
    At(Phase, Box<Expr>),
}

impl Expr {
    /// Value operands evaluated in the current binding scope. Conditions and
    /// comprehension predicates have their own control flow.
    pub fn coverage_child(&self, index: usize) -> Option<&Self> {
        match (self, index) {
            (Self::Record(_, values) | Self::Sequence(_, values), index) => values.get(index),
            (
                Self::Binary(_, left, _)
                | Self::Arith(_, left, _)
                | Self::If(_, left, _)
                | Self::Append(left, _)
                | Self::Take(left, _)
                | Self::RemoveAt(left, _)
                | Self::RemoveAll(left, _)
                | Self::ReachableFrom(left, _)
                | Self::Index(left, _)
                | Self::Field(left, _)
                | Self::Count(left)
                | Self::Tail(left)
                | Self::Keys(left)
                | Self::Closure(left)
                | Self::Length(left)
                | Self::Elements(left)
                | Self::Variant(_, Some(left))
                | Self::Payload(left, _),
                0,
            ) => Some(left),
            (
                Self::Binary(_, _, right)
                | Self::Arith(_, _, right)
                | Self::If(_, _, right)
                | Self::Append(_, right)
                | Self::Take(_, right)
                | Self::RemoveAt(_, right)
                | Self::RemoveAll(_, right)
                | Self::ReachableFrom(_, right)
                | Self::Index(_, right),
                1,
            ) => Some(right),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compare {
    In,
    NotIn,
    Equal,
    NotEqual,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntCompare {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Int {
    Count(Expr),
    Literal(i64),
    /// An expression of one integer (plan 0103).
    Value(Expr),
    /// A parsed value operand with its source offset, removed by type checking.
    #[doc(hidden)]
    LocatedValue(Expr, usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantifier {
    All,
    Some,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Formula {
    Compare(Compare, Expr, Expr),
    Ints(IntCompare, Int, Int),
    Mult(Mult, Expr),
    /// `no e`.
    Empty(Expr),
    HasTag(Expr, String),
    Not(Box<Formula>),
    And(Vec<Formula>),
    Or(Box<Formula>, Box<Formula>),
    Implies(Box<Formula>, Box<Formula>),
    Quantified(Quantifier, String, String, Box<Formula>),
}
#[derive(Clone, Debug)]
pub struct Action {
    pub name: String,
    /// Parameters and their sigs.
    pub params: Vec<(String, String)>,
    pub guards: Vec<Formula>,
    /// Simultaneous updates, at most one per var.
    pub assignments: Vec<(String, Expr)>,
}

impl Action {
    /// Keep the guard's evaluation order while separating the clauses that
    /// can run before action arguments are bound.
    pub fn guard_parts(&self) -> (Vec<&Formula>, Vec<&Formula>) {
        if self.params.is_empty() {
            return (Vec::new(), self.guards.iter().collect());
        }

        fn conjuncts<'a>(formula: &'a Formula, clauses: &mut Vec<&'a Formula>) {
            match formula {
                Formula::And(children) => {
                    for child in children {
                        conjuncts(child, clauses);
                    }
                }
                _ => clauses.push(formula),
            }
        }

        let arguments = self.params.iter().map(|(name, _)| name.as_str()).collect();
        let mut clauses = Vec::new();
        for guard in &self.guards {
            conjuncts(guard, &mut clauses);
        }
        let prefix = clauses
            .iter()
            .take_while(|clause| !may_read_names(clause, &arguments))
            .count();
        let suffix = clauses.split_off(prefix);
        (clauses, suffix)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckKind {
    /// Holds in every reachable state.
    Invariant,
    /// Holds in some reachable state.
    Reachable,
}
#[derive(Clone, Debug)]
pub struct Check {
    pub name: String,
    pub kind: CheckKind,
    pub formula: Formula,
}
#[derive(Clone, Debug)]
pub struct TransitionCheck {
    pub name: String,
    pub action: Option<String>,
    pub formula: Formula,
}

/// A temporal obligation over fair executions of the initial transition system.
#[derive(Clone, Debug)]
pub struct TemporalCheck {
    pub name: String,
    pub formula: Formula,
    pub params: Vec<(String, String)>,
    pub trigger: Option<Formula>,
    /// An override for this property; None uses the model defaults.
    pub fairness: Option<Vec<Fairness>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FairnessKind {
    Weak,
    Strong,
}

#[derive(Clone, Debug)]
pub struct Fairness {
    pub kind: FairnessKind,
    pub action: String,
    /// Require fairness for every argument tuple, rather than their union.
    pub each: bool,
}

/// `record Money { amount: Int, currency: Currency }`: a value compared by
/// its fields (plan 0103).
#[derive(Clone, Debug)]
pub struct Record {
    pub name: String,
    /// Field names and their sorts.
    pub fields: Vec<(String, String)>,
    pub data: Option<DataKind>,
    pub refinements: Vec<crate::Refinement>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataKind {
    Newtype,
    Record,
}

impl Record {
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|(field, _)| field == name)
    }
}
#[derive(Clone, Debug)]
pub struct Scope {
    /// Exactly this many atoms per sig.
    pub sizes: BTreeMap<String, usize>,
    /// `Int = lo..hi`: the integers enumerated for initial states,
    /// parameters and records. Values are mathematical; a state with an
    /// integer outside leaves the scope (plan 0103).
    pub ints: Option<(i64, i64)>,
    /// An optional legacy override of the SAT engine's trace length.
    pub steps: Option<usize>,
}
#[derive(Clone, Debug)]
pub struct Model {
    pub name: String,
    pub sigs: Vec<String>,
    /// Glossary text keyed by the entity's checked sort name.
    pub entity_definitions: BTreeMap<String, String>,
    /// Translated glossary text keyed by sort name and normalized language tag.
    pub entity_translations: BTreeMap<String, BTreeMap<String, String>>,
    /// External vocabulary ID keyed by the declaration that gives it a formal meaning.
    pub term_bindings: BTreeMap<String, String>,
    /// `enum Name = A | B`: named atoms, one each, with no scope (0093).
    pub enums: Vec<(String, Vec<String>)>,
    /// `type Party = Person + Org`: a union of signatures and enums with no
    /// atoms of its own (plan 0100).
    pub sums: Vec<(String, Vec<String>)>,
    pub records: Vec<Record>,
    pub sequences: Vec<Sequence>,
    pub variants: Vec<Variant>,
    pub relations: Vec<Relation>,
    /// Named, single-state expressions. Uses inside the kernel are expanded.
    pub derivations: Vec<(String, Expr)>,
    pub facts: Vec<Formula>,
    pub init: Vec<Formula>,
    pub actions: Vec<Action>,
    pub checks: Vec<Check>,
    pub temporal: Vec<TemporalCheck>,
    pub transitions: Vec<TransitionCheck>,
    pub fairness: Vec<Fairness>,
    pub scope: Scope,
}
impl Model {
    pub fn relation(&self, name: &str) -> Option<&Relation> {
        self.relations.iter().find(|relation| relation.name == name)
    }
    pub fn term_target_kind(&self, name: &str) -> Option<&'static str> {
        if let Some(relation) = self.relation(name) {
            Some(if relation.var { "state" } else { "relation" })
        } else if self.sigs.iter().any(|sort| sort == name)
            || self.enums.iter().any(|(sort, _)| sort == name)
        {
            Some("entity")
        } else if self.actions.iter().any(|action| action.name == name) {
            Some("action")
        } else if self.checks.iter().any(|check| check.name == name)
            || self.transitions.iter().any(|check| check.name == name)
            || self.temporal.iter().any(|check| check.name == name)
        {
            Some("check")
        } else {
            None
        }
    }
    /// The enum a member name belongs to.
    pub fn member(&self, name: &str) -> Option<&str> {
        self.enums
            .iter()
            .find(|(_, members)| members.iter().any(|member| member == name))
            .map(|(enumeration, _)| enumeration.as_str())
    }
    /// The sum sort a signature or enum belongs to.
    pub fn parent(&self, sort: &str) -> Option<&str> {
        self.sums
            .iter()
            .find(|(_, children)| children.iter().any(|child| child == sort))
            .map(|(sum, _)| sum.as_str())
    }
    /// Whether values of sort `a` are values of sort `b`.
    pub fn subsort(&self, a: &str, b: &str) -> bool {
        a == b || (a == NAT && b == INT) || self.parent(a) == Some(b)
    }
    /// The smallest sort containing both, if any.
    pub fn join_sorts(&self, a: &str, b: &str) -> Option<String> {
        if self.subsort(a, b) {
            Some(b.to_owned())
        } else if self.subsort(b, a) {
            Some(a.to_owned())
        } else {
            match (self.parent(a), self.parent(b)) {
                (Some(x), Some(y)) if x == y => Some(x.to_owned()),
                _ => None,
            }
        }
    }
    pub fn has_refinements(&self) -> bool {
        self.records
            .iter()
            .any(|record| !record.refinements.is_empty())
    }

    pub fn record(&self, name: &str) -> Option<&Record> {
        self.records.iter().find(|record| record.name == name)
    }
    /// The first declared record with this field, and its position there.
    pub fn field(&self, name: &str) -> Option<(&Record, usize)> {
        self.records
            .iter()
            .find_map(|record| record.field_index(name).map(|index| (record, index)))
    }
    /// Values are integers and records; atoms are everything else.
    pub fn is_value(&self, sort: &str) -> bool {
        numeric_sort(sort)
            || self.record(sort).is_some()
            || self.sequence(sort).is_some()
            || self.variant(sort).is_some()
    }
    /// Whether the model has integer or record values (plan 0103).
    pub fn has_values(&self) -> bool {
        self.scope.ints.is_some() || !self.records.is_empty() || self.has_structures()
    }
    pub fn uses_nat(&self) -> bool {
        self.declared_sorts().any(|sort| sort == NAT)
    }
    fn declared_sorts(&self) -> impl Iterator<Item = &str> {
        self.records
            .iter()
            .flat_map(|record| record.fields.iter().map(|(_, sort)| sort.as_str()))
            .chain(self.sequences.iter().map(|s| s.element.as_str()))
            .chain(
                self.variants
                    .iter()
                    .flat_map(|v| v.cases.iter().filter_map(|c| c.payload.as_deref())),
            )
            .chain(self.relations.iter().map(|relation| match &relation.shape {
                Shape::Unary { sig, .. } => sig.as_str(),
                Shape::Binary { to, .. } => to.as_str(),
            }))
            .chain(
                self.actions
                    .iter()
                    .flat_map(|action| action.params.iter().map(|(_, sort)| sort.as_str())),
            )
    }
    pub fn is_sum(&self, name: &str) -> bool {
        self.sums.iter().any(|(sum, _)| sum == name)
    }
    pub fn is_enum(&self, name: &str) -> bool {
        self.enums
            .iter()
            .any(|(enumeration, _)| enumeration == name)
    }
    pub fn vars(&self) -> impl Iterator<Item = &Relation> {
        self.relations.iter().filter(|relation| relation.var)
    }
}

/// The built-in sorts (plan 0103). Bool is an enum of `true` and `false`,
/// added when the model mentions it.
pub const INT: &str = "Int";
pub const NAT: &str = "Nat";

pub(crate) fn numeric_sort(sort: &str) -> bool {
    matches!(sort, INT | NAT)
}

pub const BOOL: &str = "Bool";
const KEYWORDS: &[&str] = &[
    "model",
    "record",
    "true",
    "false",
    "enum",
    "type",
    "sig",
    "rel",
    "var",
    "fact",
    "init",
    "action",
    "invariant",
    "reachable",
    "scope",
    "steps",
    "set",
    "lone",
    "one",
    "some",
    "no",
    "all",
    "not",
    "and",
    "or",
    "implies",
    "iff",
    "in",
    "none",
];
const MAX_SCOPE: usize = 7;

fn valid_int_scope(low: i64, high: i64) -> bool {
    high.checked_sub(low)
        .is_some_and(|width| (0..20).contains(&width))
}
const TRIPLE_QUOTE: &[char] = &['"', '"', '"'];
// No ordinary token can begin with a newline. Keep block strings distinct
// from quoted reasons and the legacy one-line entity syntax.
const BLOCK_STRING_TOKEN: char = '\n';

fn definition_token(characters: &[char], position: &mut usize) -> Result<String, Error> {
    if characters[*position..].starts_with(TRIPLE_QUOTE) {
        *position += TRIPLE_QUOTE.len();
        let mut token = String::from(BLOCK_STRING_TOKEN);
        while let Some(&character) = characters.get(*position) {
            if characters[*position..].starts_with(TRIPLE_QUOTE) {
                *position += TRIPLE_QUOTE.len();
                return Ok(token);
            }
            *position += 1;
            if character == '\r' && characters.get(*position) == Some(&'\n') {
                continue;
            }
            token.push(character);
        }
        return Err(Error::syntax("unterminated triple-quoted description"));
    }
    *position += 1;
    // The prefix distinguishes a definition token from syntax and names.
    let mut token = String::from("\"");
    while let Some(&character) = characters.get(*position) {
        *position += 1;
        match character {
            '"' => return Ok(token),
            '\\' => {
                let Some(&escaped @ ('"' | '\\')) = characters.get(*position) else {
                    return Err(Error::syntax("invalid entity definition escape"));
                };
                token.push(escaped);
                *position += 1;
            }
            c if !definition_character(c) => {
                return Err(Error::syntax("entity definitions must be on one line"));
            }
            c => token.push(c),
        }
    }
    Err(Error::syntax("unterminated entity definition"))
}

fn definition_character(c: char) -> bool {
    !c.is_control() && !matches!(c, '\u{2028}' | '\u{2029}')
}

fn valid_definition(text: &str) -> bool {
    !text.trim().is_empty()
        && text
            .chars()
            .all(|c| c == '\n' || c == '\t' || definition_character(c))
}

fn valid_language_tag(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let first = parts.next().unwrap_or_default();
    (1..=8).contains(&first.len())
        && first.bytes().all(|c| c.is_ascii_lowercase())
        && parts.all(|part| {
            (1..=8).contains(&part.len())
                && part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// IDs shared by `.ideal` terms and the specification vocabulary.
pub fn valid_term_id(id: &str) -> bool {
    id.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && id.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

fn tokens(source: &str) -> Result<(Vec<String>, Vec<usize>), ParseDiagnostic> {
    lex(source, false)
}

pub(crate) fn component_slots(
    source: &str,
) -> Result<Vec<(std::ops::Range<usize>, String)>, ParseDiagnostic> {
    let (tokens, offsets) = lex(source, true)?;
    let bytes: Vec<_> = source.char_indices().map(|(byte, _)| byte).collect();
    Ok(tokens
        .into_iter()
        .zip(offsets)
        .filter_map(|(token, start)| {
            let name = token.strip_prefix('@')?;
            let byte = bytes[start];
            Some((byte..byte + token.len(), name.to_owned()))
        })
        .collect())
}

fn lex(source: &str, component: bool) -> Result<(Vec<String>, Vec<usize>), ParseDiagnostic> {
    let mut tokens = Vec::new();
    let mut offsets = Vec::new();
    let characters: Vec<char> = source.chars().collect();
    let mut position = 0;
    while position < characters.len() {
        let character = characters[position];
        if character.is_whitespace() {
            position += 1;
            continue;
        }
        if character == '/' && characters.get(position + 1) == Some(&'/') {
            while position < characters.len() && characters[position] != '\n' {
                position += 1;
            }
            continue;
        }
        let start = position;
        offsets.push(start);
        if character == '"' {
            tokens.push(
                definition_token(&characters, &mut position).map_err(|error| ParseDiagnostic {
                    error,
                    offset: Some(start),
                })?,
            );
            continue;
        }
        let slot = component
            && character == '@'
            && characters
                .get(position + 1)
                .is_some_and(|c| c.is_ascii_alphabetic() || *c == '_');
        if slot {
            position += 1;
        }
        if slot || character.is_ascii_alphanumeric() || character == '_' {
            while position < characters.len()
                && (characters[position].is_ascii_alphanumeric() || characters[position] == '_')
            {
                position += 1;
            }
        } else if let Some(operator) = ["->", "!=", "<=", ">=", "++"].into_iter().find(|operator| {
            characters[position..].starts_with(&operator.chars().collect::<Vec<_>>())
        }) {
            position += operator.len();
        } else if "{}[]():,.+-*&#=<>'|".contains(character) {
            position += 1;
        } else {
            return Err(ParseDiagnostic {
                error: Error::syntax(format!("unexpected character {character:?}")),
                offset: Some(start),
            });
        }
        tokens.push(characters[start..position].iter().collect());
    }
    Ok((tokens, offsets))
}

#[derive(Default)]
struct Locations {
    named: BTreeMap<String, usize>,
    terms: BTreeMap<String, usize>,
    facts: Vec<usize>,
    init: Vec<usize>,
    scope: Option<usize>,
    bindings: BTreeMap<String, LocalDefinitions>,
    update_branches: BTreeMap<String, Vec<crate::UpdateBranch>>,
    binding_branches: BTreeMap<String, BTreeMap<String, Vec<BindingBranch>>>,
    // The current source declaration, if checking a model read from text.
    current: Cell<Option<usize>>,
    operand_error: Cell<Option<usize>>,
}

struct LocalDefinitions {
    definitions: Vec<(String, Expr)>,
    remaining: usize,
}

struct BindingBranch {
    branch: crate::UpdateBranch,
    path: Vec<usize>,
}

fn record_binding_sites<'a>(
    mut source: &'a Expr,
    checked: &Expr,
    definitions: &BTreeMap<&str, &'a Expr>,
    bindings: &mut BTreeMap<String, Vec<BindingBranch>>,
    assignment: &str,
    path: &mut Vec<usize>,
) {
    // A source binding may be an alias of an earlier binding. Resolve
    // aliases at the same position without recursing through the chain.
    while let Expr::Name(name) = source {
        let Some(value) = definitions.get(name.as_str()) else {
            return;
        };
        if let Some(branches) = bindings.get_mut(name) {
            for entry in branches {
                let mut site = path.clone();
                site.extend(&entry.path);
                entry.branch.sites.push((assignment.to_owned(), site));
            }
        }
        source = value;
    }
    for (index, (source, checked)) in (0..)
        .map_while(|index| {
            source
                .coverage_child(index)
                .zip(checked.coverage_child(index))
        })
        .enumerate()
    {
        path.push(index);
        record_binding_sites(source, checked, definitions, bindings, assignment, path);
        path.pop();
    }
}

struct ValueBranches<'a> {
    assignee: &'a str,
    path: Vec<usize>,
    embedded: bool,
    branches: &'a mut Vec<crate::UpdateBranch>,
    allow_expectations: bool,
}

impl ValueBranches<'_> {
    fn wrap_left_operand(&mut self, first_branch: usize) -> Result<(), Error> {
        for branch in &mut self.branches[first_branch..] {
            if branch.unreachable_reasons().iter().any(Option::is_some) {
                return Err(Error::syntax(
                    "an unreachable expectation needs a direct conditional assignment",
                ));
            }
            if branch.sites.is_empty() {
                let path = std::mem::take(&mut branch.ancestors)
                    .into_iter()
                    .map(|yes| usize::from(!yes))
                    .collect();
                branch.sites.push((self.assignee.to_owned(), path));
            }
            branch.sites[0].1.insert(self.path.len(), 0);
        }
        Ok(())
    }
}

impl Locations {
    fn enter(&self, name: &str) {
        self.current.set(self.named.get(name).copied());
    }
}

pub(crate) mod vocabulary;

#[derive(Default)]
struct Parser {
    tokens: Vec<String>,
    offsets: Vec<usize>,
    locations: Locations,
    track_locations: bool,
    uses_bool: bool,
    position: usize,
    /// Nesting of formulas and expressions being parsed (review of #102).
    depth: usize,
    /// Formulas and expressions parsed in the current declaration (issue
    /// #106): bounds the tree the checker and Drop recurse over.
    nodes: usize,
    /// Declared records and their field names, for record literals.
    records: BTreeMap<String, Vec<String>>,
    newtypes: BTreeSet<String>,
}
/// Nesting and operator chains are bounded, so neither the parser nor the
/// checks after it can exhaust the stack.
const MAX_DEPTH: usize = 64;
const MAX_CHAIN: usize = 256;
const MAX_NODES: usize = 4096;
impl Parser {
    fn peek(&self) -> &str {
        self.tokens.get(self.position).map_or("", String::as_str)
    }
    fn peek_at(&self, offset: usize) -> &str {
        self.tokens
            .get(self.position + offset)
            .map_or("", String::as_str)
    }
    fn take(&mut self, token: &str) -> bool {
        if self.peek() == token {
            self.position += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, token: &str) -> Result<(), Error> {
        if self.take(token) {
            Ok(())
        } else {
            Err(Error::syntax(format!(
                "expected {token}, got {:?}",
                self.peek()
            )))
        }
    }
    fn name(&mut self) -> Result<String, Error> {
        let token = self.peek().to_owned();
        if token.is_empty()
            || !(token.as_bytes()[0].is_ascii_alphabetic() || token.starts_with('_'))
            || KEYWORDS.contains(&token.as_str())
        {
            return Err(Error::syntax(format!("expected a name, got {token:?}")));
        }
        self.position += 1;
        Ok(token)
    }

    fn term_id(&mut self) -> Result<String, Error> {
        let id = self
            .peek()
            .strip_prefix('"')
            .filter(|id| valid_term_id(id))
            .ok_or_else(|| Error::syntax("expected a quoted term ID"))?
            .to_owned();
        self.position += 1;
        Ok(id)
    }

    fn declared_name(&mut self) -> Result<String, Error> {
        let offset = self.offsets.get(self.position).copied();
        let name = self.name()?;
        self.locations
            .named
            .insert(name.clone(), offset.expect("name was read"));
        Ok(name)
    }
    fn number(&mut self) -> Result<i64, Error> {
        let value = self
            .peek()
            .parse()
            .map_err(|_| Error::syntax(format!("expected a number, got {:?}", self.peek())))?;
        self.position += 1;
        Ok(value)
    }
    fn mult(&mut self) -> Mult {
        for (word, mult) in [
            ("set", Mult::Set),
            ("lone", Mult::Lone),
            ("one", Mult::One),
            ("some", Mult::Some),
        ] {
            if self.take(word) {
                return mult;
            }
        }
        Mult::One
    }

    fn model(&mut self) -> Result<Model, Error> {
        self.expect("model")?;
        let mut model = Model {
            name: self.name()?,
            sigs: Vec::new(),
            entity_definitions: BTreeMap::new(),
            entity_translations: BTreeMap::new(),
            term_bindings: BTreeMap::new(),
            enums: Vec::new(),
            sums: Vec::new(),
            records: Vec::new(),
            sequences: Vec::new(),
            variants: Vec::new(),
            relations: Vec::new(),
            derivations: Vec::new(),
            facts: Vec::new(),
            init: Vec::new(),
            actions: Vec::new(),
            checks: Vec::new(),
            temporal: Vec::new(),
            transitions: Vec::new(),
            fairness: Vec::new(),
            scope: Scope {
                sizes: BTreeMap::new(),
                ints: None,
                steps: None,
            },
        };
        let mut scoped = false;
        while !self.peek().is_empty() {
            // The node bound applies to each declaration (issue #106).
            self.nodes = 0;
            let start = self.position;
            match self.peek() {
                "term" => {
                    self.position += 1;
                    let id = self.term_id()?;
                    self.expect("for")?;
                    let target = self.name()?;
                    if model.term_bindings.insert(target.clone(), id).is_some() {
                        return Err(Error::syntax(format!("more than one term binds {target}")));
                    }
                    self.locations.terms.insert(target, self.offsets[start]);
                }
                "derive" => {
                    self.position += 1;
                    let name = self.name()?;
                    self.expect("=")?;
                    let value = self.expression(0)?;
                    model.derivations.push((name, value));
                }
                "enum" => {
                    self.position += 1;
                    let name = self.name()?;
                    self.expect("=")?;
                    let mut members = vec![self.declared_name()?];
                    while self.take("|") {
                        members.push(self.declared_name()?);
                    }
                    model.enums.push((name, members));
                }
                "type" => {
                    self.position += 1;
                    let name = self.name()?;
                    self.expect("=")?;
                    let mut children = vec![self.name()?];
                    while self.take("+") {
                        children.push(self.name()?);
                    }
                    model.sums.push((name, children));
                }
                "sequence" => {
                    self.position += 1;
                    let name = self.name()?;
                    self.expect("=")?;
                    let element = self.name()?;
                    self.expect(",")?;
                    self.expect("max")?;
                    self.expect("=")?;
                    let max = self.size(crate::structured::MAX_SEQUENCE_LENGTH)?;
                    model.sequences.push(Sequence { name, element, max });
                }
                "variant" => {
                    self.position += 1;
                    let name = self.name()?;
                    self.expect("=")?;
                    let mut cases = Vec::new();
                    loop {
                        let tag = self.declared_name()?;
                        let payload = if self.take("(") {
                            let t = self.name()?;
                            self.expect(")")?;
                            Some(t)
                        } else {
                            None
                        };
                        cases.push(Case { tag, payload });
                        if !self.take("|") {
                            break;
                        }
                    }
                    model.variants.push(Variant { name, cases });
                }
                "record" | "data" => {
                    let data = self.peek() == "data";
                    self.position += 1;
                    let name = self.name()?;
                    if data {
                        self.expect("=")?;
                    }
                    let mut fields = Vec::new();
                    let kind = if data && self.peek() != "{" {
                        fields.push(("value".into(), self.name()?));
                        self.newtypes.insert(name.clone());
                        Some(DataKind::Newtype)
                    } else {
                        self.expect("{")?;
                        while !self.take("}") {
                            if !fields.is_empty() {
                                self.expect(",")?;
                                if self.take("}") {
                                    break;
                                }
                            }
                            let field = self.name()?;
                            self.expect(":")?;
                            fields.push((field, self.name()?));
                        }
                        data.then_some(DataKind::Record)
                    };
                    if fields.is_empty() {
                        return Err(Error::syntax(format!("record {name} has no fields")));
                    }
                    self.records.insert(
                        name.clone(),
                        fields.iter().map(|(field, _)| field.clone()).collect(),
                    );
                    let refinements = if data && self.take("where") {
                        self.refinements()?
                    } else {
                        Vec::new()
                    };
                    model.records.push(Record {
                        refinements,
                        name,
                        fields,
                        data: kind,
                    });
                }
                "sig" | "entity" => {
                    let entity = self.peek() == "entity";
                    self.position += 1;
                    let name = self.name()?;
                    if entity {
                        self.entity_description(&name, &mut model)?;
                    }
                    model.sigs.push(name);
                }
                "rel" | "var" => {
                    let var = self.take("var") || {
                        self.position += 1;
                        false
                    };
                    let name = self.name()?;
                    self.expect(":")?;
                    let shape = if ["set", "lone", "one", "some"].contains(&self.peek()) {
                        let mult = self.mult();
                        Shape::Unary {
                            sig: self.name()?,
                            mult,
                        }
                    } else {
                        let from = self.name()?;
                        self.expect("->")?;
                        let mult = self.mult();
                        Shape::Binary {
                            from,
                            to: self.name()?,
                            mult,
                        }
                    };
                    model.relations.push(Relation { name, var, shape });
                }
                "fact" | "init" => {
                    let initial = self.peek() == "init";
                    self.position += 1;
                    let formulas = self.block()?;
                    if initial {
                        model.init.extend(formulas);
                    } else {
                        model.facts.extend(formulas);
                    }
                }
                "action" | "behavior" => {
                    let explicit = self.peek() == "behavior";
                    self.position += 1;
                    model.actions.push(self.action(explicit)?);
                }
                "fairness" => {
                    self.position += 1;
                    model.fairness.push(self.fairness()?);
                }
                "eventually" | "leads_to" => {
                    let leads_to = self.take("leads_to");
                    if !leads_to {
                        self.expect("eventually")?;
                    }
                    let name = self.name()?;
                    let params = if self.peek() == "(" {
                        self.params()?
                    } else {
                        Vec::new()
                    };
                    let fairness = if self.take("with") {
                        self.expect("{")?;
                        let mut rules = Vec::new();
                        while !self.take("}") {
                            rules.push(self.fairness()?);
                        }
                        Some(rules)
                    } else {
                        None
                    };
                    let trigger = if leads_to {
                        Some(Formula::And(self.block()?))
                    } else {
                        None
                    };
                    let formula = Formula::And(self.block()?);
                    model.temporal.push(TemporalCheck {
                        name,
                        formula,
                        params,
                        trigger,
                        fairness,
                    });
                }
                "transition" => {
                    self.position += 1;
                    let name = self.name()?;
                    let action = if self.take("on") {
                        Some(self.name()?)
                    } else {
                        None
                    };
                    self.expect("{")?;
                    let definitions = self.local_definitions(None)?;
                    let formula = Formula::And(self.block_body()?);
                    self.save_bindings(&name, definitions);
                    model.transitions.push(TransitionCheck {
                        name,
                        action,
                        formula,
                    });
                }
                "invariant" | "reachable" => {
                    let kind = if self.take("invariant") {
                        CheckKind::Invariant
                    } else {
                        self.position += 1;
                        CheckKind::Reachable
                    };
                    let name = self.name()?;
                    let formula = Formula::And(self.block()?);
                    model.checks.push(Check {
                        name,
                        kind,
                        formula,
                    });
                }
                "scope" => {
                    self.position += 1;
                    if scoped {
                        return Err(Error::syntax("a second scope"));
                    }
                    scoped = true;
                    loop {
                        if self.take("steps") {
                            self.expect("=")?;
                            if model.scope.steps.is_some() {
                                return Err(Error::syntax("duplicate scope for steps"));
                            }
                            model.scope.steps = Some(self.size(usize::MAX)?);
                        } else if self.peek() == INT {
                            self.position += 1;
                            self.expect("=")?;
                            let low = self.signed()?;
                            self.expect(".")?;
                            self.expect(".")?;
                            let high = self.signed()?;
                            if !valid_int_scope(low, high) || model.scope.ints.is_some() {
                                return Err(Error::syntax(format!(
                                    "Int = {low}..{high}: at most one range of at most 20 integers"
                                )));
                            }
                            model.scope.ints = Some((low, high));
                        } else {
                            let sig = self.name()?;
                            self.expect("=")?;
                            let size = self.size(MAX_SCOPE)?;
                            if model.scope.sizes.insert(sig.clone(), size).is_some() {
                                return Err(Error::syntax(format!("duplicate scope for {sig}")));
                            }
                        }
                        if !self.take(",") {
                            break;
                        }
                    }
                }
                other => return Err(Error::syntax(format!("unexpected {other:?}"))),
            }
            let offset = self.offsets[start];
            match self.tokens[start].as_str() {
                "fact" => self.locations.facts.resize(model.facts.len(), offset),
                "init" => self.locations.init.resize(model.init.len(), offset),
                "scope" => self.locations.scope = Some(offset),
                "fairness" | "term" => {}
                _ => {
                    self.locations
                        .named
                        .insert(self.tokens[start + 1].clone(), offset);
                }
            }
        }
        Ok(model)
    }

    fn entity_description(&mut self, name: &str, model: &mut Model) -> Result<(), Error> {
        if let Some(definition) = self.peek().strip_prefix('"') {
            model
                .entity_definitions
                .insert(name.to_owned(), definition.to_owned());
            self.position += 1;
            return Ok(());
        }
        if !self.take("description") {
            return Ok(());
        }
        self.expect("=")?;
        if !self.take("{") {
            model
                .entity_definitions
                .insert(name.to_owned(), self.description_text()?);
            return Ok(());
        }
        let mut descriptions = BTreeMap::new();
        loop {
            let tag = if let Some(tag) = self.peek().strip_prefix('"') {
                let tag = tag.to_owned();
                self.position += 1;
                tag
            } else {
                self.name()?
            };
            let tag = tag.to_ascii_lowercase();
            self.expect(":")?;
            let text = self.description_text()?;
            if descriptions.insert(tag.clone(), text).is_some() {
                return Err(Error::syntax(format!("duplicate description for {tag}")));
            }
            if self.take("}") {
                break;
            }
            self.expect(",")?;
            if self.take("}") {
                break;
            }
        }
        if let Some(default) = descriptions.remove("default") {
            model.entity_definitions.insert(name.to_owned(), default);
        }
        if !descriptions.is_empty() {
            model
                .entity_translations
                .insert(name.to_owned(), descriptions);
        }
        Ok(())
    }

    fn description_text(&mut self) -> Result<String, Error> {
        let text = self
            .peek()
            .strip_prefix(['"', BLOCK_STRING_TOKEN])
            .ok_or_else(|| Error::syntax("expected a quoted description"))?
            .to_owned();
        self.position += 1;
        Ok(text)
    }

    fn signed(&mut self) -> Result<i64, Error> {
        if !self.take("-") {
            return self.number();
        }
        let token = self.peek();
        let value = format!("-{token}")
            .parse()
            .map_err(|_| Error::syntax(format!("expected a signed integer, got -{token}")))?;
        self.position += 1;
        Ok(value)
    }
    fn size(&mut self, maximum: usize) -> Result<usize, Error> {
        let value = self.number()?;
        usize::try_from(value)
            .ok()
            .filter(|size| (1..=maximum).contains(size))
            .ok_or_else(|| Error::syntax(format!("size {value} outside 1..={maximum}")))
    }

    fn refinements(&mut self) -> Result<Vec<crate::Refinement>, Error> {
        self.expect("{")?;
        let mut clauses = Vec::new();
        loop {
            self.charge_nodes(1)?;
            let left = self.refinement_operand()?;
            let op = crate::Refinement::operator(self.peek())
                .ok_or_else(|| Error::syntax("expected a numeric comparison in where"))?;
            self.position += 1;
            let right = self.refinement_operand()?;
            clauses.push(crate::Refinement { left, op, right });
            if self.take("}") {
                return Ok(clauses);
            }
            if !self.take("and") {
                self.expect(",")?;
                if self.take("}") {
                    return Ok(clauses);
                }
            }
        }
    }

    fn refinement_operand(&mut self) -> Result<crate::RefinementOperand, Error> {
        use crate::RefinementOperand;
        if self.peek().starts_with(|c: char| c.is_ascii_digit()) || self.peek() == "-" {
            let negative = self.take("-");
            let n = self.number()?;
            return Ok(RefinementOperand::Literal(if negative { -n } else { n }));
        }
        let mut path = vec![self.name()?];
        while self.take(".") {
            Self::chain(path.len() + 1)?;
            self.charge_nodes(1)?;
            path.push(self.name()?);
        }
        Ok(RefinementOperand::Field(path))
    }

    fn block(&mut self) -> Result<Vec<Formula>, Error> {
        self.expect("{")?;
        self.block_body()
    }

    fn block_body(&mut self) -> Result<Vec<Formula>, Error> {
        let mut formulas = Vec::new();
        while !self.take("}") {
            if self.peek().is_empty() {
                return Err(Error::syntax("unterminated block"));
            }
            formulas.push(self.formula()?);
            self.take(",");
        }
        Ok(formulas)
    }

    fn local_definitions(
        &mut self,
        mut binding_branches: Option<&mut BTreeMap<String, Vec<BindingBranch>>>,
    ) -> Result<Vec<(String, Expr)>, Error> {
        let mut definitions = Vec::new();
        while self.peek() == "let" && self.peek_at(1) != "=" {
            self.expect("let")?;
            let name = self.name()?;
            self.expect("=")?;
            let value = if let Some(bindings) = &mut binding_branches
                && self.track_locations
            {
                let mut branches = Vec::new();
                let mut locations = ValueBranches {
                    assignee: &name,
                    path: Vec::new(),
                    embedded: false,
                    branches: &mut branches,
                    allow_expectations: false,
                };
                let value = self.expression_located(0, Some(&mut locations))?;
                if !branches.is_empty() {
                    let branches = branches
                        .into_iter()
                        .map(|mut branch| {
                            let path = branch.sites.pop().map_or_else(
                                || {
                                    branch
                                        .ancestors
                                        .iter()
                                        .map(|&yes| usize::from(!yes))
                                        .collect()
                                },
                                |(_, path)| path,
                            );
                            branch.binding = true;
                            BindingBranch { branch, path }
                        })
                        .collect();
                    bindings.insert(name.clone(), branches);
                }
                value
            } else {
                self.expression(0)?
            };
            definitions.push((name, value));
        }
        Ok(definitions)
    }

    fn save_bindings(&mut self, declaration: &str, definitions: Vec<(String, Expr)>) {
        if !definitions.is_empty() {
            self.locations.bindings.insert(
                declaration.to_owned(),
                LocalDefinitions {
                    definitions,
                    remaining: MAX_NODES - self.nodes,
                },
            );
        }
    }
    fn fairness(&mut self) -> Result<Fairness, Error> {
        let kind = if self.take("weak") {
            FairnessKind::Weak
        } else {
            self.expect("strong")?;
            FairnessKind::Strong
        };
        let action = self.name()?;
        let each = self.take("(");
        if each {
            self.expect("*")?;
            self.expect(")")?;
        }
        Ok(Fairness { kind, action, each })
    }

    fn params(&mut self) -> Result<Vec<(String, String)>, Error> {
        let mut params = Vec::new();
        self.expect("(")?;
        if !self.take(")") {
            loop {
                let param = self.name()?;
                self.expect(":")?;
                params.push((param, self.name()?));
                if !self.take(",") {
                    break;
                }
            }
            self.expect(")")?;
        }
        Ok(params)
    }

    fn action(&mut self, explicit: bool) -> Result<Action, Error> {
        let name = self.name()?;
        let params = self.params()?;
        let mut action = Action {
            name,
            params,
            guards: Vec::new(),
            assignments: Vec::new(),
        };
        self.expect("{")?;
        if explicit {
            if self.take("guard") {
                action.guards = self.block()?;
            }
            if self.take("update") {
                self.expect("{")?;
                let mut binding_branches = BTreeMap::new();
                let definitions = self.local_definitions(Some(&mut binding_branches))?;
                if !binding_branches.is_empty() {
                    self.locations
                        .binding_branches
                        .insert(action.name.clone(), binding_branches);
                }
                let mut branches = Vec::new();
                action.assignments = self.update_assignments(&mut Vec::new(), &mut branches)?;
                self.save_branches(&action.name, branches);
                self.save_bindings(&action.name, definitions);
            }
            self.expect("}")?;
            return Ok(action);
        }
        let mut branches = Vec::new();
        while !self.take("}") {
            if self.peek().is_empty() {
                return Err(Error::syntax("unterminated action"));
            }
            if self.peek_at(1) == "'" {
                action
                    .assignments
                    .push(self.assignment(true, &[], &mut branches)?);
            } else {
                action.guards.push(self.formula()?);
            }
        }
        self.save_branches(&action.name, branches);
        Ok(action)
    }

    fn save_branches(&mut self, action: &str, mut branches: Vec<crate::UpdateBranch>) {
        if !branches.is_empty() {
            branches.sort_by_key(|branch| branch.offset);
            self.locations
                .update_branches
                .insert(action.to_owned(), branches);
        }
    }

    fn update_assignments(
        &mut self,
        ancestors: &mut Vec<bool>,
        branches: &mut Vec<crate::UpdateBranch>,
    ) -> Result<Vec<(String, Expr)>, Error> {
        let mut assignments = Vec::new();
        while !self.take("}") {
            if self.peek() == "if" && self.peek_at(1) == "{" {
                self.enter()?;
                let group = self.conditional_updates(ancestors, branches);
                self.depth -= 1;
                assignments.extend(group?);
            } else {
                assignments.push(self.assignment(false, ancestors, branches)?);
            }
        }
        Ok(assignments)
    }

    fn conditional_updates(
        &mut self,
        ancestors: &mut Vec<bool>,
        branches: &mut Vec<crate::UpdateBranch>,
    ) -> Result<Vec<(String, Expr)>, Error> {
        let offset = self.offsets[self.position];
        self.expect("if")?;
        let start = self.nodes;
        let condition = Formula::And(self.block()?);
        let condition_nodes = self.nodes - start;
        self.expect("then")?;
        let then_unreachable = self.arm_expectation()?;
        self.expect("{")?;
        ancestors.push(true);
        let yes = self.update_assignments(ancestors, branches)?;
        ancestors.pop();
        let (no, else_unreachable) =
            if self.peek() == "else" && matches!(self.peek_at(1), "{" | "expect") {
                self.position += 1;
                let expectation = self.arm_expectation()?;
                self.expect("{")?;
                ancestors.push(false);
                let no = self.update_assignments(ancestors, branches)?;
                ancestors.pop();
                (no, expectation)
            } else {
                (Vec::new(), None)
            };
        let mut pairs = BTreeMap::<String, [Option<Expr>; 2]>::new();
        for (index, branch) in [yes, no].into_iter().enumerate() {
            for (target, value) in branch {
                let field = &mut pairs.entry(target.clone()).or_default()[index];
                if field.replace(value).is_some() {
                    return Err(Error::kind(format!("duplicate update of {target}")));
                }
            }
        }
        let Some(assignee) = pairs.keys().next() else {
            return Err(Error::syntax("conditional update needs an assignment"));
        };
        if self.track_locations {
            branches.push(crate::UpdateBranch {
                offset,
                assignee: assignee.clone(),
                ancestors: ancestors.clone(),
                sites: Vec::new(),
                binding: false,
                value_conditional: false,
                unreachable_reasons: [then_unreachable, else_unreachable],
            });
        }
        // Every expanded assignment carries the condition, a conditional
        // expression, and possibly a reference to its previous value.
        self.charge_nodes((condition_nodes + 3).saturating_mul(pairs.len()))?;
        Ok(pairs
            .into_iter()
            .map(|(target, [yes, no])| {
                let prior = || Expr::Name(target.clone());
                let value = Expr::If(
                    Box::new(condition.clone()),
                    Box::new(yes.unwrap_or_else(prior)),
                    Box::new(no.unwrap_or_else(prior)),
                );
                (target, value)
            })
            .collect())
    }

    fn arm_expectation(&mut self) -> Result<Option<String>, Error> {
        if !self.take("expect") {
            return Ok(None);
        }
        self.expect("unreachable")?;
        let Some(reason) = self
            .peek()
            .strip_prefix('"')
            .filter(|reason| !reason.trim().is_empty())
        else {
            return Err(Error::syntax(
                "expected a nonempty quoted reason for the unreachable arm",
            ));
        };
        let reason = reason.to_owned();
        self.position += 1;
        Ok(Some(reason))
    }

    fn assignment(
        &mut self,
        primed: bool,
        ancestors: &[bool],
        branches: &mut Vec<crate::UpdateBranch>,
    ) -> Result<(String, Expr), Error> {
        let var = self.name()?;
        if primed {
            self.expect("'")?;
        }
        self.expect("=")?;
        let mut locations = ValueBranches {
            assignee: &var,
            path: ancestors.iter().map(|&yes| usize::from(!yes)).collect(),
            embedded: false,
            branches,
            allow_expectations: true,
        };
        let value = self.expression_located(0, Some(&mut locations))?;
        Ok((var, value))
    }

    fn charge_nodes(&mut self, count: usize) -> Result<(), Error> {
        self.nodes += count;
        if self.nodes > MAX_NODES {
            return Err(Error::syntax(format!(
                "a declaration with more than {MAX_NODES} nodes"
            )));
        }
        Ok(())
    }

    fn enter(&mut self) -> Result<(), Error> {
        self.charge_nodes(1)?;
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Error::syntax(format!("nesting deeper than {MAX_DEPTH}")));
        }
        Ok(())
    }
    fn chain(length: usize) -> Result<(), Error> {
        if length > MAX_CHAIN {
            return Err(Error::syntax(format!(
                "more than {MAX_CHAIN} operands in a chain"
            )));
        }
        Ok(())
    }
    /// iff (non-associative) < implies (right) < or < and < not < atoms.
    fn formula(&mut self) -> Result<Formula, Error> {
        self.enter()?;
        let formula = self.equivalence();
        self.depth -= 1;
        formula
    }
    fn equivalence(&mut self) -> Result<Formula, Error> {
        let start = self.nodes;
        let left = self.implication()?;
        if !self.take("iff") {
            return Ok(left);
        }
        let right = self.implication()?;
        if self.peek() == "iff" {
            return Err(Error::syntax(
                "use parentheses to group multiple iff operators",
            ));
        }
        self.charge_nodes(self.nodes - start + 3)?;
        Ok(Formula::And(vec![
            Formula::Implies(Box::new(left.clone()), Box::new(right.clone())),
            Formula::Implies(Box::new(right), Box::new(left)),
        ]))
    }
    fn implication(&mut self) -> Result<Formula, Error> {
        let left = self.disjunction()?;
        if self.take("implies") {
            self.enter()?;
            let right = self.implication();
            self.depth -= 1;
            return Ok(Formula::Implies(Box::new(left), Box::new(right?)));
        }
        Ok(left)
    }
    fn disjunction(&mut self) -> Result<Formula, Error> {
        let mut left = self.conjunction()?;
        let mut length = 1;
        while self.take("or") {
            length += 1;
            Self::chain(length)?;
            left = Formula::Or(Box::new(left), Box::new(self.conjunction()?));
        }
        Ok(left)
    }
    fn conjunction(&mut self) -> Result<Formula, Error> {
        let mut parts = vec![self.negation()?];
        while self.take("and") {
            Self::chain(parts.len() + 1)?;
            parts.push(self.negation()?);
        }
        Ok(if parts.len() == 1 {
            parts.pop().unwrap()
        } else {
            Formula::And(parts)
        })
    }
    fn negation(&mut self) -> Result<Formula, Error> {
        if self.take("not") {
            // A chain of `not` counts against the nesting bound (review of
            // #110).
            self.enter()?;
            let inner = self.negation();
            self.depth -= 1;
            return Ok(Formula::Not(Box::new(inner?)));
        }
        self.atom()
    }
    fn atom(&mut self) -> Result<Formula, Error> {
        if self.peek() == "{" {
            return Ok(Formula::And(self.block()?));
        }
        if self.peek() == "is" && self.peek_at(1) == "(" {
            self.position += 2;
            let value = self.expression(0)?;
            self.expect(",")?;
            let tag = self.name()?;
            self.expect(")")?;
            return Ok(Formula::HasTag(value, tag));
        }
        if ["all", "some"].contains(&self.peek()) && self.peek_at(2) == ":" {
            let quantifier = if self.take("all") {
                Quantifier::All
            } else {
                self.position += 1;
                Quantifier::Some
            };
            let variable = self.name()?;
            self.expect(":")?;
            let sig = self.name()?;
            self.expect("|")?;
            return Ok(Formula::Quantified(
                quantifier,
                variable,
                sig,
                Box::new(self.formula()?),
            ));
        }
        for (word, make) in [
            ("no", None),
            ("some", Some(Mult::Some)),
            ("one", Some(Mult::One)),
            ("lone", Some(Mult::Lone)),
        ] {
            if self.take(word) {
                let expression = self.expression(0)?;
                return Ok(match make {
                    None => Formula::Empty(expression),
                    Some(mult) => Formula::Mult(mult, expression),
                });
            }
        }
        // A parenthesis opens a formula, or a set expression compared after
        // it, as in `(A + B) in C`: try the formula and fall back (plan 0097,
        // found by the generated models).
        if self.peek() == "(" {
            let start = (self.position, self.depth, self.nodes);
            self.position += 1;
            let formula = self.formula().and_then(|formula| {
                self.expect(")")?;
                Ok(formula)
            });
            // Integer comparisons and `*` follow parenthesized integers
            // (plan 0103, found by the generated models).
            let comparison_follows = matches!(
                self.peek(),
                "in" | "=" | "!=" | "<" | "<=" | ">" | ">=" | "'"
            ) || Self::infix(self.peek()).is_some()
                || self.peek() == "not" && self.peek_at(1) == "in";
            match formula {
                Ok(formula) if !comparison_follows => return Ok(formula),
                _ => (self.position, self.depth, self.nodes) = start,
            }
        }
        let integer = self.peek() == "#"
            || self.peek() == "-"
            || self.peek().bytes().all(|byte| byte.is_ascii_digit());
        let start_nodes = self.nodes;
        let left_start = self.offsets.get(self.position).copied();
        let left = self.expression(0)?;
        let ordering = match self.peek() {
            "<" => Some(IntCompare::Less),
            "<=" => Some(IntCompare::LessEqual),
            ">" => Some(IntCompare::Greater),
            ">=" => Some(IntCompare::GreaterEqual),
            "=" if integer => Some(IntCompare::Equal),
            "!=" if integer => Some(IntCompare::NotEqual),
            _ => None,
        };
        if let Some(operator) = ordering {
            self.position += 1;
            let right_start = self.offsets.get(self.position).copied();
            let right = self.expression(0)?;
            return Ok(Formula::Ints(
                operator,
                Self::int(left, left_start),
                Self::int(right, right_start),
            ));
        }
        if integer && self.peek() != "in" && !(self.peek() == "not" && self.peek_at(1) == "in") {
            return Err(Error::syntax(format!(
                "expected an integer comparison, got {:?}",
                self.peek()
            )));
        }
        let operator = match self.peek() {
            "in" => Compare::In,
            "=" => Compare::Equal,
            "!=" => Compare::NotEqual,
            "not" if self.peek_at(1) == "in" => {
                self.position += 1;
                Compare::NotIn
            }
            // Only a standalone call is a preservation condition. In a
            // comparison, the same spelling can still be a variant tag.
            // A Bool expression on its own holds when it is true (plan
            // 0103); the checker rejects any other expression here.
            _ => {
                let left = match left {
                    Expr::Variant(name, Some(value)) if name == "unchanged" => {
                        // Reuse the phase checker and charge for both operands.
                        self.charge_nodes(self.nodes - start_nodes)?;
                        return Ok(Formula::Compare(
                            Compare::Equal,
                            Expr::At(Phase::Before, value.clone()),
                            Expr::At(Phase::After, value),
                        ));
                    }
                    other => other,
                };
                return Ok(Formula::Compare(
                    Compare::Equal,
                    left,
                    Expr::Name("true".into()),
                ));
            }
        };
        self.position += 1;
        Ok(Formula::Compare(operator, left, self.expression(0)?))
    }
    /// A record literal: every declared field once, in any order.
    fn record(
        &mut self,
        name: String,
        declared: &[String],
        mut locations: Option<&mut ValueBranches<'_>>,
    ) -> Result<Expr, Error> {
        self.expect("{")?;
        let mut given: BTreeMap<String, Expr> = BTreeMap::new();
        while !self.take("}") {
            if !given.is_empty() {
                self.expect(",")?;
            }
            let field = self.name()?;
            self.expect(":")?;
            let index = declared.iter().position(|candidate| candidate == &field);
            let value = match index {
                Some(index) => self.value_operand(0, index, locations.as_deref_mut())?,
                None => self.expression(0)?,
            };
            if index.is_none() || given.insert(field.clone(), value).is_some() {
                return Err(Error::syntax(format!("{name}: unexpected field {field}")));
            }
        }
        let values = declared
            .iter()
            .map(|field| {
                given
                    .remove(field)
                    .ok_or_else(|| Error::syntax(format!("{name}: missing field {field}")))
            })
            .collect::<Result<_, _>>()?;
        Ok(Expr::Record(name, values))
    }
    /// `#e` and literals keep their own forms, which the Alloy lowering
    /// writes as Alloy integers.
    fn int(expression: Expr, offset: Option<usize>) -> Int {
        match expression {
            Expr::Count(counted) => Int::Count(*counted),
            Expr::Literal(value) => Int::Literal(value),
            other => match offset {
                Some(offset) => Int::LocatedValue(other, offset),
                None => Int::Value(other),
            },
        }
    }
    fn infix(token: &str) -> Option<(SetOp, u8)> {
        Some(match token {
            "+" => (SetOp::Union, 1),
            "++" => (SetOp::Override, 1),
            "-" => (SetOp::Difference, 1),
            "&" => (SetOp::Intersection, 2),
            "*" => (SetOp::Multiply, 2),
            "->" => (SetOp::Product, 3),
            "." => (SetOp::Join, 4),
            _ => return None,
        })
    }

    fn expression(&mut self, minimum: u8) -> Result<Expr, Error> {
        self.expression_located(minimum, None)
    }

    fn expression_located(
        &mut self,
        minimum: u8,
        locations: Option<&mut ValueBranches<'_>>,
    ) -> Result<Expr, Error> {
        self.enter()?;
        let expression = self.operand_chain(minimum, locations);
        self.depth -= 1;
        expression
    }

    fn value_arm(
        &mut self,
        taken: bool,
        locations: Option<&mut ValueBranches<'_>>,
    ) -> Result<(Expr, Option<String>), Error> {
        let expectation = if self.peek() == "expect" && self.peek_at(1) == "unreachable" {
            if !locations
                .as_ref()
                .is_some_and(|locations| locations.allow_expectations && !locations.embedded)
            {
                return Err(Error::syntax(
                    "an unreachable expectation needs a direct conditional assignment",
                ));
            }
            self.arm_expectation()?
        } else {
            None
        };
        let Some(locations) = locations else {
            return Ok((self.expression(0)?, expectation));
        };
        locations.path.push(usize::from(!taken));
        let value = self.expression_located(0, Some(locations));
        locations.path.pop();
        value.map(|value| (value, expectation))
    }

    fn value_operand(
        &mut self,
        minimum: u8,
        index: usize,
        locations: Option<&mut ValueBranches<'_>>,
    ) -> Result<Expr, Error> {
        let Some(locations) = locations else {
            return self.expression(minimum);
        };
        let embedded = std::mem::replace(&mut locations.embedded, true);
        locations.path.push(index);
        let value = self.expression_located(minimum, Some(locations));
        locations.path.pop();
        locations.embedded = embedded;
        value
    }
    fn starts_conditional(&self) -> bool {
        if self.peek() != "if" || self.peek_at(1) != "{" || self.records.contains_key("if") {
            return false;
        }
        let mut braces = 0;
        for (offset, token) in self.tokens[self.position + 1..].iter().enumerate() {
            match token.as_str() {
                "{" => braces += 1,
                "}" => {
                    braces -= 1;
                    if braces == 0 {
                        return self.peek_at(offset + 2) == "then";
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn operand_chain(
        &mut self,
        minimum: u8,
        mut locations: Option<&mut ValueBranches<'_>>,
    ) -> Result<Expr, Error> {
        let first_branch = locations.as_ref().map(|locations| locations.branches.len());
        let mut length = 1;
        let mut left = if self.starts_conditional() {
            let offset = self.offsets[self.position];
            self.expect("if")?;
            let condition = Formula::And(self.block()?);
            self.expect("then")?;
            let (yes, then_unreachable) = self.value_arm(true, locations.as_deref_mut())?;
            self.expect("else")?;
            let (no, else_unreachable) = self.value_arm(false, locations.as_deref_mut())?;
            if let Some(locations) = &mut locations
                && (self.track_locations
                    || then_unreachable.is_some()
                    || else_unreachable.is_some())
            {
                let ancestors = if locations.embedded {
                    Vec::new()
                } else {
                    locations.path.iter().map(|&index| index == 0).collect()
                };
                let mut branch = crate::UpdateBranch::from_selector(
                    offset,
                    locations.assignee.to_owned(),
                    ancestors,
                )
                .with_value_conditional(true)
                .with_unreachable_reasons([then_unreachable, else_unreachable]);
                if locations.embedded {
                    branch
                        .sites
                        .push((locations.assignee.to_owned(), locations.path.clone()));
                }
                locations.branches.push(branch);
            }
            Expr::If(Box::new(condition), Box::new(yes), Box::new(no))
        } else if self.peek() == "bool"
            && self.peek_at(1) == "{"
            && !self.records.contains_key("bool")
        {
            self.position += 1;
            self.uses_bool = true;
            let condition = Formula::And(self.block()?);
            self.charge_nodes(2)?;
            Expr::If(
                Box::new(condition),
                Box::new(Expr::Name("true".into())),
                Box::new(Expr::Name("false".into())),
            )
        } else if self.peek() == "select" && self.peek_at(1) == "(" && self.peek_at(3) == ":" {
            self.position += 2;
            let variable = self.name()?;
            self.expect(":")?;
            let sort = self.name()?;
            self.expect("|")?;
            let predicate = self.formula()?;
            self.expect(")")?;
            Expr::Select(variable, sort, Box::new(predicate))
        } else if self.take("(") {
            let inner = self.expression_located(0, locations.as_deref_mut())?;
            self.expect(")")?;
            inner
        } else if self.take("none") {
            Expr::None
        } else if self.take("true") {
            Expr::Name("true".into())
        } else if self.take("false") {
            Expr::Name("false".into())
        } else if self.take("#") {
            Expr::Count(Box::new(self.value_operand(
                4,
                0,
                locations.as_deref_mut(),
            )?))
        } else if self.peek() == "-" || self.peek().bytes().all(|byte| byte.is_ascii_digit()) {
            Expr::Literal(self.signed()?)
        } else {
            let name = self.name()?;
            if self.take("[") {
                let mut items = Vec::new();
                if !self.take("]") {
                    loop {
                        Self::chain(items.len() + 1)?;
                        items.push(self.value_operand(0, items.len(), locations.as_deref_mut())?);
                        if self.take("]") {
                            break;
                        }
                        self.expect(",")?;
                    }
                }
                Expr::Sequence(name, items)
            } else if self.take("(") {
                let argument = if self.peek() == ")" {
                    None
                } else {
                    Some(Box::new(self.value_operand(
                        0,
                        0,
                        locations.as_deref_mut(),
                    )?))
                };
                let required = |arg: Option<Box<Expr>>| {
                    arg.ok_or_else(|| Error::syntax("expected an argument"))
                };
                let call = match name.as_str() {
                    _ if self.newtypes.contains(&name) => {
                        let value = argument
                            .ok_or_else(|| Error::syntax(format!("{name} expects one argument")))?;
                        Expr::Record(name, vec![*value])
                    }
                    "len" => Expr::Length(required(argument)?),
                    "before" => Expr::At(Phase::Before, required(argument)?),
                    "after" => Expr::At(Phase::After, required(argument)?),
                    "append" | "at" => {
                        let first = required(argument)?;
                        self.expect(",")?;
                        let second =
                            Box::new(self.value_operand(0, 1, locations.as_deref_mut())?);
                        if name == "append" {
                            Expr::Append(first, second)
                        } else {
                            Expr::Index(first, second)
                        }
                    }
                    operation @ ("take" | "removeAt" | "removeAll") if self.take(",") => {
                        let seq = required(argument)?;
                        let value = Box::new(self.value_operand(0, 1, locations.as_deref_mut())?);
                        match operation {
                            "take" => Expr::Take(seq, value),
                            "removeAt" => Expr::RemoveAt(seq, value),
                            _ => Expr::RemoveAll(seq, value),
                        }
                    }
                    "reachableFrom" if self.take(",") => Expr::ReachableFrom(
                        required(argument)?,
                        Box::new(self.value_operand(0, 1, locations.as_deref_mut())?),
                    ),
                    "union" | "difference" | "withoutKeys" if self.take(",") => {
                        let operator = match name.as_str() {
                            "union" => SetOp::SetUnion,
                            "difference" => SetOp::SetDifference,
                            _ => SetOp::WithoutKeys,
                        };
                        Expr::Binary(
                            operator,
                            required(argument)?,
                            Box::new(self.value_operand(0, 1, locations.as_deref_mut())?),
                        )
                    }
                    "payload" => {
                        let first = required(argument)?;
                        self.expect(",")?;
                        Expr::Payload(first, self.name()?)
                    }
                    _ => Expr::Variant(name, argument),
                };
                self.expect(")")?;
                call
            } else {
                match self.records.get(&name).cloned() {
                    Some(declared) if self.peek() == "{" => {
                        self.record(name, &declared, locations.as_deref_mut())?
                    }
                    _ => Expr::Name(name),
                }
            }
        };
        if self.take("'") {
            left = Expr::At(Phase::After, Box::new(left));
        }
        while let Some((operator, precedence)) = Self::infix(self.peek()) {
            if precedence < minimum {
                break;
            }
            self.position += 1;
            length += 1;
            Self::chain(length)?;
            if let (Some(locations), Some(first_branch)) = (locations.as_deref_mut(), first_branch)
            {
                locations.wrap_left_operand(first_branch)?;
            }
            let right = self.value_operand(precedence + 1, 1, locations.as_deref_mut())?;
            left = Expr::Binary(operator, Box::new(left), Box::new(right));
        }
        Ok(left)
    }
}

/// The type of an expression: a set of one sort, or a binary relation
/// between two sorts (plan 0099). `none` fits any type.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Type {
    Any,
    Set(String),
    Pair(String, String),
}
impl Type {
    fn arity(&self) -> Option<usize> {
        match self {
            Type::Any => None,
            Type::Set(_) => Some(1),
            Type::Pair(..) => Some(2),
        }
    }
    fn numeric(&self) -> bool {
        matches!(self, Self::Set(sort) if numeric_sort(sort))
    }
    fn describe(&self) -> String {
        match self {
            Type::Any => "none".into(),
            Type::Set(sort) => format!("Set[{sort}]"),
            Type::Pair(from, to) => format!("{from} -> {to}"),
        }
    }
}
/// Names bound by parameters and quantifiers, with their sorts.
type Bound = BTreeMap<String, String>;

type BindingScope<'a> = (&'a BTreeMap<String, BindingValue>, &'a Cell<usize>);

#[derive(Clone, Copy)]
enum BindingSite {
    Update,
    Transition,
    Derivation,
}

impl BindingSite {
    fn expansion_limit(self) -> &'static str {
        match self {
            Self::Update => "expanded update bindings exceed the declaration size limit",
            Self::Transition => "expanded transition bindings exceed the declaration size limit",
            Self::Derivation => "expanded derivations exceed the model size limit",
        }
    }
}

struct BindingValue {
    expression: Expr,
    nodes: usize,
}

type StateReads = RefCell<BTreeMap<usize, BTreeSet<String>>>;

#[derive(Clone, Copy)]
struct ReadLocation<'a> {
    offset: &'a Cell<Option<usize>>,
    reads: &'a StateReads,
}

impl ReadLocation<'_> {
    fn record(self, name: &str) {
        if let Some(offset) = self.offset.get() {
            self.reads
                .borrow_mut()
                .entry(offset)
                .or_default()
                .insert(name.to_owned());
        }
    }
}

#[derive(Clone)]
struct Checker<'a> {
    model: &'a Model,
    transition: bool,
    source_calls: bool,
    sigs: &'a BTreeSet<&'a str>,
    singletons: Vec<Expr>,
    local_bindings: Option<(BindingScope<'a>, BindingSite)>,
    derived_bindings: Option<BindingScope<'a>>,
    // Set only while checking an already resolved expression expanded at a use site.
    expanded_binders: Option<BTreeMap<String, String>>,
    read_location: Option<ReadLocation<'a>>,
    operand_error: &'a Cell<Option<usize>>,
}
impl Checker<'_> {
    fn binding(&self, name: &str) -> Option<(&BindingValue, &Cell<usize>, BindingSite)> {
        [
            self.local_bindings,
            self.derived_bindings
                .map(|scope| (scope, BindingSite::Derivation)),
        ]
        .into_iter()
        .flatten()
        .find_map(|((values, remaining), site)| Some((values.get(name)?, remaining, site)))
    }

    fn local_values(
        &self,
        definitions: Option<LocalDefinitions>,
        bound: &Bound,
        remaining: &Cell<usize>,
        site: BindingSite,
    ) -> Result<BTreeMap<String, BindingValue>, Error> {
        let mut locals = BTreeMap::new();
        if let Some(LocalDefinitions {
            definitions,
            remaining: budget,
        }) = definitions
        {
            remaining.set(budget);
            for (name, mut expression) in definitions {
                let scoped = Checker {
                    local_bindings: Some(((&locals, remaining), site)),
                    ..self.clone()
                };
                scoped.fresh(&name, bound)?;
                scoped.expression(&mut expression, bound, true)?;
                let context = if matches!(site, BindingSite::Transition) {
                    "transition expression"
                } else {
                    "update expression"
                };
                let nodes = binding_value_nodes(&expression, context)?;
                crate::numeric::validate_binding(self.model, &expression)?;
                locals.insert(name, BindingValue { expression, nodes });
            }
        }
        Ok(locals)
    }

    /// Sorts a join may connect: one is a subsort of the other.
    fn compatible(&self, a: &str, b: &str) -> bool {
        self.model.subsort(a, b) || self.model.subsort(b, a)
    }
    /// Whether a value of type `value` fits a variable of type `expected`.
    fn fits(&self, value: &Type, expected: &Type) -> bool {
        match (value, expected) {
            (Type::Any, _) => true,
            (Type::Set(a), Type::Set(b)) => self.model.subsort(a, b),
            (Type::Pair(a, b), Type::Pair(c, d)) => {
                self.model.subsort(a, c) && self.model.subsort(b, d)
            }
            _ => false,
        }
    }
    fn fits_storage(&self, value: &Type, expected: &Type) -> bool {
        self.fits(value, expected)
            || match (value, expected) {
                (Type::Set(a), Type::Set(b)) => a == INT && b == NAT,
                (Type::Pair(a, b), Type::Pair(c, d)) => {
                    self.model.subsort(a, c) && b == INT && d == NAT
                }
                _ => false,
            }
    }
    fn shape(shape: &Shape) -> Type {
        match shape {
            Shape::Unary { sig, .. } => Type::Set(sig.clone()),
            Shape::Binary { from, to, .. } => Type::Pair(from.clone(), to.clone()),
        }
    }
    /// Both sides of a set operation or comparison have one type, up to
    /// the sum sorts both belong to (plan 0100).
    fn same(&self, left: Type, right: Type, what: &str) -> Result<Type, Error> {
        Self::combine_types(left, right, what, |a, b| self.model.join_sorts(a, b))
    }

    fn intersection(&self, left: Type, right: Type) -> Result<Type, Error> {
        Self::combine_types(left, right, "set operation", |a, b| {
            if self.model.subsort(a, b) {
                Some(a.to_owned())
            } else if self.model.subsort(b, a) {
                Some(b.to_owned())
            } else {
                self.model.join_sorts(a, b)
            }
        })
    }

    fn combine_types(
        left: Type,
        right: Type,
        what: &str,
        sort: impl Fn(&str, &str) -> Option<String>,
    ) -> Result<Type, Error> {
        let combined = match (&left, &right) {
            (Type::Any, _) => return Ok(right),
            (_, Type::Any) => return Ok(left),
            (Type::Set(a), Type::Set(b)) => sort(a, b).map(Type::Set),
            (Type::Pair(a, b), Type::Pair(c, d)) => sort(a, c)
                .zip(sort(b, d))
                .map(|(from, to)| Type::Pair(from, to)),
            _ => return Err(Error::kind(format!("{what} of different arities"))),
        };
        combined.ok_or_else(|| {
            Error::kind(format!(
                "{what} of different sorts: {} and {}",
                left.describe(),
                right.describe()
            ))
        })
    }
    /// Whether an expression has exactly one element under the checked guards.
    fn singular(&self, expression: &Expr, bound: &Bound) -> bool {
        if self.singletons.contains(expression) {
            return true;
        }
        match expression {
            Expr::Literal(_) | Expr::Count(_) | Expr::Arith(..) => true,
            Expr::At(phase, expr) => self.state_checker(*phase).singular(expr, bound),
            Expr::If(condition, yes, no) => {
                let mut branch = self.clone();
                branch.assume(condition, bound);
                branch.singular(yes, bound) && self.singular(no, bound)
            }
            Expr::Sequence(_, items) => items.iter().all(|item| self.singular(item, bound)),
            Expr::Append(left, right) | Expr::Take(left, right) | Expr::RemoveAt(left, right) => {
                self.singular(left, bound) && self.singular(right, bound)
            }
            Expr::Tail(value) | Expr::Length(value) | Expr::RemoveAll(value, _) => {
                self.singular(value, bound)
            }
            Expr::Variant(_, value) => value.as_ref().is_none_or(|v| self.singular(v, bound)),
            Expr::Name(name) => {
                bound.contains_key(name)
                    || self.model.member(name).is_some()
                    || self.model.relation(name).is_some_and(|relation| {
                        matches!(
                            relation.shape,
                            Shape::Unary {
                                mult: Mult::One,
                                ..
                            }
                        )
                    })
            }
            Expr::Record(_, values) => values.iter().all(|value| self.singular(value, bound)),
            Expr::Field(receiver, _) => self.singular(receiver, bound),
            Expr::Binary(SetOp::Join, left, right) => {
                self.singular(left, bound)
                    && match right.as_ref() {
                        Expr::Name(name) => self.model.relation(name).is_some_and(|relation| {
                            matches!(
                                relation.shape,
                                Shape::Binary {
                                    mult: Mult::One,
                                    ..
                                }
                            )
                        }),
                        _ => false,
                    }
            }
            _ => false,
        }
    }
    fn state_checker(&self, phase: Phase) -> Self {
        let singletons = self
            .singletons
            .iter()
            .filter_map(|value| match (phase, value) {
                (Phase::After, Expr::At(Phase::After, inner)) => Some(inner.as_ref().clone()),
                (Phase::Before, Expr::At(Phase::After, _)) | (Phase::After, _) => None,
                (Phase::Before, value) => Some(value.clone()),
            })
            .collect();
        Self {
            transition: false,
            singletons,
            expanded_binders: self.expanded_binders.clone(),
            ..*self
        }
    }

    fn at_most_one(&self, expression: &Expr, bound: &Bound) -> bool {
        match expression {
            Expr::At(phase, expr) => self.state_checker(*phase).at_most_one(expr, bound),
            Expr::Field(receiver, _) => {
                self.at_most_one(receiver, bound) || self.singular(receiver, bound)
            }
            Expr::Name(name) => self.model.relation(name).is_some_and(|relation| {
                matches!(
                    relation.shape,
                    Shape::Unary {
                        mult: Mult::Lone,
                        ..
                    }
                )
            }),
            Expr::Binary(SetOp::Join, key, relation) => {
                self.singular(key, bound)
                    && matches!(relation.as_ref(), Expr::Name(name)
                    if self.model.relation(name).is_some_and(|relation| {
                        matches!(relation.shape, Shape::Binary { mult: Mult::Lone, .. })
                    }))
            }
            Expr::Index(..) => true,
            _ => false,
        }
    }

    fn assume(&mut self, formula: &Formula, bound: &Bound) {
        match formula {
            Formula::And(parts) => {
                for part in parts {
                    self.assume(part, bound);
                }
            }
            Formula::Mult(Mult::Some, expression) if self.at_most_one(expression, bound) => {
                self.singletons.push(expression.clone());
            }
            Formula::Mult(Mult::One, expression) => {
                self.singletons.push(expression.clone());
            }
            _ => {}
        }
    }

    fn guard(&mut self, formula: &mut Formula, bound: &Bound, vars: bool) -> Result<(), Error> {
        if let Formula::And(parts) = formula {
            for part in parts {
                self.guard(part, bound, vars)?;
            }
        } else {
            self.formula(formula, bound, vars)?;
            self.assume(formula, bound);
        }
        Ok(())
    }

    fn integer(&self, expression: &Expr, bound: &Bound, what: &str) -> Result<(), Error> {
        if !self.singular(expression, bound) {
            let detail = if self.at_most_one(expression, bound) {
                "This value may be empty. Check presence with `some` before using it"
            } else {
                "This value may be empty or contain several integers"
            };
            return Err(Error::kind(format!(
                "{what} needs exactly one integer on each side. {detail}"
            )));
        }
        Ok(())
    }
    fn comparison_operand(
        &self,
        expression: &mut Expr,
        bound: &Bound,
        vars: bool,
    ) -> Result<(), Error> {
        if !self.expression(expression, bound, vars)?.numeric() {
            return Err(Error::kind("an integer comparison needs integers"));
        }
        self.integer(expression, bound, "an integer comparison")
    }
    fn sequence_type(
        &self,
        expression: &mut Expr,
        bound: &Bound,
        vars: bool,
    ) -> Result<&Sequence, Error> {
        let ty = self.expression(expression, bound, vars)?;
        if let Type::Set(name) = ty
            && self.singular(expression, bound)
            && let Some(seq) = self.model.sequence(&name)
        {
            return Ok(seq);
        }
        Err(Error::kind("expected one named sequence"))
    }

    fn variant_subject(
        &self,
        expression: &mut Expr,
        tag: &str,
        bound: &Bound,
        vars: bool,
    ) -> Result<(), Error> {
        let variant = self
            .model
            .case(tag)
            .ok_or_else(|| Error::kind(format!("unknown variant tag {tag}")))?
            .0;
        if self.expression(expression, bound, vars)? != Type::Set(variant.name.clone())
            || !self.singular(expression, bound)
        {
            return Err(Error::kind(format!("{tag} requires one {}", variant.name)));
        }
        Ok(())
    }

    fn value_of(
        &self,
        expression: &mut Expr,
        sort: &str,
        bound: &Bound,
        vars: bool,
    ) -> Result<(), Error> {
        let ty = self.expression(expression, bound, vars)?;
        if !self.fits_storage(&ty, &Type::Set(sort.into())) || !self.singular(expression, bound) {
            return Err(Error::kind(format!("expected one {sort}")));
        }
        Ok(())
    }

    fn expression(&self, expression: &mut Expr, bound: &Bound, vars: bool) -> Result<Type, Error> {
        let int = || Type::Set(INT.to_owned());
        Ok(match expression {
            Expr::None => Type::Any,
            Expr::Literal(value) => {
                if self.model.scope.ints.is_none() {
                    return Err(Error::kind("an integer value needs a scope Int = lo..hi"));
                }
                Type::Set(if *value >= 0 { NAT } else { INT }.to_owned())
            }
            Expr::Count(counted) => {
                self.expression(counted, bound, vars)?;
                int()
            }
            Expr::Arith(_, left, right) => {
                for side in [left, right] {
                    if !self.expression(side, bound, vars)?.numeric() {
                        return Err(Error::kind("arithmetic needs integers"));
                    }
                    self.integer(side, bound, "arithmetic")?;
                }
                int()
            }
            Expr::If(condition, yes, no) => {
                let mut branch = self.clone();
                branch.guard(condition, bound, vars)?;
                let yes = branch.expression(yes, bound, vars)?;
                let no = self.expression(no, bound, vars)?;
                self.same(yes, no, "conditional branches")?
            }
            Expr::Select(variable, sort, predicate) => {
                self.check_atom_sort(sort)?;
                self.bound_formula(variable, sort, predicate, bound, vars)?;
                Type::Set(sort.clone())
            }
            Expr::At(phase, inner) => {
                if !self.transition {
                    return Err(Error::kind(
                        "expected a state expression; before, after and prime are only allowed in a transition property, without nesting",
                    ));
                }
                let state = self.state_checker(*phase);
                let ty = state.expression(inner, bound, vars)?;
                if *phase == Phase::Before {
                    *expression = std::mem::replace(inner.as_mut(), Expr::None);
                }
                ty
            }
            Expr::Sequence(name, items) => {
                let seq = self
                    .model
                    .sequence(name)
                    .ok_or_else(|| Error::kind(format!("unknown sequence {name}")))?;
                if items.len() > seq.max {
                    return Err(Error::kind(format!(
                        "{name}: literal exceeds max {}",
                        seq.max
                    )));
                }
                for item in items {
                    self.value_of(item, &seq.element, bound, vars)?;
                }
                Type::Set(name.clone())
            }
            Expr::Append(seq, item) => {
                let seq = self.sequence_type(seq, bound, vars)?;
                self.value_of(item, &seq.element, bound, vars)?;
                Type::Set(seq.name.clone())
            }
            Expr::Tail(seq) => Type::Set(self.sequence_type(seq, bound, vars)?.name.clone()),
            Expr::Take(seq, count) => {
                let seq = self.sequence_type(seq, bound, vars)?;
                if !self.expression(count, bound, vars)?.numeric() || !self.singular(count, bound) {
                    return Err(Error::kind("take needs one integer count"));
                }
                Type::Set(seq.name.clone())
            }
            Expr::RemoveAt(seq, index) => {
                let seq = self.sequence_type(seq, bound, vars)?;
                if !self.expression(index, bound, vars)?.numeric() || !self.singular(index, bound) {
                    return Err(Error::kind("removeAt needs one integer index"));
                }
                Type::Set(seq.name.clone())
            }
            Expr::RemoveAll(seq, excluded) => {
                let seq = self.sequence_type(seq, bound, vars)?;
                let excluded = self.expression(excluded, bound, vars)?;
                self.same(Type::Set(seq.element.clone()), excluded, "removeAll")?;
                Type::Set(seq.name.clone())
            }
            Expr::Keys(relation) => match self.expression(relation, bound, vars)? {
                Type::Pair(from, _) => Type::Set(from),
                _ => return Err(Error::kind("keys needs one binary relation")),
            },
            Expr::Closure(edges) => match self.expression(edges, bound, vars)? {
                Type::Pair(from, to)
                    if self.sigs.contains(from.as_str())
                        && self.sigs.contains(to.as_str())
                        && !self.model.is_value(&from)
                        && !self.model.is_value(&to)
                        && self.compatible(&from, &to) =>
                {
                    Type::Pair(from, to)
                }
                _ => {
                    return Err(Error::kind(
                        "closure needs a binary relation on comparable atom sorts",
                    ));
                }
            },
            Expr::ReachableFrom(roots, edges) => {
                let roots = self.expression(roots, bound, vars)?;
                let edges = self.expression(edges, bound, vars)?;
                match edges {
                    Type::Pair(from, to)
                        if self.sigs.contains(from.as_str())
                            && self.sigs.contains(to.as_str())
                            && (from == to
                                || (self.compatible(&from, &to)
                                    && !self.model.is_value(&from)
                                    && !self.model.is_value(&to))) =>
                    {
                        let carrier = if self.model.subsort(&from, &to) {
                            &to
                        } else {
                            &from
                        };
                        if !self.fits(&roots, &Type::Set(carrier.clone())) {
                            return Err(Error::kind(
                                "reachableFrom roots must fit the graph's atom sort",
                            ));
                        }
                        self.same(roots, Type::Set(to), "reachableFrom")?
                    }
                    Type::Any
                        if matches!(&roots, Type::Any)
                            || matches!(&roots, Type::Set(sort) if self.sigs.contains(sort.as_str())) =>
                    {
                        roots
                    }
                    _ => {
                        return Err(Error::kind(
                            "reachableFrom needs atom roots and a relation on comparable atom sorts",
                        ));
                    }
                }
            }
            Expr::Elements(seq) => Type::Set(self.sequence_type(seq, bound, vars)?.element.clone()),
            Expr::Length(seq) => {
                self.sequence_type(seq, bound, vars)?;
                int()
            }
            Expr::Index(seq, index) => {
                let seq = self.sequence_type(seq, bound, vars)?;
                if !self.expression(index, bound, vars)?.numeric() || !self.singular(index, bound) {
                    return Err(Error::kind("at needs one integer index"));
                }
                Type::Set(seq.element.clone())
            }
            // Resolve an ordinary call after seeing all variant declarations.
            Expr::Variant(tag, payload)
                if self.source_calls
                    && matches!(tag.as_str(), "tail" | "elements")
                    && self.model.case(tag).is_none() =>
            {
                let seq = payload
                    .as_mut()
                    .ok_or_else(|| Error::kind(format!("{tag} needs one sequence argument")))?;
                let ty = self.sequence_type(seq, bound, vars)?;
                let arg = payload.take().expect("checked argument");
                let (lowered, sort) = if tag == "tail" {
                    (Expr::Tail(arg), &ty.name)
                } else {
                    (Expr::Elements(arg), &ty.element)
                };
                *expression = lowered;
                Type::Set(sort.clone())
            }
            Expr::Variant(tag, payload)
                if self.source_calls && tag == "keys" && self.model.case(tag).is_none() =>
            {
                let relation = payload
                    .take()
                    .ok_or_else(|| Error::kind("keys needs one binary relation"))?;
                *expression = Expr::Keys(relation);
                return self.expression(expression, bound, vars);
            }
            Expr::Variant(tag, payload)
                if self.source_calls && tag == "closure" && self.model.case(tag).is_none() =>
            {
                let edges = payload
                    .take()
                    .ok_or_else(|| Error::kind("closure needs one relation argument"))?;
                *expression = Expr::Closure(edges);
                return self.expression(expression, bound, vars);
            }
            Expr::Variant(tag, payload) => {
                let (variant, case) = self
                    .model
                    .case(tag)
                    .ok_or_else(|| Error::kind(format!("unknown variant tag {tag}")))?;
                match (payload, &case.payload) {
                    (Some(value), Some(sort)) => self.value_of(value, sort, bound, vars)?,
                    (None, None) => {}
                    _ => {
                        return Err(Error::kind(format!(
                            "{tag}: payload does not match its declaration"
                        )));
                    }
                }
                Type::Set(variant.name.clone())
            }
            Expr::Payload(value, tag) => {
                self.variant_subject(value, tag, bound, vars)?;
                let sort = self
                    .model
                    .case(tag)
                    .and_then(|(_, c)| c.payload.as_ref())
                    .ok_or_else(|| Error::kind(format!("{tag} has no payload")))?;
                Type::Set(sort.clone())
            }
            Expr::Record(name, values) => {
                let record = self
                    .model
                    .record(name)
                    .ok_or_else(|| Error::kind(format!("unknown record {name}")))?;
                if values.len() != record.fields.len() {
                    return Err(Error::kind(format!(
                        "{name}: expected {} fields",
                        record.fields.len()
                    )));
                }
                for (value, (field, sort)) in values.iter_mut().zip(&record.fields) {
                    let found = self.expression(value, bound, vars)?;
                    if !self.fits_storage(&found, &Type::Set(sort.clone()))
                        || !self.singular(value, bound)
                    {
                        return Err(Error::kind(format!(
                            "{name}.{field} needs one value of {sort}"
                        )));
                    }
                }
                Type::Set(name.clone())
            }
            Expr::Field(receiver, field) => {
                let ty = self.expression(receiver, bound, vars)?;
                self.field_projection(&ty, field, bound)
                    .ok_or_else(|| Error::kind(format!("{} has no field {field}", ty.describe())))?
            }
            Expr::Name(name) => {
                if let Some(renamed) = self.expanded_binders.as_ref().and_then(|map| map.get(name))
                {
                    name.clone_from(renamed);
                }
                if let Some((value, remaining, site)) = self.binding(name) {
                    if matches!(site, BindingSite::Transition) && !self.transition {
                        return Err(Error::kind(format!(
                            "transition binding {name} must be used outside before/after"
                        )));
                    }
                    remaining.set(
                        remaining
                            .get()
                            .checked_sub(value.nodes)
                            .ok_or_else(|| Error::syntax(site.expansion_limit()))?,
                    );
                    *expression = value.expression.clone();
                    // Resolve the expanded expression in the same scope as the wire IR.
                    return Checker {
                        local_bindings: None,
                        derived_bindings: None,
                        expanded_binders: Some(BTreeMap::new()),
                        ..self.clone()
                    }
                    .expression(expression, bound, vars);
                } else if let Some(sort) = bound.get(name) {
                    Type::Set(sort.clone())
                } else if self.model.is_value(name) {
                    return Err(Error::kind(format!(
                        "{name} is a sort of values, not a set to use in an expression"
                    )));
                } else if self.sigs.contains(name.as_str()) {
                    Type::Set(name.clone())
                } else if let Some(enumeration) = self.model.member(name) {
                    Type::Set(enumeration.to_owned())
                } else if let Some(relation) = self.model.relation(name) {
                    if relation.var && !vars {
                        return Err(Error::kind(format!("fact refers to the var {name}")));
                    }
                    if relation.var
                        && let Some(location) = self.read_location
                    {
                        location.record(name);
                    }
                    Self::shape(&relation.shape)
                } else {
                    return Err(Error::kind(format!("unknown name {name}")));
                }
            }
            Expr::Binary(operator, left_expression, right_expression) => {
                let left = self.expression(left_expression, bound, vars)?;
                if *operator == SetOp::Join
                    && let Expr::Name(field) = right_expression.as_ref()
                    && let Some(projected) = self.field_projection(&left, field, bound)
                {
                    let receiver = std::mem::replace(left_expression.as_mut(), Expr::None);
                    *expression = Expr::Field(Box::new(receiver), field.clone());
                    return Ok(projected);
                }
                let right = self.expression(right_expression, bound, vars)?;
                // `+`, `-` and `*` between integers are arithmetic (plan 0103).
                let arith = match operator {
                    SetOp::Union => Some(Arith::Add),
                    SetOp::Difference => Some(Arith::Sub),
                    SetOp::Multiply => Some(Arith::Mul),
                    _ => None,
                };
                if let Some(arith) = arith
                    && left.numeric()
                    && right.numeric()
                {
                    self.integer(left_expression, bound, "arithmetic")?;
                    self.integer(right_expression, bound, "arithmetic")?;
                    let (l, r) = (
                        std::mem::replace(left_expression.as_mut(), Expr::None),
                        std::mem::replace(right_expression.as_mut(), Expr::None),
                    );
                    *expression = Expr::Arith(arith, Box::new(l), Box::new(r));
                    return Ok(int());
                }
                if *operator == SetOp::Multiply {
                    return Err(Error::kind("* needs integers"));
                }
                match operator {
                    SetOp::Intersection => self.intersection(left, right)?,
                    SetOp::WithoutKeys => match left {
                        Type::Pair(from, to) => {
                            self.same(Type::Set(from.clone()), right, "withoutKeys")?;
                            Type::Pair(from, to)
                        }
                        Type::Any if matches!(right, Type::Set(_) | Type::Any) => Type::Any,
                        _ => {
                            return Err(Error::kind(
                                "withoutKeys needs a binary relation and a set of keys",
                            ));
                        }
                    },
                    SetOp::Override => {
                        let relation = self.same(left, right, "override")?;
                        if !matches!(relation, Type::Pair(..)) {
                            return Err(Error::kind("override needs a binary relation"));
                        }
                        relation
                    }
                    SetOp::Product => match (left, right) {
                        (Type::Pair(..), _) | (_, Type::Pair(..)) => {
                            return Err(Error::kind("product above arity two"));
                        }
                        (Type::Set(a), Type::Set(b)) => Type::Pair(a, b),
                        _ => Type::Any,
                    },
                    SetOp::Join => {
                        let mismatch = |l: &str, r: &str| {
                            Err(Error::kind(format!("join of different sorts: {l} and {r}")))
                        };
                        match (left, right) {
                            (Type::Set(a), Type::Pair(b, c)) => {
                                if !self.compatible(&a, &b) {
                                    return mismatch(&a, &b);
                                }
                                Type::Set(c)
                            }
                            (Type::Pair(a, b), Type::Set(c)) => {
                                if !self.compatible(&b, &c) {
                                    return mismatch(&b, &c);
                                }
                                Type::Set(a)
                            }
                            (Type::Pair(a, b), Type::Pair(c, d)) => {
                                if !self.compatible(&b, &c) {
                                    return mismatch(&b, &c);
                                }
                                Type::Pair(a, d)
                            }
                            (Type::Any, Type::Pair(..)) | (Type::Pair(..), Type::Any) => Type::Any,
                            _ => return Err(Error::kind("join needs a binary relation")),
                        }
                    }
                    _ => self.same(left, right, "set operation")?,
                }
            }
        })
    }
    fn field_projection(&self, receiver: &Type, field: &str, bound: &Bound) -> Option<Type> {
        let field_sort = |sort| {
            let record = self.model.record(sort)?;
            let index = record.field_index(field)?;
            Some(record.fields[index].1.clone())
        };
        match receiver {
            Type::Set(sort) => Some(Type::Set(field_sort(sort)?)),
            Type::Pair(key, sort) => Some(Type::Pair(key.clone(), field_sort(sort)?)),
            Type::Any => (!bound.contains_key(field)
                && self.binding(field).is_none()
                && !self.sigs.contains(field)
                && self.model.member(field).is_none()
                && self.model.relation(field).is_none()
                && self.model.field(field).is_some())
            .then_some(Type::Any),
        }
    }

    fn formula(&self, formula: &mut Formula, bound: &Bound, vars: bool) -> Result<(), Error> {
        match formula {
            Formula::Compare(operator, left, right) => {
                let truth = *operator == Compare::Equal
                    && matches!(right, Expr::Name(name) if name == "true");
                let left = self.expression(left, bound, vars)?;
                if truth && left != Type::Set(BOOL.to_owned()) {
                    return Err(Error::kind(format!(
                        "expected a comparison, or an expression of Bool; got {}",
                        left.describe()
                    )));
                }
                let right = self.expression(right, bound, vars)?;
                self.same(left, right, "comparison")?;
            }
            Formula::Ints(_, left, right) => {
                let counts = matches!(left, Int::Count(_)) || matches!(right, Int::Count(_));
                for int in [left, right] {
                    match int {
                        Int::Count(expression) => {
                            self.expression(expression, bound, vars)?;
                        }
                        Int::Literal(_) => {
                            if !counts && self.model.scope.ints.is_none() {
                                return Err(Error::kind(
                                    "an integer value needs a scope Int = lo..hi",
                                ));
                            }
                        }
                        Int::Value(expression) => {
                            self.comparison_operand(expression, bound, vars)?
                        }
                        Int::LocatedValue(expression, offset) => {
                            if let Err(error) = self.comparison_operand(expression, bound, vars) {
                                if self.operand_error.get().is_none() {
                                    self.operand_error.set(Some(*offset));
                                }
                                return Err(error);
                            }
                            *int = Int::Value(std::mem::replace(expression, Expr::None));
                        }
                    }
                }
            }
            Formula::HasTag(value, tag) => self.variant_subject(value, tag, bound, vars)?,
            Formula::Mult(_, expression) | Formula::Empty(expression) => {
                self.expression(expression, bound, vars)?;
            }
            Formula::Not(inner) => self.formula(inner, bound, vars)?,
            Formula::And(_) => self.clone().guard(formula, bound, vars)?,
            Formula::Or(left, right) => {
                self.formula(left, bound, vars)?;
                self.formula(right, bound, vars)?;
            }
            Formula::Implies(left, right) => {
                let mut branch = self.clone();
                branch.guard(left, bound, vars)?;
                branch.formula(right, bound, vars)?;
            }
            Formula::Quantified(_, variable, sig, body) => {
                if numeric_sort(sig) {
                    let (_, high) = self
                        .model
                        .scope
                        .ints
                        .ok_or_else(|| Error::kind("Int and Nat need a scope Int = lo..hi"))?;
                    if sig == NAT && high < 0 {
                        return Err(Error::kind(
                            "the Int scope needs a nonnegative value for Nat",
                        ));
                    }
                } else {
                    self.check_atom_sort(sig)?;
                }
                self.bound_formula(variable, sig, body, bound, vars)?;
            }
        }
        Ok(())
    }
    fn check_atom_sort(&self, sig: &str) -> Result<(), Error> {
        if self.model.is_value(sig) {
            return Err(Error::kind(format!(
                "quantifying over {sig}: only atoms are quantified, values have no fixed set"
            )));
        }
        if !self.sigs.contains(sig) {
            return Err(Error::kind(format!("unknown sig {sig}")));
        }
        Ok(())
    }
    fn bound_formula(
        &self,
        variable: &mut String,
        sort: &str,
        body: &mut Formula,
        bound: &Bound,
        vars: bool,
    ) -> Result<(), Error> {
        let derived_name = |name: &str| {
            self.model
                .derivations
                .iter()
                .any(|(declared, _)| declared == name)
        };
        let mut scoped;
        let checker = if self.expanded_binders.is_some()
            && (bound.contains_key(variable) || derived_name(variable))
        {
            let name = (0..)
                .map(|index| format!("_rmodel_bound_{index}"))
                .find(|name| self.fresh(name, bound).is_ok() && !derived_name(name))
                .expect("infinitely many possible binding names");
            scoped = self.clone();
            scoped
                .expanded_binders
                .as_mut()
                .expect("checking an expansion")
                .insert(std::mem::replace(variable, name.clone()), name);
            &scoped
        } else {
            self.fresh(variable, bound)?;
            self
        };
        let mut inner = bound.clone();
        inner.insert(variable.clone(), sort.to_owned());
        checker.formula(body, &inner, vars)
    }
    fn fresh(&self, name: &str, bound: &Bound) -> Result<(), Error> {
        if bound.contains_key(name)
            || self.binding(name).is_some()
            || self.sigs.contains(name)
            || self.model.member(name).is_some()
            || self.model.relation(name).is_some()
        {
            return Err(Error::kind(format!("{name} shadows another name")));
        }
        Ok(())
    }
}

pub(crate) fn parse(
    source: &str,
    track_locations: bool,
) -> Result<crate::LocatedModel, ParseDiagnostic> {
    if source.len() > 1 << 20 {
        return Err(ParseDiagnostic {
            error: Error::syntax("source exceeds 1 MiB"),
            offset: None,
        });
    }
    let (tokens, offsets) = tokens(source)?;
    let boolean = tokens
        .iter()
        .any(|token| [BOOL, "true", "false"].contains(&token.as_str()));
    let mut parser = Parser {
        tokens,
        offsets,
        locations: Locations::default(),
        track_locations,
        uses_bool: boolean,
        position: 0,
        depth: 0,
        nodes: 0,
        records: BTreeMap::new(),
        newtypes: BTreeSet::new(),
    };
    let mut model = parser.model().map_err(|error| ParseDiagnostic {
        error,
        offset: Some(
            parser
                .offsets
                .get(parser.position)
                .copied()
                .unwrap_or_else(|| source.chars().count()),
        ),
    })?;
    if parser.uses_bool {
        model
            .enums
            .push((BOOL.to_owned(), vec!["false".into(), "true".into()]));
    }
    let state_reads = StateReads::default();
    check(
        &mut model,
        true,
        &mut parser.locations,
        track_locations.then_some(&state_reads),
    )
    .map_err(|error| ParseDiagnostic {
        error,
        offset: parser
            .locations
            .operand_error
            .get()
            .or(parser.locations.current.get()),
    })?;
    Ok(crate::LocatedModel {
        model,
        declarations: parser.locations.named,
        term_locations: parser.locations.terms,
        state_reads: state_reads.into_inner(),
        update_branches: parser.locations.update_branches,
    })
}

/// Parse an observation predicate in the same environment as an action-scoped transition.
pub fn transition_predicate(
    model: &Model,
    action: &Action,
    source: &str,
) -> Result<Formula, Error> {
    if source.len() > 1 << 20 {
        return Err(Error::syntax("observation predicate exceeds 1 MiB"));
    }
    let (tokens, offsets) = tokens(source).map_err(|diagnostic| diagnostic.error)?;
    let mut parser = Parser {
        tokens,
        offsets,
        locations: Locations::default(),
        track_locations: false,
        uses_bool: false,
        position: 0,
        depth: 0,
        nodes: 0,
        records: model
            .records
            .iter()
            .map(|record| {
                (
                    record.name.clone(),
                    record.fields.iter().map(|(name, _)| name.clone()).collect(),
                )
            })
            .collect(),
        newtypes: model
            .records
            .iter()
            .filter(|record| record.data == Some(DataKind::Newtype))
            .map(|record| record.name.clone())
            .collect(),
    };
    let mut formula = parser.formula()?;
    if parser.position != parser.tokens.len() {
        return Err(Error::syntax(format!(
            "unexpected token {:?} after observation predicate",
            parser.peek()
        )));
    }
    let sigs = model
        .sigs
        .iter()
        .map(String::as_str)
        .chain(model.enums.iter().map(|(name, _)| name.as_str()))
        .chain(model.sums.iter().map(|(name, _)| name.as_str()))
        .chain(
            model
                .value_domains()?
                .into_iter()
                .map(|value| value.kind.name()),
        )
        .chain([INT, NAT])
        .collect();
    let remaining = Cell::new(MAX_NODES * 16);
    let derived = model
        .derivations
        .iter()
        .map(|(name, expression)| {
            Ok((
                name.clone(),
                BindingValue {
                    nodes: binding_value_nodes(expression, "derivation")?,
                    expression: expression.clone(),
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, Error>>()?;
    let checker = Checker {
        model,
        sigs: &sigs,
        transition: true,
        source_calls: true,
        singletons: Vec::new(),
        local_bindings: None,
        derived_bindings: Some((&derived, &remaining)),
        expanded_binders: None,
        read_location: None,
        operand_error: &parser.locations.operand_error,
    };
    checker.formula(&mut formula, &action.params.iter().cloned().collect(), true)?;
    inspect_formula(&formula, &mut 0, 0, &mut BTreeSet::new()).map_err(|_| {
        Error::syntax("expanded observation predicate exceeds the parser's size limits")
    })?;
    crate::numeric::validate_predicate(model, &formula)?;
    Ok(formula)
}

fn inspect_expression(
    e: &Expr,
    nodes: &mut usize,
    depth: usize,
    names: &mut BTreeSet<String>,
) -> Result<(), Error> {
    *nodes += 1;
    if *nodes > MAX_NODES * 16 || depth > MAX_DEPTH {
        return Err(Error::syntax("the IR exceeds the parser's size limits"));
    }
    match e {
        Expr::Name(name) => {
            names.insert(name.clone());
        }
        Expr::Field(receiver, field) => {
            names.insert(field.clone());
            inspect_expression(receiver, nodes, depth + 1, names)?;
        }
        Expr::None | Expr::Literal(_) => {}
        Expr::Tail(inner)
        | Expr::Keys(inner)
        | Expr::Closure(inner)
        | Expr::Length(inner)
        | Expr::Elements(inner)
        | Expr::Payload(inner, _)
        | Expr::At(_, inner) => inspect_expression(inner, nodes, depth + 1, names)?,
        Expr::Append(left, right)
        | Expr::Take(left, right)
        | Expr::RemoveAt(left, right)
        | Expr::RemoveAll(left, right)
        | Expr::ReachableFrom(left, right)
        | Expr::Index(left, right) => {
            inspect_expression(left, nodes, depth + 1, names)?;
            inspect_expression(right, nodes, depth + 1, names)?;
        }
        Expr::Sequence(_, items) => {
            for item in items {
                inspect_expression(item, nodes, depth + 1, names)?;
            }
        }
        Expr::Variant(_, payload) => {
            if let Some(value) = payload {
                inspect_expression(value, nodes, depth + 1, names)?;
            }
        }
        Expr::If(condition, yes, no) => {
            inspect_formula(condition, nodes, depth + 1, names)?;
            inspect_expression(yes, nodes, depth + 1, names)?;
            inspect_expression(no, nodes, depth + 1, names)?;
        }
        Expr::Select(_, sort, predicate) => {
            names.insert(sort.clone());
            inspect_formula(predicate, nodes, depth + 1, names)?;
        }
        Expr::Count(inner) => inspect_expression(inner, nodes, depth + 1, names)?,
        Expr::Binary(_, l, r) | Expr::Arith(_, l, r) => {
            inspect_expression(l, nodes, depth + 1, names)?;
            inspect_expression(r, nodes, depth + 1, names)?;
        }
        Expr::Record(name, values) => {
            names.insert(name.clone());
            for v in values {
                inspect_expression(v, nodes, depth + 1, names)?;
            }
        }
    }
    Ok(())
}
fn inspect_formula(
    f: &Formula,
    nodes: &mut usize,
    depth: usize,
    names: &mut BTreeSet<String>,
) -> Result<(), Error> {
    *nodes += 1;
    if *nodes > MAX_NODES * 16 || depth > MAX_DEPTH {
        return Err(Error::syntax("the IR exceeds the parser's size limits"));
    }
    let int = |i: &Int, nodes: &mut usize, names: &mut BTreeSet<String>| match i {
        Int::Count(e) | Int::Value(e) | Int::LocatedValue(e, _) => {
            inspect_expression(e, nodes, depth + 1, names)
        }
        Int::Literal(_) => Ok(()),
    };
    match f {
        Formula::Compare(_, l, r) => {
            inspect_expression(l, nodes, depth + 1, names)?;
            inspect_expression(r, nodes, depth + 1, names)?;
        }
        Formula::Ints(_, l, r) => {
            int(l, nodes, names)?;
            int(r, nodes, names)?;
        }
        Formula::Mult(_, e) | Formula::Empty(e) | Formula::HasTag(e, _) => {
            inspect_expression(e, nodes, depth + 1, names)?
        }
        Formula::Not(inner) => inspect_formula(inner, nodes, depth + 1, names)?,
        Formula::And(parts) => {
            for p in parts {
                inspect_formula(p, nodes, depth + 1, names)?;
            }
        }
        Formula::Or(l, r) | Formula::Implies(l, r) => {
            inspect_formula(l, nodes, depth + 1, names)?;
            inspect_formula(r, nodes, depth + 1, names)?;
        }
        Formula::Quantified(_, _, sort, body) => {
            names.insert(sort.clone());
            inspect_formula(body, nodes, depth + 1, names)?;
        }
    }
    Ok(())
}

fn inspect_tree(model: &Model) -> Result<BTreeSet<String>, Error> {
    let mut names = BTreeSet::new();
    let mut nodes = 0usize;
    for relation in &model.relations {
        match &relation.shape {
            Shape::Unary { sig, .. } => names.insert(sig.clone()),
            Shape::Binary { from, to, .. } => names.insert(from.clone()) | names.insert(to.clone()),
        };
    }
    for record in &model.records {
        names.extend(record.fields.iter().map(|(_, sort)| sort.clone()));
    }
    names.extend(model.sequences.iter().map(|seq| seq.element.clone()));
    names.extend(
        model
            .variants
            .iter()
            .flat_map(|variant| variant.cases.iter().filter_map(|case| case.payload.clone())),
    );
    for (_, expression) in &model.derivations {
        inspect_expression(expression, &mut nodes, 0, &mut names)?;
    }
    for f in model
        .facts
        .iter()
        .chain(&model.init)
        .chain(model.checks.iter().map(|c| &c.formula))
        .chain(
            model
                .temporal
                .iter()
                .flat_map(|c| std::iter::once(&c.formula).chain(&c.trigger)),
        )
        .chain(model.transitions.iter().map(|check| &check.formula))
    {
        inspect_formula(f, &mut nodes, 0, &mut names)?;
    }
    for action in &model.actions {
        names.extend(action.params.iter().map(|(_, sort)| sort.clone()));
        for g in &action.guards {
            inspect_formula(g, &mut nodes, 0, &mut names)?;
        }
        for (_, value) in &action.assignments {
            inspect_expression(value, &mut nodes, 0, &mut names)?;
        }
    }
    Ok(names)
}

pub(crate) fn may_read_names(formula: &Formula, candidates: &BTreeSet<&str>) -> bool {
    let mut names = BTreeSet::new();
    inspect_formula(formula, &mut 0, 0, &mut names).is_err()
        || names.iter().any(|name| candidates.contains(name.as_str()))
}

/// Type-checks a syntax tree built without the parser, such as one read
/// from the JSON IR (plan 0108), with the parser's limits on its size.
/// Bool is added when the tree mentions it, as the parser does.
pub(crate) fn validate(mut model: Model) -> Result<Model, Error> {
    let names = inspect_tree(&model)?;
    let boolean = [BOOL, "true", "false"].iter().any(|n| names.contains(*n));
    if boolean && !model.enums.iter().any(|(name, _)| name == BOOL) {
        model
            .enums
            .push((BOOL.to_owned(), vec!["false".into(), "true".into()]));
    }
    if model.scope.steps == Some(0) {
        return Err(Error::syntax("steps must be positive"));
    }
    for (name, &size) in &model.scope.sizes {
        if !(1..=MAX_SCOPE).contains(&size) {
            return Err(Error::syntax(format!(
                "{name}: size {size} outside 1..={MAX_SCOPE}"
            )));
        }
    }
    if let Some((low, high)) = model.scope.ints
        && !valid_int_scope(low, high)
    {
        return Err(Error::syntax(format!(
            "Int = {low}..{high}: at most 20 consecutive integers"
        )));
    }
    check(&mut model, false, &mut Locations::default(), None)?;
    Ok(model)
}

fn binding_value_nodes(expression: &Expr, context: &str) -> Result<usize, Error> {
    let mut nodes = 0;
    inspect_expression(expression, &mut nodes, 0, &mut BTreeSet::new()).map_err(|_| {
        Error::syntax(format!(
            "expanded {context} exceeds the parser's size limits"
        ))
    })?;
    Ok(nodes)
}

fn check(
    model: &mut Model,
    source_calls: bool,
    locations: &mut Locations,
    state_reads: Option<&StateReads>,
) -> Result<(), Error> {
    // The checker reads a snapshot and rewrites arithmetic in the model.
    let snapshot = model.clone();
    let has_derivations = !model.derivations.is_empty();
    check_in(&snapshot, model, source_calls, locations, state_reads)?;
    locations.current.set(None);
    if has_derivations {
        inspect_tree(model)
            .map_err(|_| Error::syntax("expanded derivations exceed the parser's size limits"))?;
    }
    crate::numeric::validate(model, i64::MIN..=i64::MAX)
}

fn check_in(
    model: &Model,
    target: &mut Model,
    source_calls: bool,
    locations: &mut Locations,
    state_reads: Option<&StateReads>,
) -> Result<(), Error> {
    let definitions = std::mem::take(&mut target.derivations);
    let mut names = BTreeSet::new();
    for name in model
        .sigs
        .iter()
        .chain(
            model
                .enums
                .iter()
                .flat_map(|(name, members)| std::iter::once(name).chain(members)),
        )
        .chain(model.sums.iter().map(|(name, _)| name))
        .chain(model.records.iter().map(|record| &record.name))
        .chain(model.sequences.iter().map(|seq| &seq.name))
        .chain(
            model
                .variants
                .iter()
                .flat_map(|v| std::iter::once(&v.name).chain(v.cases.iter().map(|c| &c.tag))),
        )
        .chain(model.relations.iter().map(|relation| &relation.name))
        .chain(model.actions.iter().map(|action| &action.name))
        .chain(model.checks.iter().map(|check| &check.name))
        .chain(model.temporal.iter().map(|check| &check.name))
        .chain(model.transitions.iter().map(|check| &check.name))
        .chain(definitions.iter().map(|(name, _)| name))
    {
        locations.enter(name);
        if !names.insert(name.as_str()) {
            return Err(Error::kind(format!("duplicate name {name}")));
        }
        if numeric_sort(name) || name == crate::SCOPE {
            return Err(Error::kind(format!("{name} is a built-in name")));
        }
        if name == crate::DECLARATIONS {
            return Err(Error::kind(format!(
                "{name} is the built-in multiplicity check"
            )));
        }
    }
    for record in &model.records {
        locations.enter(&record.name);
        crate::refinement::validate(model, record)?;
        match record.data {
            Some(DataKind::Newtype) if !matches!(record.fields.as_slice(), [(field, _)] if field == "value") =>
            {
                return Err(Error::kind(format!(
                    "{}: a newtype needs exactly one value field",
                    record.name
                )));
            }
            Some(DataKind::Record) if record.fields.is_empty() => {
                return Err(Error::kind(format!(
                    "{}: data record needs at least one field",
                    record.name
                )));
            }
            _ => {}
        }
        let mut fields = BTreeSet::new();
        for (field, _) in &record.fields {
            if !fields.insert(field) {
                return Err(Error::kind(format!(
                    "{}: duplicate field {field}",
                    record.name
                )));
            }
            if numeric_sort(field)
                || matches!(field.as_str(), BOOL | crate::SCOPE | crate::DECLARATIONS)
            {
                return Err(Error::kind(format!(
                    "{}: {field} is a built-in name",
                    record.name
                )));
            }
        }
    }
    let sigs: BTreeSet<&str> = model.sigs.iter().map(String::as_str).collect();
    for (name, definition) in &model.entity_definitions {
        locations.enter(name);
        if !sigs.contains(name.as_str()) {
            return Err(Error::kind(format!("definition for unknown entity {name}")));
        }
        if !valid_definition(definition) {
            return Err(Error::kind(format!(
                "{name}: expected a nonempty description with printable text"
            )));
        }
    }
    for (name, translations) in &model.entity_translations {
        locations.enter(name);
        if !sigs.contains(name.as_str()) {
            return Err(Error::kind(format!(
                "description for unknown entity {name}"
            )));
        }
        if translations.is_empty() {
            return Err(Error::kind(format!(
                "{name}: expected at least one translation"
            )));
        }
        for (tag, text) in translations {
            if tag == "default" || !valid_language_tag(tag) {
                return Err(Error::kind(format!("{name}: invalid language tag {tag:?}")));
            }
            if !valid_definition(text) {
                return Err(Error::kind(format!(
                    "{name}: expected a nonempty description with printable text for {tag}"
                )));
            }
        }
    }
    for (target, id) in &model.term_bindings {
        locations.current.set(locations.terms.get(target).copied());
        if !valid_term_id(id) {
            return Err(Error::kind(format!("invalid term ID {id:?}")));
        }
        if model.term_target_kind(target).is_none() {
            return Err(Error::kind(format!("unknown term target {target}")));
        }
    }
    for sig in &model.sigs {
        locations.enter(sig);
        if !model.scope.sizes.contains_key(sig) {
            return Err(Error::kind(format!("no scope for {sig}")));
        }
    }
    for sig in model.scope.sizes.keys() {
        locations.current.set(locations.scope);
        if !sigs.contains(sig.as_str()) {
            return Err(Error::kind(format!("scope for unknown sig {sig}")));
        }
    }
    // Enums and sums are sorts for typing, without a scope.
    let sigs: BTreeSet<&str> = sigs
        .into_iter()
        .chain(model.enums.iter().map(|(name, _)| name.as_str()))
        .collect();
    // A sum's children are signatures or enums, each in one sum at most
    // (plan 0100).
    let mut owned = BTreeSet::new();
    for (sum, children) in &model.sums {
        locations.enter(sum);
        for child in children {
            if !sigs.contains(child.as_str()) {
                return Err(Error::kind(format!(
                    "sum sort {sum}: {child} is not a signature or enum"
                )));
            }
            if !owned.insert(child.as_str()) {
                return Err(Error::kind(format!("{child} belongs to two sum sorts")));
            }
        }
    }
    // Values (plan 0103): Int with its scope, and records whose fields are
    // declared sorts or earlier records.
    let mut sigs: BTreeSet<&str> = sigs
        .into_iter()
        .chain(model.sums.iter().map(|(name, _)| name.as_str()))
        .collect();
    locations.current.set(locations.scope);
    if model.declared_sorts().any(numeric_sort) && model.scope.ints.is_none() {
        return Err(Error::kind("Int and Nat need a scope Int = lo..hi"));
    }
    if model.uses_nat() && model.scope.ints.is_some_and(|(_, high)| high < 0) {
        return Err(Error::kind(
            "the Int scope needs a nonnegative value for Nat",
        ));
    }
    sigs.extend([INT, NAT]);
    locations.current.set(None);
    for value in model.value_domains()? {
        sigs.insert(value.kind.name());
    }
    for relation in &model.relations {
        locations.enter(&relation.name);
        let used = match &relation.shape {
            Shape::Unary { sig, .. } => vec![sig],
            Shape::Binary { from, to, .. } => vec![from, to],
        };
        if let Some(sig) = used.iter().find(|sig| !sigs.contains(sig.as_str())) {
            return Err(Error::kind(format!("{}: unknown sig {sig}", relation.name)));
        }
        // Unary integer sets use the finite Int scope. Value-bearing rows
        // and structured values retain their one/lone multiplicity.
        let (target, mult) = match &relation.shape {
            Shape::Unary { sig, mult } => (sig, *mult),
            Shape::Binary { from, to, mult } => {
                if model.is_value(from) {
                    return Err(Error::kind(format!(
                        "{}: a relation starts at atoms, not at the values of {from}",
                        relation.name
                    )));
                }
                (to, *mult)
            }
        };
        let integer_set = matches!(&relation.shape, Shape::Unary { sig, .. } if numeric_sort(sig));
        if model.is_value(target) && !integer_set && !matches!(mult, Mult::One | Mult::Lone) {
            return Err(Error::kind(format!(
                "{}: a relation to {target} holds one or lone value, not a set",
                relation.name
            )));
        }
    }
    let remaining = Cell::new(MAX_NODES * 16);
    let mut derived = BTreeMap::new();
    let mut checker = Checker {
        model,
        sigs: &sigs,
        transition: false,
        source_calls,
        singletons: Vec::new(),
        local_bindings: None,
        derived_bindings: None,
        expanded_binders: None,
        read_location: state_reads.map(|reads| ReadLocation {
            offset: &locations.current,
            reads,
        }),
        operand_error: &locations.operand_error,
    };
    let none = Bound::new();
    for (name, mut expression) in definitions {
        locations.enter(&name);
        let scoped = Checker {
            derived_bindings: Some((&derived, &remaining)),
            ..checker.clone()
        };
        scoped.expression(&mut expression, &none, true)?;
        let nodes = binding_value_nodes(&expression, "derivation")?;
        crate::numeric::validate_binding(model, &expression)?;
        target.derivations.push((name.clone(), expression.clone()));
        derived.insert(name, BindingValue { expression, nodes });
    }
    if !derived.is_empty() {
        checker.derived_bindings = Some((&derived, &remaining));
    }
    for (index, fact) in target.facts.iter_mut().enumerate() {
        locations.current.set(locations.facts.get(index).copied());
        checker.formula(fact, &none, false)?;
    }
    for (index, formula) in target.init.iter_mut().enumerate() {
        locations.current.set(locations.init.get(index).copied());
        checker.formula(formula, &none, true)?;
    }
    for check in &mut target.checks {
        locations.enter(&check.name);
        checker.formula(&mut check.formula, &none, true)?;
    }
    for variant in &model.variants {
        locations.enter(&variant.name);
        for case in &variant.cases {
            if matches!(
                case.tag.as_str(),
                "is" | "at" | "len" | "append" | "payload"
            ) {
                return Err(Error::kind(format!(
                    "{} is reserved for a value operation",
                    case.tag
                )));
            }
        }
    }
    for check in &mut target.temporal {
        locations.enter(&check.name);
        let mut bound = Bound::new();
        for (name, sort) in &check.params {
            if !checker.sigs.contains(sort.as_str()) || model.is_value(sort) {
                return Err(Error::kind(format!(
                    "{}: temporal parameter {name} needs an atom sort",
                    check.name
                )));
            }
            checker.fresh(name, &bound)?;
            bound.insert(name.clone(), sort.clone());
        }
        checker.formula(&mut check.formula, &bound, true)?;
        if let Some(trigger) = &mut check.trigger {
            checker.formula(trigger, &bound, true)?;
        }
    }
    let transition_checker = Checker {
        transition: true,
        ..checker.clone()
    };
    for check in &mut target.transitions {
        locations.enter(&check.name);
        let bound = match &check.action {
            Some(name) => model
                .actions
                .iter()
                .find(|action| &action.name == name)
                .ok_or_else(|| Error::kind(format!("{}: unknown action {name}", check.name)))?
                .params
                .iter()
                .cloned()
                .collect(),
            None => Bound::new(),
        };
        let remaining = Cell::new(0);
        let locals = transition_checker.local_values(
            locations.bindings.remove(&check.name),
            &bound,
            &remaining,
            BindingSite::Transition,
        )?;
        let scoped = Checker {
            local_bindings: Some(((&locals, &remaining), BindingSite::Transition)),
            ..transition_checker.clone()
        };
        scoped.formula(&mut check.formula, &bound, true)?;
        if !locals.is_empty() {
            inspect_formula(&check.formula, &mut 0, 0, &mut BTreeSet::new()).map_err(|_| {
                Error::syntax("expanded transition formula exceeds the parser's size limits")
            })?;
        }
    }
    locations.current.set(None);
    for rules in std::iter::once(&model.fairness).chain(
        model
            .temporal
            .iter()
            .filter_map(|check| check.fairness.as_ref()),
    ) {
        let mut fair_actions = BTreeSet::new();
        for fair in rules {
            if !model
                .actions
                .iter()
                .any(|action| action.name == fair.action)
            {
                return Err(Error::kind(format!(
                    "fairness refers to unknown action {}",
                    fair.action
                )));
            }
            if !fair_actions.insert(&fair.action) {
                return Err(Error::kind(format!(
                    "duplicate fairness for {}",
                    fair.action
                )));
            }
        }
    }
    for (action, written) in model.actions.iter().zip(&mut target.actions) {
        locations.enter(&action.name);
        let mut checker = checker.clone();
        let mut bound = Bound::new();
        for (param, sig) in &action.params {
            if !checker.sigs.contains(sig.as_str()) {
                return Err(Error::kind(format!("{}: unknown sig {sig}", action.name)));
            }
            checker.fresh(param, &bound)?;
            bound.insert(param.clone(), sig.clone());
        }
        for guard in &mut written.guards {
            checker.guard(guard, &bound, true)?;
        }
        let remaining = Cell::new(0);
        let definitions = locations.bindings.remove(&action.name);
        let binding_branches = locations.binding_branches.remove(&action.name);
        let source_definitions = binding_branches
            .as_ref()
            .and(definitions.as_ref())
            .map(|definitions| definitions.definitions.clone());
        let locals = checker.local_values(definitions, &bound, &remaining, BindingSite::Update)?;
        if !locals.is_empty() {
            checker.local_bindings = Some(((&locals, &remaining), BindingSite::Update));
        }
        let mut assigned = BTreeSet::new();
        for (var, value) in written.assignments.iter_mut() {
            let var = &*var;
            let relation = model
                .relation(var)
                .filter(|relation| relation.var)
                .ok_or_else(|| Error::kind(format!("{}: {var} is not a var", action.name)))?;
            if !assigned.insert(var) {
                return Err(Error::kind(format!(
                    "{}: {var} assigned twice",
                    action.name
                )));
            }
            let value = checker.expression(value, &bound, true)?;
            let expected = Checker::shape(&relation.shape);
            if !checker.fits_storage(&value, &expected) {
                let what = if value.arity() == expected.arity() {
                    format!(
                        "another sort ({} for {})",
                        value.describe(),
                        expected.describe()
                    )
                } else {
                    "another arity".to_owned()
                };
                return Err(Error::kind(format!(
                    "{}: {var} assigned a value of {what}",
                    action.name
                )));
            }
        }
        if !locals.is_empty() {
            for (_, value) in &written.assignments {
                binding_value_nodes(value, "update expression")?;
            }
        }
        if let (Some(definitions), Some(mut bindings)) = (source_definitions, binding_branches) {
            let definitions = definitions
                .iter()
                .map(|(name, expression)| (name.as_str(), expression))
                .collect();
            for ((name, source), (_, checked)) in
                action.assignments.iter().zip(&written.assignments)
            {
                record_binding_sites(
                    source,
                    checked,
                    &definitions,
                    &mut bindings,
                    name,
                    &mut Vec::new(),
                );
            }
            let branches = locations
                .update_branches
                .entry(action.name.clone())
                .or_default();
            branches.extend(
                bindings
                    .into_values()
                    .flatten()
                    .map(|entry| entry.branch)
                    .filter(|branch| !branch.sites.is_empty()),
            );
            branches.sort_by_key(|branch| branch.offset);
        }
    }
    Ok(())
}

/// Every atom in scope: signature sizes and enum members.
pub fn atoms(model: &Model) -> usize {
    model.scope.sizes.values().sum::<usize>()
        + model
            .enums
            .iter()
            .map(|(_, members)| members.len())
            .sum::<usize>()
}
