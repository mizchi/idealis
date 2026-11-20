use crate::{Assumption, Edge, Error, Formula, Problem, Result, Solver, Variable};
use num_bigint::BigInt;
use num_traits::{Signed, Zero};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
enum Node {
    Atom(String, bool),
    List(Vec<Self>),
}
fn symbol(node: &Node) -> Result<&str> {
    match node {
        Node::Atom(value, _) => Ok(value),
        _ => Err(Error::new("syntax", "expected symbol")),
    }
}
fn list(node: &Node) -> Result<&[Node]> {
    match node {
        Node::List(values) => Ok(values),
        _ => Err(Error::new("syntax", "expected list")),
    }
}
fn parse(source: &str) -> Result<Vec<Node>> {
    if source.len() > 1_048_576 {
        return Err(Error::new("resource", "source exceeds 1 MiB"));
    }
    let mut tokens = vec![];
    let mut chars = source.chars().peekable();
    while let Some(character) = chars.next() {
        if character.is_whitespace() {
            continue;
        }
        if character == ';' {
            for character in chars.by_ref() {
                if character == '\n' {
                    break;
                }
            }
            continue;
        }
        if character == '(' || character == ')' {
            tokens.push(Node::Atom(character.to_string(), false));
        } else if character == '|' {
            let mut value = String::new();
            let mut closed = false;
            for character in chars.by_ref() {
                if character == '|' {
                    closed = true;
                    break;
                }
                if character == '\\' || !character.is_ascii() {
                    return Err(Error::new("unsupported", "quoted identifier character"));
                }
                value.push(character);
            }
            if !closed {
                return Err(Error::new("syntax", "unclosed quoted identifier"));
            }
            tokens.push(Node::Atom(value, true));
        } else {
            if character == '"' || !character.is_ascii() {
                return Err(Error::new("unsupported", "strings and Unicode symbols"));
            }
            let mut value = character.to_string();
            while let Some(character) = chars.peek() {
                if character.is_whitespace() || ['(', ')', ';'].contains(character) {
                    break;
                }
                value.push(*character);
                chars.next();
            }
            if !value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "~!@$%^&*_-+=<>.?/:".contains(c))
            {
                return Err(Error::new("syntax", "invalid simple symbol"));
            }
            if value.len() > 4096 {
                return Err(Error::new("resource", "token exceeds 4096 bytes"));
            }
            tokens.push(Node::Atom(value, false));
        }
        if tokens.len() > 200_000 {
            return Err(Error::new("resource", "too many tokens"));
        }
    }
    fn node(tokens: &[Node], position: &mut usize, depth: usize) -> Result<Node> {
        if depth > 64 {
            return Err(Error::new("resource", "nesting exceeds 64"));
        }
        let value = tokens
            .get(*position)
            .ok_or_else(|| Error::new("syntax", "unexpected end"))?;
        *position += 1;
        match value {
            Node::Atom(value, false) if value == "(" => {
                let mut values = vec![];
                loop {
                    if matches!(tokens.get(*position),Some(Node::Atom(value,false)) if value==")") {
                        *position += 1;
                        break;
                    }
                    values.push(node(tokens, position, depth + 1)?);
                }
                Ok(Node::List(values))
            }
            Node::Atom(value, false) if value == ")" => {
                Err(Error::new("syntax", "unexpected closing parenthesis"))
            }
            _ => Ok(value.clone()),
        }
    }
    let mut result = vec![];
    let mut position = 0;
    while position < tokens.len() {
        result.push(node(&tokens, &mut position, 0)?);
    }
    Ok(result)
}
#[derive(Clone, Default)]
struct Linear {
    constant: BigInt,
    coefficients: BTreeMap<usize, BigInt>,
}
impl Linear {
    fn add(&mut self, other: &Self, sign: i64) {
        self.constant += &other.constant * sign;
        for (slot, value) in &other.coefficients {
            *self.coefficients.entry(*slot).or_default() += value * sign;
        }
        self.coefficients.retain(|_, value| !value.is_zero());
    }
    fn scaled(&self, factor: &BigInt) -> Self {
        Self {
            constant: &self.constant * factor,
            coefficients: self
                .coefficients
                .iter()
                .map(|(slot, value)| (*slot, value * factor))
                .filter(|(_, value)| !value.is_zero())
                .collect(),
        }
    }
}
#[derive(Clone)]
struct IntCase {
    guard: Arc<Formula>,
    value: Linear,
}
#[derive(Clone)]
struct Integer {
    cases: Vec<IntCase>,
}
impl Integer {
    fn plain(value: Linear) -> Self {
        Self {
            cases: vec![IntCase {
                guard: Arc::new(Formula::Constant(true)),
                value,
            }],
        }
    }
    fn combine(
        &self,
        other: &Self,
        operation: impl Fn(&Linear, &Linear) -> Result<Linear>,
    ) -> Result<Self> {
        if self.cases.len() * other.cases.len() > 256 {
            return Err(Error::new("resource", "integer alternatives exceed 256"));
        }
        let mut cases = Vec::new();
        for left in &self.cases {
            for right in &other.cases {
                cases.push(IntCase {
                    guard: conjunction(vec![left.guard.clone(), right.guard.clone()]),
                    value: operation(&left.value, &right.value)?,
                });
            }
        }
        Ok(Self { cases })
    }
}
fn conjunction(values: Vec<Arc<Formula>>) -> Arc<Formula> {
    let mut values: Vec<_> = values
        .into_iter()
        .filter(|value| !matches!(value.as_ref(), Formula::Constant(true)))
        .collect();
    if values.is_empty() {
        return Arc::new(Formula::Constant(true));
    }
    if values.len() == 1 {
        values.pop().expect("one guard")
    } else {
        Arc::new(Formula::And(values))
    }
}
fn integer_predicate(
    left: &Integer,
    right: &Integer,
    predicate: impl Fn(&Linear, &Linear) -> Result<Arc<Formula>>,
) -> Result<Arc<Formula>> {
    if left.cases.len() * right.cases.len() > 256 {
        return Err(Error::new("resource", "integer alternatives exceed 256"));
    }
    let mut choices = Vec::new();
    for left in &left.cases {
        for right in &right.cases {
            choices.push(conjunction(vec![
                left.guard.clone(),
                right.guard.clone(),
                predicate(&left.value, &right.value)?,
            ]));
        }
    }
    Ok(if choices.len() == 1 {
        choices.pop().expect("one comparison")
    } else {
        Arc::new(Formula::Or(choices))
    })
}
#[derive(Clone)]
enum Term {
    Int(Integer),
    Bool(Arc<Formula>),
}
impl Term {
    fn integer(self) -> Result<Integer> {
        match self {
            Self::Int(value) => Ok(value),
            _ => Err(Error::new("type", "expected Int")),
        }
    }
    fn boolean(self) -> Result<Arc<Formula>> {
        match self {
            Self::Bool(value) => Ok(value),
            _ => Err(Error::new("type", "expected Bool")),
        }
    }
}
struct Compiler {
    variables: BTreeMap<String, (Variable, usize)>,
    constants: BTreeMap<String, BigInt>,
    visible: usize,
}
fn name(node: &Node) -> Result<String> {
    let value = symbol(node)?;
    if value.is_empty()
        || [
            "true", "false", "let", "and", "or", "not", "xor", "ite", "=>", "=", "distinct", "+",
            "-", "*", "<", ">", "<=", ">=",
        ]
        .contains(&value)
        || matches!(node, Node::Atom(_, false))
            && value.chars().next().is_some_and(|c| c.is_ascii_digit())
    {
        return Err(Error::new(
            "unsupported",
            "reserved declaration/binding name",
        ));
    }
    Ok(value.into())
}
fn constant(node: &Node) -> Option<BigInt> {
    match node {
        Node::Atom(value, false)
            if !value.is_empty()
                && value.chars().all(|c| c.is_ascii_digit())
                && !(value.len() > 1 && value.starts_with('0')) =>
        {
            value.parse().ok()
        }
        Node::List(values) if values.len() > 1 => {
            let operator = match &values[0] {
                Node::Atom(value, false) => value.as_str(),
                _ => return None,
            };
            if !["+", "-", "*"].contains(&operator) {
                return None;
            }
            let mut args = values[1..].iter().map(constant);
            let mut result = args.next()??;
            if operator == "-" && values.len() == 2 {
                return Some(-result);
            }
            if ["+", "*"].contains(&operator) && values.len() < 3 {
                return None;
            }
            for value in args {
                if operator == "*" {
                    result *= value?;
                } else if operator == "+" {
                    result += value?;
                } else {
                    result -= value?;
                }
            }
            Some(result)
        }
        _ => None,
    }
}
fn collect_constants(
    node: &Node,
    variables: &BTreeMap<String, (Variable, usize)>,
    visible: usize,
    values: &mut BTreeMap<String, BigInt>,
) {
    if let Node::List(nodes) = node {
        if matches!(nodes.first(),Some(Node::Atom(op,false)) if op=="and") {
            for node in &nodes[1..] {
                collect_constants(node, variables, visible, values);
            }
        }
        if nodes.len() == 3 && matches!(&nodes[0],Node::Atom(op,false) if op=="=") {
            for (left, right) in [(&nodes[1], &nodes[2]), (&nodes[2], &nodes[1])] {
                if let Node::Atom(name, _) = left
                    && variables.get(name).is_some_and(|(variable, order)| {
                        matches!(variable, Variable::Int(_)) && *order < visible
                    })
                    && let Some(value) = constant(right)
                {
                    values.entry(name.clone()).or_insert(value);
                }
            }
        }
    }
}

