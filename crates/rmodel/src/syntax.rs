//! The model language (plan 0091): tokens, syntax tree, parser and the
//! type and form checks every lowering relies on.
use crate::{Case, Error, Sequence, Variant};
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
    Difference,
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

#[derive(Clone, Debug)]
pub enum Expr {
    /// A sig, a relation, or a bound atom variable.
    Name(String),
    None,
    Binary(SetOp, Box<Expr>, Box<Expr>),
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
    Sequence(String, Vec<Expr>),
    Append(Box<Expr>, Box<Expr>),
    Length(Box<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Variant(String, Option<Box<Expr>>),
    Payload(Box<Expr>, String),
    At(Phase, Box<Expr>),
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
#[derive(Clone, Debug)]
pub enum Int {
    Count(Expr),
    Literal(i64),
    /// An expression of one integer (plan 0103).
    Value(Expr),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantifier {
    All,
    Some,
}
#[derive(Clone, Debug)]
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
}

/// `record Money { amount: Int, currency: Currency }`: a value compared by
/// its fields (plan 0103).
#[derive(Clone, Debug)]
pub struct Record {
    pub name: String,
    /// Field names and their sorts.
    pub fields: Vec<(String, String)>,
}
#[derive(Clone, Debug)]
pub struct Scope {
    /// Exactly this many atoms per sig.
    pub sizes: BTreeMap<String, usize>,
    /// `Int = lo..hi`: the integers enumerated for initial states,
    /// parameters and records. Values are mathematical; a state with an
    /// integer outside leaves the scope (plan 0103).
    pub ints: Option<(i64, i64)>,
    /// The SAT engine's trace length.
    pub steps: usize,
}
#[derive(Clone, Debug)]
pub struct Model {
    pub name: String,
    pub sigs: Vec<String>,
    /// Glossary text keyed by the entity's checked sort name.
    pub entity_definitions: BTreeMap<String, String>,
    /// `enum Name = A | B`: named atoms, one each, with no scope (0093).
    pub enums: Vec<(String, Vec<String>)>,
    /// `type Party = Person + Org`: a union of signatures and enums with no
    /// atoms of its own (plan 0100).
    pub sums: Vec<(String, Vec<String>)>,
    pub records: Vec<Record>,
    pub sequences: Vec<Sequence>,
    pub variants: Vec<Variant>,
    pub relations: Vec<Relation>,
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
    pub fn record(&self, name: &str) -> Option<&Record> {
        self.records.iter().find(|record| record.name == name)
    }
    /// The record a field belongs to, and the field's position.
    pub fn field(&self, name: &str) -> Option<(&Record, usize)> {
        self.records.iter().find_map(|record| {
            record
                .fields
                .iter()
                .position(|(field, _)| field == name)
                .map(|index| (record, index))
        })
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
    pub(crate) fn uses_nat(&self) -> bool {
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
    "in",
    "none",
];
const MAX_SCOPE: usize = 7;
const MAX_STEPS: usize = 32;

fn valid_int_scope(low: i64, high: i64) -> bool {
    high.checked_sub(low)
        .is_some_and(|width| (0..20).contains(&width))
}
const MAX_LITERAL: i64 = 7;

fn definition_token(characters: &[char], position: &mut usize) -> Result<String, Error> {
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

fn tokens(source: &str) -> Result<Vec<String>, Error> {
    let mut tokens = Vec::new();
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
        if character == '"' {
            tokens.push(definition_token(&characters, &mut position)?);
            continue;
        }
        let start = position;
        if character.is_ascii_alphanumeric() || character == '_' {
            while position < characters.len()
                && (characters[position].is_ascii_alphanumeric() || characters[position] == '_')
            {
                position += 1;
            }
        } else if let Some(operator) = ["->", "!=", "<=", ">="].into_iter().find(|operator| {
            characters[position..].starts_with(&operator.chars().collect::<Vec<_>>())
        }) {
            position += operator.len();
        } else if "{}[]():,.+-*&#=<>'|".contains(character) {
            position += 1;
        } else {
            return Err(Error::syntax(format!("unexpected character {character:?}")));
        }
        tokens.push(characters[start..position].iter().collect());
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<String>,
    position: usize,
    /// Nesting of formulas and expressions being parsed (review of #102).
    depth: usize,
    /// Formulas and expressions parsed in the current declaration (issue
    /// #106): bounds the tree the checker and Drop recurse over.
    nodes: usize,
    /// Declared records and their field names, for record literals.
    records: BTreeMap<String, Vec<String>>,
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
            enums: Vec::new(),
            sums: Vec::new(),
            records: Vec::new(),
            sequences: Vec::new(),
            variants: Vec::new(),
            relations: Vec::new(),
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
                steps: 0,
            },
        };
        let mut scoped = false;
        while !self.peek().is_empty() {
            // The node bound applies to each declaration (issue #106).
            self.nodes = 0;
            match self.peek() {
                "enum" => {
                    self.position += 1;
                    let name = self.name()?;
                    self.expect("=")?;
                    let mut members = vec![self.name()?];
                    while self.take("|") {
                        members.push(self.name()?);
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
                        let tag = self.name()?;
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
                "record" => {
                    self.position += 1;
                    let name = self.name()?;
                    self.expect("{")?;
                    let mut fields = Vec::new();
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
                    if fields.is_empty() {
                        return Err(Error::syntax(format!("record {name} has no fields")));
                    }
                    self.records.insert(
                        name.clone(),
                        fields.iter().map(|(field, _)| field.clone()).collect(),
                    );
                    model.records.push(Record { name, fields });
                }
                "sig" | "entity" => {
                    let entity = self.peek() == "entity";
                    self.position += 1;
                    let name = self.name()?;
                    if entity && let Some(definition) = self.peek().strip_prefix('"') {
                        model
                            .entity_definitions
                            .insert(name.clone(), definition.to_owned());
                        self.position += 1;
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
                    let formula = Formula::And(self.block()?);
                    model.transitions.push(TransitionCheck { name, formula });
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
                            model.scope.steps = self.size(MAX_STEPS)?;
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
        }
        if !scoped || model.scope.steps == 0 {
            return Err(Error::syntax("the model needs a scope with steps"));
        }
        Ok(model)
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
        if value < 1 || value as usize > maximum {
            return Err(Error::syntax(format!("size {value} outside 1..={maximum}")));
        }
        Ok(value as usize)
    }
    fn block(&mut self) -> Result<Vec<Formula>, Error> {
        self.expect("{")?;
        let mut formulas = Vec::new();
        while !self.take("}") {
            if self.peek().is_empty() {
                return Err(Error::syntax("unterminated block"));
            }
            formulas.push(self.formula()?);
        }
        Ok(formulas)
    }
    fn fairness(&mut self) -> Result<Fairness, Error> {
        let kind = if self.take("weak") {
            FairnessKind::Weak
        } else {
            self.expect("strong")?;
            FairnessKind::Strong
        };
        Ok(Fairness {
            kind,
            action: self.name()?,
        })
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
                while !self.take("}") {
                    action.assignments.push(self.assignment(false)?);
                }
            }
            self.expect("}")?;
            return Ok(action);
        }
        while !self.take("}") {
            if self.peek().is_empty() {
                return Err(Error::syntax("unterminated action"));
            }
            if self.peek_at(1) == "'" {
                action.assignments.push(self.assignment(true)?);
            } else {
                action.guards.push(self.formula()?);
            }
        }
        Ok(action)
    }

    fn assignment(&mut self, primed: bool) -> Result<(String, Expr), Error> {
        let var = self.name()?;
        if primed {
            self.expect("'")?;
        }
        self.expect("=")?;
        Ok((var, self.expression(0)?))
    }

    /// implies (right) < or < and < not < atoms.
    fn enter(&mut self) -> Result<(), Error> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(Error::syntax(format!(
                "a declaration with more than {MAX_NODES} nodes"
            )));
        }
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
    fn formula(&mut self) -> Result<Formula, Error> {
        self.enter()?;
        let formula = self.implication();
        self.depth -= 1;
        formula
    }
    fn implication(&mut self) -> Result<Formula, Error> {
        let left = self.disjunction()?;
        if self.take("implies") {
            return Ok(Formula::Implies(Box::new(left), Box::new(self.formula()?)));
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
                "in" | "=" | "!=" | "." | "+" | "-" | "&" | "->" | "*" | "<" | "<=" | ">" | ">="
            ) || self.peek() == "not" && self.peek_at(1) == "in";
            match formula {
                Ok(formula) if !comparison_follows => return Ok(formula),
                _ => (self.position, self.depth, self.nodes) = start,
            }
        }
        let integer = self.peek() == "#"
            || self.peek() == "-"
            || self.peek().bytes().all(|byte| byte.is_ascii_digit());
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
            let right = self.expression(0)?;
            return Ok(Formula::Ints(operator, Self::int(left), Self::int(right)));
        }
        if integer {
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
            // A Bool expression on its own holds when it is true (plan
            // 0103); the checker rejects any other expression here.
            _ => {
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
    fn record(&mut self, name: String, declared: &[String]) -> Result<Expr, Error> {
        self.expect("{")?;
        let mut given: BTreeMap<String, Expr> = BTreeMap::new();
        while !self.take("}") {
            if !given.is_empty() {
                self.expect(",")?;
            }
            let field = self.name()?;
            self.expect(":")?;
            let value = self.expression(0)?;
            if !declared.contains(&field) || given.insert(field.clone(), value).is_some() {
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
    fn int(expression: Expr) -> Int {
        match expression {
            Expr::Count(counted) => Int::Count(*counted),
            Expr::Literal(value) => Int::Literal(value),
            other => Int::Value(other),
        }
    }
    /// `+ -` (1) < `&` (2) < `->` (3) < `.` (4).
    fn expression(&mut self, minimum: u8) -> Result<Expr, Error> {
        self.enter()?;
        let expression = self.operand_chain(minimum);
        self.depth -= 1;
        expression
    }
    fn operand_chain(&mut self, minimum: u8) -> Result<Expr, Error> {
        let mut length = 1;
        let mut left = if self.take("(") {
            let inner = self.expression(0)?;
            self.expect(")")?;
            inner
        } else if self.take("none") {
            Expr::None
        } else if self.take("true") {
            Expr::Name("true".into())
        } else if self.take("false") {
            Expr::Name("false".into())
        } else if self.take("#") {
            Expr::Count(Box::new(self.expression(4)?))
        } else if self.peek() == "-" || self.peek().bytes().all(|byte| byte.is_ascii_digit()) {
            Expr::Literal(self.signed()?)
        } else {
            let name = self.name()?;
            if self.take("[") {
                let mut items = Vec::new();
                if !self.take("]") {
                    loop {
                        Self::chain(items.len() + 1)?;
                        items.push(self.expression(0)?);
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
                    Some(Box::new(self.expression(0)?))
                };
                let required = |arg: Option<Box<Expr>>| {
                    arg.ok_or_else(|| Error::syntax("expected an argument"))
                };
                let call = match name.as_str() {
                    "len" => Expr::Length(required(argument)?),
                    "before" => Expr::At(Phase::Before, required(argument)?),
                    "after" => Expr::At(Phase::After, required(argument)?),
                    "append" | "at" => {
                        let first = required(argument)?;
                        self.expect(",")?;
                        let second = Box::new(self.expression(0)?);
                        if name == "append" {
                            Expr::Append(first, second)
                        } else {
                            Expr::Index(first, second)
                        }
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
                    Some(declared) if self.peek() == "{" => self.record(name, &declared)?,
                    _ => Expr::Name(name),
                }
            }
        };
        if self.take("'") {
            left = Expr::At(Phase::After, Box::new(left));
        }
        loop {
            let (operator, precedence) = match self.peek() {
                "+" => (SetOp::Union, 1),
                "-" => (SetOp::Difference, 1),
                "&" => (SetOp::Intersection, 2),
                "*" => (SetOp::Multiply, 2),
                "->" => (SetOp::Product, 3),
                "." => (SetOp::Join, 4),
                _ => break,
            };
            if precedence < minimum {
                break;
            }
            self.position += 1;
            length += 1;
            Self::chain(length)?;
            let right = self.expression(precedence + 1)?;
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

struct Checker<'a> {
    model: &'a Model,
    transition: bool,
    sigs: BTreeSet<&'a str>,
}
impl Checker<'_> {
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
        let joined = |a: &str, b: &str| self.model.join_sorts(a, b);
        match (left, right) {
            (Type::Any, other) | (other, Type::Any) => Ok(other),
            (left, right) if left == right => Ok(left),
            (Type::Set(a), Type::Set(b)) if joined(&a, &b).is_some() => {
                Ok(Type::Set(joined(&a, &b).expect("joined")))
            }
            (Type::Pair(a, b), Type::Pair(c, d))
                if joined(&a, &c).is_some() && joined(&b, &d).is_some() =>
            {
                Ok(Type::Pair(
                    joined(&a, &c).expect("joined"),
                    joined(&b, &d).expect("joined"),
                ))
            }
            (left, right) if left.arity() != right.arity() => {
                Err(Error::kind(format!("{what} of different arities")))
            }
            (left, right) => Err(Error::kind(format!(
                "{what} of different sorts: {} and {}",
                left.describe(),
                right.describe()
            ))),
        }
    }
    /// Whether an expression has exactly one element in every state, so
    /// arithmetic and integer comparisons on it are total (plan 0103).
    fn singular(&self, expression: &Expr, bound: &Bound) -> bool {
        match expression {
            Expr::Literal(_) | Expr::Count(_) | Expr::Arith(..) => true,
            Expr::At(_, expr) => self.singular(expr, bound),
            Expr::Sequence(_, items) => items.iter().all(|item| self.singular(item, bound)),
            Expr::Append(left, right) => self.singular(left, bound) && self.singular(right, bound),
            Expr::Length(value) => self.singular(value, bound),
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
            Expr::Binary(SetOp::Join, left, right) => {
                self.singular(left, bound)
                    && match right.as_ref() {
                        Expr::Name(name) => {
                            self.model.field(name).is_some()
                                || self.model.relation(name).is_some_and(|relation| {
                                    matches!(
                                        relation.shape,
                                        Shape::Binary {
                                            mult: Mult::One,
                                            ..
                                        }
                                    )
                                })
                        }
                        _ => false,
                    }
            }
            _ => false,
        }
    }
    fn integer(&self, expression: &Expr, bound: &Bound, what: &str) -> Result<(), Error> {
        if !self.singular(expression, bound) {
            return Err(Error::kind(format!(
                "{what} needs exactly one integer on each side; this one may be empty or several"
            )));
        }
        Ok(())
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
                if matches!(self.expression(counted, bound, vars)?, Type::Pair(..)) {
                    return Err(Error::kind("# counts only sets"));
                }
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
            Expr::At(phase, inner) => {
                if !self.transition {
                    return Err(Error::kind(
                        "expected a state expression; before, after and prime are only allowed in a transition property, without nesting",
                    ));
                }
                if matches!(inner.as_ref(), Expr::Name(name) if self.model.field(name).is_some() && !bound.contains_key(name))
                {
                    return Err(Error::kind(
                        "wrap the whole record projection in before or after",
                    ));
                }
                let state = Checker {
                    transition: false,
                    model: self.model,
                    sigs: self.sigs.clone(),
                };
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
            Expr::Name(name) => {
                if let Some(sort) = bound.get(name) {
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
                    Self::shape(&relation.shape)
                } else if let Some((record, index)) = self.model.field(name) {
                    Type::Pair(record.name.clone(), record.fields[index].1.clone())
                } else {
                    return Err(Error::kind(format!("unknown name {name}")));
                }
            }
            Expr::Binary(operator, left_expression, right_expression) => {
                let left = self.expression(left_expression, bound, vars)?;
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
                            if matches!(self.expression(expression, bound, vars)?, Type::Pair(..)) {
                                return Err(Error::kind("# counts only sets"));
                            }
                        }
                        // Counts are Alloy integers of a few bits; integer
                        // values have their own scope (plan 0103).
                        Int::Literal(value) => {
                            if self.model.scope.ints.is_none() && *value > MAX_LITERAL {
                                return Err(Error::syntax(format!(
                                    "literal {value} above {MAX_LITERAL}"
                                )));
                            }
                            if !counts && self.model.scope.ints.is_none() {
                                return Err(Error::kind(
                                    "an integer value needs a scope Int = lo..hi",
                                ));
                            }
                        }
                        Int::Value(expression) => {
                            if !self.expression(expression, bound, vars)?.numeric() {
                                return Err(Error::kind("an integer comparison needs integers"));
                            }
                            self.integer(expression, bound, "an integer comparison")?;
                        }
                    }
                }
            }
            Formula::HasTag(value, tag) => self.variant_subject(value, tag, bound, vars)?,
            Formula::Mult(_, expression) | Formula::Empty(expression) => {
                self.expression(expression, bound, vars)?;
            }
            Formula::Not(inner) => self.formula(inner, bound, vars)?,
            Formula::And(parts) => {
                for part in parts {
                    self.formula(part, bound, vars)?;
                }
            }
            Formula::Or(left, right) | Formula::Implies(left, right) => {
                self.formula(left, bound, vars)?;
                self.formula(right, bound, vars)?;
            }
            Formula::Quantified(_, variable, sig, body) => {
                if self.model.is_value(sig) {
                    return Err(Error::kind(format!(
                        "quantifying over {sig}: only atoms are quantified, values have no fixed set"
                    )));
                }
                if !self.sigs.contains(sig.as_str()) {
                    return Err(Error::kind(format!("unknown sig {sig}")));
                }
                self.fresh(variable, bound)?;
                let mut inner = bound.clone();
                inner.insert(variable.clone(), sig.clone());
                self.formula(body, &inner, vars)?;
            }
        }
        Ok(())
    }
    fn fresh(&self, name: &str, bound: &Bound) -> Result<(), Error> {
        if bound.contains_key(name)
            || self.sigs.contains(name)
            || self.model.member(name).is_some()
            || self.model.relation(name).is_some()
            || self.model.field(name).is_some()
        {
            return Err(Error::kind(format!("{name} shadows another name")));
        }
        Ok(())
    }
}

pub(crate) fn parse(source: &str) -> Result<Model, Error> {
    if source.len() > 1 << 20 {
        return Err(Error::syntax("source exceeds 1 MiB"));
    }
    let tokens = tokens(source)?;
    let boolean = tokens
        .iter()
        .any(|token| [BOOL, "true", "false"].contains(&token.as_str()));
    let mut parser = Parser {
        tokens,
        position: 0,
        depth: 0,
        nodes: 0,
        records: BTreeMap::new(),
    };
    let mut model = parser.model()?;
    if boolean {
        model
            .enums
            .push((BOOL.to_owned(), vec!["false".into(), "true".into()]));
    }
    check(&mut model)?;
    Ok(model)
}

/// Type-checks a syntax tree built without the parser, such as one read
/// from the JSON IR (plan 0108), with the parser's limits on its size.
/// Bool is added when the tree mentions it, as the parser does.
pub(crate) fn validate(mut model: Model) -> Result<Model, Error> {
    fn expr(
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
            Expr::None | Expr::Literal(_) => {}
            Expr::Length(inner) | Expr::Payload(inner, _) | Expr::At(_, inner) => {
                expr(inner, nodes, depth + 1, names)?
            }
            Expr::Append(left, right) | Expr::Index(left, right) => {
                expr(left, nodes, depth + 1, names)?;
                expr(right, nodes, depth + 1, names)?;
            }
            Expr::Sequence(_, items) => {
                for item in items {
                    expr(item, nodes, depth + 1, names)?;
                }
            }
            Expr::Variant(_, payload) => {
                if let Some(value) = payload {
                    expr(value, nodes, depth + 1, names)?;
                }
            }
            Expr::Count(inner) => expr(inner, nodes, depth + 1, names)?,
            Expr::Binary(_, l, r) | Expr::Arith(_, l, r) => {
                expr(l, nodes, depth + 1, names)?;
                expr(r, nodes, depth + 1, names)?;
            }
            Expr::Record(name, values) => {
                names.insert(name.clone());
                for v in values {
                    expr(v, nodes, depth + 1, names)?;
                }
            }
        }
        Ok(())
    }
    fn formula(
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
            Int::Count(e) | Int::Value(e) => expr(e, nodes, depth + 1, names),
            Int::Literal(_) => Ok(()),
        };
        match f {
            Formula::Compare(_, l, r) => {
                expr(l, nodes, depth + 1, names)?;
                expr(r, nodes, depth + 1, names)?;
            }
            Formula::Ints(_, l, r) => {
                int(l, nodes, names)?;
                int(r, nodes, names)?;
            }
            Formula::Mult(_, e) | Formula::Empty(e) | Formula::HasTag(e, _) => {
                expr(e, nodes, depth + 1, names)?
            }
            Formula::Not(inner) => formula(inner, nodes, depth + 1, names)?,
            Formula::And(parts) => {
                for p in parts {
                    formula(p, nodes, depth + 1, names)?;
                }
            }
            Formula::Or(l, r) | Formula::Implies(l, r) => {
                formula(l, nodes, depth + 1, names)?;
                formula(r, nodes, depth + 1, names)?;
            }
            Formula::Quantified(_, _, sort, body) => {
                names.insert(sort.clone());
                formula(body, nodes, depth + 1, names)?;
            }
        }
        Ok(())
    }
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
        formula(f, &mut nodes, 0, &mut names)?;
    }
    for action in &model.actions {
        names.extend(action.params.iter().map(|(_, sort)| sort.clone()));
        for g in &action.guards {
            formula(g, &mut nodes, 0, &mut names)?;
        }
        for (_, value) in &action.assignments {
            expr(value, &mut nodes, 0, &mut names)?;
        }
    }
    let boolean = [BOOL, "true", "false"].iter().any(|n| names.contains(*n));
    if boolean && !model.enums.iter().any(|(name, _)| name == BOOL) {
        model
            .enums
            .push((BOOL.to_owned(), vec!["false".into(), "true".into()]));
    }
    if model.scope.steps == 0 {
        return Err(Error::syntax("the model needs a scope with steps"));
    }
    if model.scope.steps > MAX_STEPS {
        return Err(Error::syntax(format!("steps outside 1..={MAX_STEPS}")));
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
    check(&mut model)?;
    Ok(model)
}

fn check(model: &mut Model) -> Result<(), Error> {
    // The checker reads a snapshot and rewrites arithmetic in the model.
    let snapshot = model.clone();
    check_in(&snapshot, model)?;
    crate::numeric::validate(model, i64::MIN..=i64::MAX)
}

fn check_in(model: &Model, target: &mut Model) -> Result<(), Error> {
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
        .chain(model.records.iter().flat_map(|record| {
            std::iter::once(&record.name).chain(record.fields.iter().map(|(field, _)| field))
        }))
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
    {
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
    let sigs: BTreeSet<&str> = model.sigs.iter().map(String::as_str).collect();
    for (name, definition) in &model.entity_definitions {
        if !sigs.contains(name.as_str()) {
            return Err(Error::kind(format!("definition for unknown entity {name}")));
        }
        if definition.trim().is_empty() || !definition.chars().all(definition_character) {
            return Err(Error::kind(format!(
                "{name}: expected a nonempty one-line definition"
            )));
        }
    }
    for sig in &model.sigs {
        if !model.scope.sizes.contains_key(sig) {
            return Err(Error::kind(format!("no scope for {sig}")));
        }
    }
    for sig in model.scope.sizes.keys() {
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
    if model.declared_sorts().any(numeric_sort) && model.scope.ints.is_none() {
        return Err(Error::kind("Int and Nat need a scope Int = lo..hi"));
    }
    if model.uses_nat() && model.scope.ints.is_some_and(|(_, high)| high < 0) {
        return Err(Error::kind(
            "the Int scope needs a nonnegative value for Nat",
        ));
    }
    sigs.extend([INT, NAT]);
    for value in model.value_order()? {
        sigs.insert(value.name());
    }
    for relation in &model.relations {
        let used = match &relation.shape {
            Shape::Unary { sig, .. } => vec![sig],
            Shape::Binary { from, to, .. } => vec![from, to],
        };
        if let Some(sig) = used.iter().find(|sig| !sigs.contains(sig.as_str())) {
            return Err(Error::kind(format!("{}: unknown sig {sig}", relation.name)));
        }
        // A relation holds at most one value of a value sort per atom: no
        // sets of values in v1.
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
        if model.is_value(target) && !matches!(mult, Mult::One | Mult::Lone) {
            return Err(Error::kind(format!(
                "{}: a relation to {target} holds one or lone value, not a set",
                relation.name
            )));
        }
    }
    // Counts are integers in the Alloy lowering, at most 5 bits (-16..15);
    // every count of a set is bounded by the atoms in scope (review of #102).
    if uses_counts(model) && atoms(model) > 15 {
        return Err(Error::kind(format!(
            "counts over {} atoms exceed the 15 an Alloy integer holds",
            atoms(model)
        )));
    }
    let checker = Checker {
        model,
        sigs,
        transition: false,
    };
    let none = Bound::new();
    for fact in &mut target.facts {
        checker.formula(fact, &none, false)?;
    }
    for formula in target
        .init
        .iter_mut()
        .chain(target.checks.iter_mut().map(|check| &mut check.formula))
    {
        checker.formula(formula, &none, true)?;
    }
    for variant in &model.variants {
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
        model,
        sigs: checker.sigs.clone(),
    };
    for check in &mut target.transitions {
        transition_checker.formula(&mut check.formula, &none, true)?;
    }
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
        let mut bound = Bound::new();
        for (param, sig) in &action.params {
            if !checker.sigs.contains(sig.as_str()) {
                return Err(Error::kind(format!("{}: unknown sig {sig}", action.name)));
            }
            checker.fresh(param, &bound)?;
            bound.insert(param.clone(), sig.clone());
        }
        for guard in &mut written.guards {
            checker.formula(guard, &bound, true)?;
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
    }
    Ok(())
}

/// Every atom in scope: signature sizes and enum members.
pub(crate) fn atoms(model: &Model) -> usize {
    model.scope.sizes.values().sum::<usize>()
        + model
            .enums
            .iter()
            .map(|(_, members)| members.len())
            .sum::<usize>()
}

/// Whether any formula compares integers.
pub(crate) fn uses_counts(model: &Model) -> bool {
    fn ints(formula: &Formula) -> bool {
        match formula {
            Formula::Ints(..) => true,
            Formula::Not(inner) | Formula::Quantified(_, _, _, inner) => ints(inner),
            Formula::And(parts) => parts.iter().any(ints),
            Formula::Or(left, right) | Formula::Implies(left, right) => ints(left) || ints(right),
            _ => false,
        }
    }
    model
        .facts
        .iter()
        .chain(&model.init)
        .chain(model.checks.iter().map(|check| &check.formula))
        .chain(model.actions.iter().flat_map(|action| &action.guards))
        .any(ints)
}
