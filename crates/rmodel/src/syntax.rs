//! The model language (plan 0091): tokens, syntax tree, parser and the
//! type and form checks every lowering relies on.
use crate::Error;
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
}
#[derive(Clone, Debug)]
pub enum Expr {
    /// A sig, a relation, or a bound atom variable.
    Name(String),
    None,
    Binary(SetOp, Box<Expr>, Box<Expr>),
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
    /// `var' = expr`, at most one per var.
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
pub struct Scope {
    /// Exactly this many atoms per sig.
    pub sizes: BTreeMap<String, usize>,
    /// The SAT engine's trace length.
    pub steps: usize,
}
#[derive(Clone, Debug)]
pub struct Model {
    pub name: String,
    pub sigs: Vec<String>,
    /// `enum Name = A | B`: named atoms, one each, with no scope (0093).
    pub enums: Vec<(String, Vec<String>)>,
    pub relations: Vec<Relation>,
    pub facts: Vec<Formula>,
    pub init: Vec<Formula>,
    pub actions: Vec<Action>,
    pub checks: Vec<Check>,
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
    pub fn is_enum(&self, name: &str) -> bool {
        self.enums
            .iter()
            .any(|(enumeration, _)| enumeration == name)
    }
    pub fn vars(&self) -> impl Iterator<Item = &Relation> {
        self.relations.iter().filter(|relation| relation.var)
    }
}

const KEYWORDS: &[&str] = &[
    "model",
    "enum",
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
const MAX_LITERAL: i64 = 7;

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
        } else if "{}():,.+-&#=<>'|".contains(character) {
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
            enums: Vec::new(),
            relations: Vec::new(),
            facts: Vec::new(),
            init: Vec::new(),
            actions: Vec::new(),
            checks: Vec::new(),
            scope: Scope {
                sizes: BTreeMap::new(),
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
                "sig" => {
                    self.position += 1;
                    model.sigs.push(self.name()?);
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
                "action" => {
                    self.position += 1;
                    model.actions.push(self.action()?);
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
                            model.scope.steps = self.size(32)?;
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
    fn action(&mut self) -> Result<Action, Error> {
        let name = self.name()?;
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
        let mut action = Action {
            name,
            params,
            guards: Vec::new(),
            assignments: Vec::new(),
        };
        self.expect("{")?;
        while !self.take("}") {
            if self.peek().is_empty() {
                return Err(Error::syntax("unterminated action"));
            }
            if self.peek_at(1) == "'" {
                let var = self.name()?;
                self.expect("'")?;
                self.expect("=")?;
                action.assignments.push((var, self.expression(0)?));
            } else {
                action.guards.push(self.formula()?);
            }
        }
        Ok(action)
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
            let comparison_follows = matches!(
                self.peek(),
                "in" | "=" | "!=" | "." | "+" | "-" | "&" | "->"
            ) || self.peek() == "not" && self.peek_at(1) == "in";
            match formula {
                Ok(formula) if !comparison_follows => return Ok(formula),
                _ => (self.position, self.depth, self.nodes) = start,
            }
        }
        if self.peek() == "#" || self.peek().bytes().all(|byte| byte.is_ascii_digit()) {
            let left = self.int()?;
            let operator = match self.peek() {
                "<" => IntCompare::Less,
                "<=" => IntCompare::LessEqual,
                ">" => IntCompare::Greater,
                ">=" => IntCompare::GreaterEqual,
                "=" => IntCompare::Equal,
                "!=" => IntCompare::NotEqual,
                other => {
                    return Err(Error::syntax(format!(
                        "expected an integer comparison, got {other:?}"
                    )));
                }
            };
            self.position += 1;
            return Ok(Formula::Ints(operator, left, self.int()?));
        }
        let left = self.expression(0)?;
        let operator = match self.peek() {
            "in" => Compare::In,
            "=" => Compare::Equal,
            "!=" => Compare::NotEqual,
            "not" if self.peek_at(1) == "in" => {
                self.position += 1;
                Compare::NotIn
            }
            other => {
                return Err(Error::syntax(format!(
                    "expected a comparison, got {other:?}"
                )));
            }
        };
        self.position += 1;
        Ok(Formula::Compare(operator, left, self.expression(0)?))
    }
    fn int(&mut self) -> Result<Int, Error> {
        if self.take("#") {
            return Ok(Int::Count(self.expression(4)?));
        }
        let value = self.number()?;
        if value > MAX_LITERAL {
            return Err(Error::syntax(format!(
                "literal {value} above {MAX_LITERAL}"
            )));
        }
        Ok(Int::Literal(value))
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
        } else {
            Expr::Name(self.name()?)
        };
        loop {
            let (operator, precedence) = match self.peek() {
                "+" => (SetOp::Union, 1),
                "-" => (SetOp::Difference, 1),
                "&" => (SetOp::Intersection, 2),
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
    sigs: BTreeSet<&'a str>,
}
impl Checker<'_> {
    fn shape(shape: &Shape) -> Type {
        match shape {
            Shape::Unary { sig, .. } => Type::Set(sig.clone()),
            Shape::Binary { from, to, .. } => Type::Pair(from.clone(), to.clone()),
        }
    }
    /// Both sides of a set operation or comparison have one type.
    fn same(left: Type, right: Type, what: &str) -> Result<Type, Error> {
        match (left, right) {
            (Type::Any, other) | (other, Type::Any) => Ok(other),
            (left, right) if left == right => Ok(left),
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
    fn expression(&self, expression: &Expr, bound: &Bound, vars: bool) -> Result<Type, Error> {
        Ok(match expression {
            Expr::None => Type::Any,
            Expr::Name(name) => {
                if let Some(sort) = bound.get(name) {
                    Type::Set(sort.clone())
                } else if self.sigs.contains(name.as_str()) {
                    Type::Set(name.clone())
                } else if let Some(enumeration) = self.model.member(name) {
                    Type::Set(enumeration.to_owned())
                } else if let Some(relation) = self.model.relation(name) {
                    if relation.var && !vars {
                        return Err(Error::kind(format!("fact refers to the var {name}")));
                    }
                    Self::shape(&relation.shape)
                } else {
                    return Err(Error::kind(format!("unknown name {name}")));
                }
            }
            Expr::Binary(operator, left, right) => {
                let left = self.expression(left, bound, vars)?;
                let right = self.expression(right, bound, vars)?;
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
                                if a != b {
                                    return mismatch(&a, &b);
                                }
                                Type::Set(c)
                            }
                            (Type::Pair(a, b), Type::Set(c)) => {
                                if b != c {
                                    return mismatch(&b, &c);
                                }
                                Type::Set(a)
                            }
                            (Type::Pair(a, b), Type::Pair(c, d)) => {
                                if b != c {
                                    return mismatch(&b, &c);
                                }
                                Type::Pair(a, d)
                            }
                            (Type::Any, Type::Pair(..)) | (Type::Pair(..), Type::Any) => Type::Any,
                            _ => return Err(Error::kind("join needs a binary relation")),
                        }
                    }
                    _ => Self::same(left, right, "set operation")?,
                }
            }
        })
    }
    fn formula(&self, formula: &Formula, bound: &Bound, vars: bool) -> Result<(), Error> {
        match formula {
            Formula::Compare(_, left, right) => {
                let (left, right) = (
                    self.expression(left, bound, vars)?,
                    self.expression(right, bound, vars)?,
                );
                Self::same(left, right, "comparison")?;
            }
            Formula::Ints(_, left, right) => {
                for int in [left, right] {
                    if let Int::Count(expression) = int
                        && matches!(self.expression(expression, bound, vars)?, Type::Pair(..))
                    {
                        return Err(Error::kind("# counts only sets"));
                    }
                }
            }
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
    let mut parser = Parser {
        tokens: tokens(source)?,
        position: 0,
        depth: 0,
        nodes: 0,
    };
    let model = parser.model()?;
    check(&model)?;
    Ok(model)
}

fn check(model: &Model) -> Result<(), Error> {
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
        .chain(model.relations.iter().map(|relation| &relation.name))
        .chain(model.actions.iter().map(|action| &action.name))
        .chain(model.checks.iter().map(|check| &check.name))
    {
        if !names.insert(name.as_str()) {
            return Err(Error::kind(format!("duplicate name {name}")));
        }
        if name == crate::DECLARATIONS {
            return Err(Error::kind(format!(
                "{name} is the built-in multiplicity check"
            )));
        }
    }
    let sigs: BTreeSet<&str> = model.sigs.iter().map(String::as_str).collect();
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
    // Enums are sigs for typing, without a scope.
    let sigs: BTreeSet<&str> = sigs
        .into_iter()
        .chain(model.enums.iter().map(|(name, _)| name.as_str()))
        .collect();
    for relation in &model.relations {
        let used = match &relation.shape {
            Shape::Unary { sig, .. } => vec![sig],
            Shape::Binary { from, to, .. } => vec![from, to],
        };
        if let Some(sig) = used.iter().find(|sig| !sigs.contains(sig.as_str())) {
            return Err(Error::kind(format!("{}: unknown sig {sig}", relation.name)));
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
    let checker = Checker { model, sigs };
    let none = Bound::new();
    for fact in &model.facts {
        checker.formula(fact, &none, false)?;
    }
    for formula in model
        .init
        .iter()
        .chain(model.checks.iter().map(|check| &check.formula))
    {
        checker.formula(formula, &none, true)?;
    }
    for action in &model.actions {
        let mut bound = Bound::new();
        for (param, sig) in &action.params {
            if !checker.sigs.contains(sig.as_str()) {
                return Err(Error::kind(format!("{}: unknown sig {sig}", action.name)));
            }
            checker.fresh(param, &bound)?;
            bound.insert(param.clone(), sig.clone());
        }
        for guard in &action.guards {
            checker.formula(guard, &bound, true)?;
        }
        let mut assigned = BTreeSet::new();
        for (var, value) in &action.assignments {
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
            if value != expected && value != Type::Any {
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