fn inequality(mut left: Linear, right: &Linear, strict: bool) -> Result<Arc<Formula>> {
    left.add(right, -1);
    let raw_bound = -left.constant - BigInt::from(u8::from(strict));
    let scale = left
        .coefficients
        .values()
        .next()
        .map(|value| value.abs())
        .unwrap_or_else(|| BigInt::from(1));
    let quotient = &raw_bound / &scale;
    let bound = if raw_bound < BigInt::zero() && &raw_bound % &scale != BigInt::zero() {
        quotient - 1
    } else {
        quotient
    };
    let mut from = 0;
    let mut to = 0;
    for (slot, coefficient) in left.coefficients {
        if coefficient == scale && to == 0 {
            to = slot;
        } else if coefficient == -&scale && from == 0 {
            from = slot;
        } else {
            return Err(Error::new(
                "unsupported",
                "constraint is not integer difference logic after constant substitution",
            ));
        }
    }
    Ok(Arc::new(if from == 0 && to == 0 {
        Formula::Constant(bound >= BigInt::zero())
    } else {
        Formula::Edge(Edge { from, to, bound })
    }))
}
fn equality(left: Term, right: Term) -> Result<Arc<Formula>> {
    match (left, right) {
        (Term::Bool(left), Term::Bool(right)) => Ok(Arc::new(Formula::Iff(left, right))),
        (Term::Int(left), Term::Int(right)) => integer_predicate(&left, &right, |left, right| {
            Ok(Arc::new(Formula::And(vec![
                inequality(left.clone(), right, false)?,
                inequality(right.clone(), left, false)?,
            ])))
        }),
        _ => Err(Error::new("type", "equality operands have different sorts")),
    }
}
impl Compiler {
    fn term(&self, node: &Node, locals: &BTreeMap<String, Term>, depth: usize) -> Result<Term> {
        if depth > 32 {
            return Err(Error::new("resource", "term nesting exceeds 32"));
        }
        match node {
            Node::Atom(value, false) if value == "true" || value == "false" => {
                Ok(Term::Bool(Arc::new(Formula::Constant(value == "true"))))
            }
            Node::Atom(value, false) if value.chars().all(|c| c.is_ascii_digit()) => {
                let value =
                    constant(node).ok_or_else(|| Error::new("syntax", "invalid numeral"))?;
                Ok(Term::Int(Integer::plain(Linear {
                    constant: value,
                    ..Linear::default()
                })))
            }
            Node::Atom(name, _) => {
                if let Some(value) = locals.get(name) {
                    return Ok(value.clone());
                }
                let (variable, order) = self
                    .variables
                    .get(name)
                    .ok_or_else(|| Error::new("type", format!("undeclared symbol {name}")))?;
                if *order >= self.visible {
                    return Err(Error::new(
                        "type",
                        format!("symbol {name} declared after assertion"),
                    ));
                }
                Ok(match variable {
                    Variable::Bool(slot) => Term::Bool(Arc::new(Formula::Boolean(*slot))),
                    Variable::Int(slot) => Term::Int(Integer::plain(
                        if let Some(value) = self.constants.get(name) {
                            Linear {
                                constant: value.clone(),
                                ..Linear::default()
                            }
                        } else {
                            Linear {
                                coefficients: BTreeMap::from([(*slot, BigInt::from(1))]),
                                ..Linear::default()
                            }
                        },
                    )),
                })
            }
            Node::List(nodes) => {
                let operator = match nodes.first() {
                    Some(Node::Atom(operator, false)) => operator.as_str(),
                    _ => return Err(Error::new("syntax", "expected built-in operator")),
                };
                let args = &nodes[1..];
                if args.len() > 64 {
                    return Err(Error::new("resource", "operator arity exceeds 64"));
                }
                if operator == "let" {
                    if args.len() != 2 {
                        return Err(Error::new("syntax", "let arity"));
                    }
                    let mut child = locals.clone();
                    let mut seen = std::collections::HashSet::new();
                    for binding in list(&args[0])? {
                        let binding = list(binding)?;
                        if binding.len() != 2 {
                            return Err(Error::new("syntax", "let binding arity"));
                        }
                        let name = name(&binding[0])?;
                        if !seen.insert(name.clone()) {
                            return Err(Error::new("syntax", "duplicate let binding"));
                        }
                        child.insert(name, self.term(&binding[1], locals, depth + 1)?);
                    }
                    return self.term(&args[1], &child, depth + 1);
                }
                let terms = args
                    .iter()
                    .map(|node| self.term(node, locals, depth + 1))
                    .collect::<Result<Vec<_>>>()?;
                match operator {
                    "+" | "-" | "*" => {
                        if terms.is_empty() || (["+", "*"].contains(&operator) && terms.len() < 2) {
                            return Err(Error::new("type", "arithmetic arity"));
                        }
                        let mut values = terms.into_iter().map(Term::integer);
                        let mut result = values
                            .next()
                            .ok_or_else(|| Error::new("type", "arithmetic arity"))??;
                        if operator == "-" && args.len() == 1 {
                            return Ok(Term::Int(Integer {
                                cases: result
                                    .cases
                                    .into_iter()
                                    .map(|case| IntCase {
                                        guard: case.guard,
                                        value: case.value.scaled(&BigInt::from(-1)),
                                    })
                                    .collect(),
                            }));
                        }
                        for value in values {
                            result = result.combine(&value?, |left, right| {
                                if operator == "*" {
                                    if left.coefficients.is_empty() {
                                        Ok(right.scaled(&left.constant))
                                    } else if right.coefficients.is_empty() {
                                        Ok(left.scaled(&right.constant))
                                    } else {
                                        Err(Error::new("unsupported", "nonlinear multiplication"))
                                    }
                                } else {
                                    let mut value = left.clone();
                                    value.add(right, if operator == "+" { 1 } else { -1 });
                                    Ok(value)
                                }
                            })?;
                        }
                        Ok(Term::Int(result))
                    }
                    "and" | "or" => {
                        if terms.is_empty() {
                            return Err(Error::new("type", "Boolean application needs arguments"));
                        }
                        let values = terms
                            .into_iter()
                            .map(Term::boolean)
                            .collect::<Result<Vec<_>>>()?;
                        Ok(Term::Bool(Arc::new(if operator == "and" {
                            Formula::And(values)
                        } else {
                            Formula::Or(values)
                        })))
                    }
                    "not" => {
                        if terms.len() != 1 {
                            return Err(Error::new("type", "not arity"));
                        }
                        Ok(Term::Bool(Arc::new(Formula::Not(
                            terms[0].clone().boolean()?,
                        ))))
                    }
                    "=>" | "xor" => {
                        if terms.len() < 2 || terms.len() > 16 {
                            return Err(Error::new(
                                "unsupported",
                                "implication/xor arity requires 2..16",
                            ));
                        }
                        let values = terms
                            .into_iter()
                            .map(Term::boolean)
                            .collect::<Result<Vec<_>>>()?;
                        let result = if operator == "=>" {
                            let mut result = values
                                .last()
                                .cloned()
                                .ok_or_else(|| Error::new("type", "implication arity"))?;
                            for value in values[..values.len() - 1].iter().rev() {
                                result = Arc::new(Formula::Or(vec![
                                    Arc::new(Formula::Not(value.clone())),
                                    result,
                                ]));
                            }
                            result
                        } else {
                            let mut result = values[0].clone();
                            for value in &values[1..] {
                                result = Arc::new(Formula::Not(Arc::new(Formula::Iff(
                                    result,
                                    value.clone(),
                                ))));
                            }
                            result
                        };
                        Ok(Term::Bool(result))
                    }
                    "ite" => {
                        if terms.len() != 3 {
                            return Err(Error::new("type", "ite arity"));
                        }
                        let condition = terms[0].clone().boolean()?;
                        match (terms[1].clone(), terms[2].clone()) {
                            (Term::Bool(consequent), Term::Bool(alternative)) => {
                                Ok(Term::Bool(Arc::new(Formula::Or(vec![
                                    Arc::new(Formula::And(vec![condition.clone(), consequent])),
                                    Arc::new(Formula::And(vec![
                                        Arc::new(Formula::Not(condition)),
                                        alternative,
                                    ])),
                                ]))))
                            }
                            (Term::Int(consequent), Term::Int(alternative)) => {
                                if consequent.cases.len() + alternative.cases.len() > 256 {
                                    return Err(Error::new(
                                        "resource",
                                        "integer alternatives exceed 256",
                                    ));
                                }
                                let cases = consequent
                                    .cases
                                    .into_iter()
                                    .map(|case| IntCase {
                                        guard: conjunction(vec![condition.clone(), case.guard]),
                                        value: case.value,
                                    })
                                    .chain(alternative.cases.into_iter().map(|case| IntCase {
                                        guard: conjunction(vec![
                                            Arc::new(Formula::Not(condition.clone())),
                                            case.guard,
                                        ]),
                                        value: case.value,
                                    }))
                                    .collect();
                                Ok(Term::Int(Integer { cases }))
                            }
                            _ => Err(Error::new("type", "ite branches have different sorts")),
                        }
                    }
                    "=" | "distinct" => {
                        if terms.len() < 2 {
                            return Err(Error::new("type", "equality arity"));
                        }
                        let mut comparisons = vec![];
                        for index in 0..terms.len() - 1 {
                            let others = if operator == "distinct" {
                                &terms[index + 1..]
                            } else {
                                &terms[index + 1..index + 2]
                            };
                            for other in others {
                                let equal = equality(terms[index].clone(), other.clone())?;
                                comparisons.push(if operator == "distinct" {
                                    Arc::new(Formula::Not(equal))
                                } else {
                                    equal
                                });
                            }
                        }
                        Ok(Term::Bool(Arc::new(Formula::And(comparisons))))
                    }
                    "<" | "<=" | ">" | ">=" => {
                        if terms.len() < 2 {
                            return Err(Error::new("type", "comparison arity"));
                        }
                        let values = terms
                            .into_iter()
                            .map(Term::integer)
                            .collect::<Result<Vec<_>>>()?;
                        let mut comparisons = vec![];
                        for pair in values.windows(2) {
                            let (left, right) = if operator.starts_with('>') {
                                (&pair[1], &pair[0])
                            } else {
                                (&pair[0], &pair[1])
                            };
                            comparisons.push(integer_predicate(left, right, |left, right| {
                                inequality(left.clone(), right, operator.len() == 1)
                            })?);
                        }
                        Ok(Term::Bool(Arc::new(Formula::And(comparisons))))
                    }
                    _ => Err(Error::new("unsupported", format!("operator {operator}"))),
                }
            }
        }
    }
}
pub(super) fn compile(source: &str) -> Result<Problem> {
    let nodes = parse(source)?;
    let mut variables = BTreeMap::new();
    let mut integers = 0;
    let mut booleans = 0;
    let mut assertions = vec![];
    let mut checked = false;
    let mut logic = false;
    for node in &nodes {
        if checked {
            return Err(Error::new("unsupported", "commands after check-sat"));
        }
        let command = list(node)?;
        let operator = match command.first() {
            Some(Node::Atom(operator, false)) => operator.as_str(),
            _ => return Err(Error::new("syntax", "expected command")),
        };
        match operator {
            "set-logic" => {
                if logic || !variables.is_empty() || !assertions.is_empty() || command.len() != 2 {
                    return Err(Error::new("syntax", "set-logic position/arity"));
                }
                if !["QF_IDL", "QF_LIA"].contains(&symbol(&command[1])?) {
                    return Err(Error::new("unsupported", "logic"));
                }
                logic = true;
            }
            "set-option" => {
                if command.len() != 3
                    || symbol(&command[1])? != ":produce-models"
                    || !matches!(&command[2],Node::Atom(value,false) if value=="true")
                {
                    return Err(Error::new("unsupported", "option"));
                }
            }
            "declare-const" | "declare-fun" => {
                let sort = if operator == "declare-const" {
                    if command.len() != 3 {
                        return Err(Error::new("syntax", "declaration arity"));
                    }
                    &command[2]
                } else {
                    if command.len() != 4 || !list(&command[2])?.is_empty() {
                        return Err(Error::new("unsupported", "only zero-argument functions"));
                    }
                    &command[3]
                };
                let name = name(&command[1])?;
                let variable = match symbol(sort)? {
                    "Int" => {
                        integers += 1;
                        Variable::Int(integers)
                    }
                    "Bool" => {
                        let slot = booleans;
                        booleans += 1;
                        Variable::Bool(slot)
                    }
                    _ => return Err(Error::new("unsupported", "sort")),
                };
                let order = variables.len();
                if variables.insert(name, (variable, order)).is_some() {
                    return Err(Error::new("type", "duplicate declaration"));
                }
                if variables.len() > 4096 {
                    return Err(Error::new("resource", "more than 4096 declarations"));
                }
            }
            "assert" => {
                if command.len() != 2 {
                    return Err(Error::new("syntax", "assert arity"));
                }
                assertions.push((command[1].clone(), variables.len()));
            }
            "check-sat" => {
                if command.len() != 1 {
                    return Err(Error::new("syntax", "check-sat arity"));
                }
                checked = true;
            }
            _ => return Err(Error::new("unsupported", format!("command {operator}"))),
        }
    }
    let mut constants = BTreeMap::new();
    for (assertion, visible) in &assertions {
        collect_constants(assertion, &variables, *visible, &mut constants);
    }
    let mut anchors = vec![];
    for (name, value) in &constants {
        if let Some((Variable::Int(slot), _)) = variables.get(name) {
            anchors.extend([
                Edge {
                    from: 0,
                    to: *slot,
                    bound: value.clone(),
                },
                Edge {
                    from: *slot,
                    to: 0,
                    bound: -value,
                },
            ]);
        }
    }
    let mut compiler = Compiler {
        variables,
        constants,
        visible: 0,
    };
    let mut formulas = vec![];
    for (assertion, visible) in &assertions {
        compiler.visible = *visible;
        formulas.push(compiler.term(assertion, &BTreeMap::new(), 0)?.boolean()?);
    }
    Ok(Problem {
        variables: compiler
            .variables
            .into_iter()
            .map(|(name, (variable, _))| (name, variable))
            .collect(),
        root: Arc::new(Formula::And(formulas)),
        anchors,
        integers,
        booleans,
    })
}

fn render(node: &Node) -> String {
    match node {
        Node::Atom(value, true) => format!("|{value}|"),
        Node::Atom(value, false) => value.clone(),
        Node::List(nodes) => format!(
            "({})",
            nodes.iter().map(render).collect::<Vec<_>>().join(" ")
        ),
    }
}
pub(super) fn session_source(source: &str) -> Result<String> {
    let mut nodes = parse(source)?;
    if matches!(nodes.last(), Some(Node::List(command)) if matches!(command.first(), Some(Node::Atom(name,false)) if name == "check-sat"))
    {
        nodes.pop();
    }
    let source = nodes.iter().map(render).collect::<Vec<_>>().join("\n");
    if source.len() > 1_048_576 {
        return Err(Error::new("resource", "normalized source exceeds 1 MiB"));
    }
    Ok(source)
}
pub(super) fn assertion(expression: &str) -> Result<String> {
    let nodes = parse(expression)?;
    if nodes.len() != 1 {
        return Err(Error::new("syntax", "assert needs a single expression"));
    }
    Ok(format!("(assert {})", render(&nodes[0])))
}

pub(super) fn script(source: &str) -> Result<Vec<(Arc<Problem>, Vec<Assumption>)>> {
    let nodes = parse(source)?;
    let mut prefix = String::new();
    let mut solver = None;
    let mut queries = Vec::new();
    for node in &nodes {
        let command = list(node)?;
        let operator = match command.first() {
            Some(Node::Atom(operator, false)) => operator.as_str(),
            _ => return Err(Error::new("syntax", "expected command")),
        };
        if ["set-logic", "set-option", "declare-const", "declare-fun"].contains(&operator) {
            if solver.is_some() {
                return Err(Error::new(
                    "unsupported",
                    "declarations and options must precede script operations",
                ));
            }
            prefix.push_str(&render(node));
            prefix.push('\n');
            continue;
        }
        if solver.is_none() {
            solver = Some(Solver::parse(&prefix)?);
        }
        let context = solver.as_mut().expect("initialized script context");
        match operator {
            "assert" if command.len() == 2 => context.assert(&render(&command[1]))?,
            "push" | "pop" if command.len() == 2 => {
                let numeral = match &command[1] {
                    Node::Atom(value, false)
                        if value == "0"
                            || !value.starts_with('0')
                                && value.chars().all(|character| character.is_ascii_digit()) =>
                    {
                        value
                    }
                    _ => return Err(Error::new("syntax", "scope count must be a numeral")),
                };
                let count = numeral
                    .parse::<u32>()
                    .map_err(|_| Error::new("resource", "scope count exceeds u32"))?;
                if operator == "push" {
                    context.push(count)?;
                } else {
                    context.pop(count)?;
                }
            }
            "check-sat" | "check-sat-assuming" => {
                if queries.len() == 256 {
                    return Err(Error::new("resource", "script checks exceed 256"));
                }
                let assumptions = if operator == "check-sat" && command.len() == 1 {
                    vec![]
                } else if operator == "check-sat-assuming" && command.len() == 2 {
                    let literals = list(&command[1])?;
                    if literals.len() > 256 {
                        return Err(Error::new("resource", "assumptions exceed 256"));
                    }
                    literals.iter().map(|literal| match literal {
                        Node::Atom(value, _) => Ok(Assumption { symbol: value.clone(), positive: true }),
                        Node::List(parts) if parts.len() == 2 && matches!(&parts[0], Node::Atom(operator, false) if operator == "not") && matches!(&parts[1], Node::Atom(_, _)) => Ok(Assumption { symbol: symbol(&parts[1])?.to_string(), positive: false }),
                        _ => Err(Error::new("unsupported", "script assumptions require Bool literals")),
                    }).collect::<Result<Vec<_>>>()?
                } else {
                    return Err(Error::new("syntax", "check command arity"));
                };
                queries.push((context.snapshot(), assumptions));
            }
            "assert" | "push" | "pop" => return Err(Error::new("syntax", "script command arity")),
            _ => {
                return Err(Error::new(
                    "unsupported",
                    format!("script command {operator}"),
                ));
            }
        }
    }
    if solver.is_none() {
        Solver::parse(&prefix)?;
    }
    Ok(queries)
}
