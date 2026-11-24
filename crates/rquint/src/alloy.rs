//! Quint JSON IR to Alloy 6 for ralloy's SAT (plan 0095), independent of
//! the TLA+ lowering except for reading the IR.
//!
//! State variables are `var` fields of `one sig State`: integers as
//! `one Int`, sum types of unit variants as one sigs. Expressions that do not
//! depend on the state are evaluated here, folds included (a set folds in
//! ascending order, TLC's normalized `CHOOSE` order). A step is an action or
//! a stutter, so dead ends do not change the reachable states, as with TLC's
//! `-deadlock`. Results are bounded by the bit width and steps.
use crate::{Error, Options, Result, text, unsupported};
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};

/// The command whose counterexample means some state-dependent addition,
/// subtraction or negation left the bit width (plan 0096). When it has one,
/// the other commands' verdicts are unknown.
pub const OVERFLOW_LABEL: &str = "c_overflow";

/// Integer bit width and trace length of every command.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub bitwidth: u32,
    pub steps: usize,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            bitwidth: 5,
            steps: 8,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Value {
    Int(i64),
    Bool(bool),
    Variant(String),
    Set(BTreeSet<Value>),
    List(Vec<Value>),
    Tuple(Vec<Value>),
}

#[derive(Clone, Debug)]
enum Binding {
    Constant(Value),
    /// An Alloy integer atom bound by a quantifier.
    Atom(String),
}

enum VarType {
    Int,
    Sum(String),
}

struct Lowering<'a> {
    definitions: &'a BTreeMap<String, Json>,
    vars: BTreeMap<String, VarType>,
    /// Variant tag to its sum type.
    tags: BTreeMap<String, String>,
    bounds: Bounds,
    fresh: usize,
    /// No-overflow conditions of state-dependent arithmetic, each closed
    /// over the nondet atoms in scope where it occurs.
    overflow: Vec<String>,
    /// Nondet atoms in scope with their membership formulas.
    binders: Vec<(String, String)>,
}

pub fn to_alloy(ir: &str, options: &Options, bounds: Bounds) -> Result<String> {
    let ir: Json = serde_json::from_str(ir).map_err(|error| Error(format!("IR JSON: {error}")))?;
    if ir["errors"]
        .as_array()
        .is_some_and(|errors| !errors.is_empty())
    {
        return Err(Error("Quint reported errors".into()));
    }
    let main = ir["main"]
        .as_str()
        .ok_or_else(|| Error("IR without a main module".into()))?;
    let module = ir["modules"]
        .as_array()
        .and_then(|modules| modules.iter().find(|module| module["name"] == main))
        .ok_or_else(|| Error(format!("main module {main} not in the IR")))?;
    let mut definitions = BTreeMap::new();
    let mut sums: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut declared = Vec::new();
    for declaration in module["declarations"].as_array().into_iter().flatten() {
        match declaration["kind"].as_str() {
            Some("var") => declared.push(declaration.clone()),
            Some("def") => {
                let name = text(&declaration["name"])?;
                if !name.contains("::") {
                    definitions.insert(name.to_owned(), declaration.clone());
                }
            }
            Some("typedef") => {
                let kind = &declaration["type"];
                if kind["kind"] != "sum" {
                    return unsupported(format!("type {} for SAT", declaration["name"]));
                }
                let mut tags = Vec::new();
                for field in kind["fields"]["fields"].as_array().into_iter().flatten() {
                    let unit = field["fieldType"]["kind"] == "tup"
                        && field["fieldType"]["fields"]["fields"]
                            .as_array()
                            .is_some_and(Vec::is_empty);
                    if !unit {
                        return unsupported("variants with payloads for SAT");
                    }
                    tags.push(text(&field["fieldName"])?.to_owned());
                }
                sums.insert(text(&declaration["name"])?.to_owned(), tags);
            }
            other => return unsupported(format!("declaration {other:?} for SAT")),
        }
    }
    let mut vars = BTreeMap::new();
    for var in &declared {
        let annotation = &var["typeAnnotation"];
        let kind = match annotation["kind"].as_str() {
            Some("int") => VarType::Int,
            Some("const") if sums.contains_key(text(&annotation["name"])?) => {
                VarType::Sum(text(&annotation["name"])?.to_owned())
            }
            _ => return unsupported(format!("state variable type {annotation} for SAT")),
        };
        vars.insert(text(&var["name"])?.to_owned(), kind);
    }
    for name in [&options.init, &options.step]
        .into_iter()
        .chain(&options.invariants)
    {
        if !definitions.contains_key(name) {
            return Err(Error(format!("definition {name} not found")));
        }
    }
    let tags = sums
        .iter()
        .flat_map(|(sum, tags)| tags.iter().map(move |tag| (tag.clone(), sum.clone())))
        .collect();
    let mut lowering = Lowering {
        definitions: &definitions,
        vars,
        tags,
        bounds,
        fresh: 0,
        overflow: Vec::new(),
        binders: Vec::new(),
    };
    let mut out = String::new();
    for (sum, tags) in &sums {
        let ones: Vec<String> = tags.iter().map(|tag| format!("V_{tag}")).collect();
        out.push_str(&format!(
            "abstract sig T_{sum} {{}}\none sig {} extends T_{sum} {{}}\n",
            ones.join(", ")
        ));
    }
    let fields: Vec<String> = lowering
        .vars
        .iter()
        .map(|(name, kind)| match kind {
            VarType::Int => format!("var v_{name}: one Int"),
            VarType::Sum(sum) => format!("var v_{name}: one T_{sum}"),
        })
        .collect();
    out.push_str(&format!("one sig State {{ {} }}\n", fields.join(", ")));
    let init = lowering.action(&definitions[&options.init]["expr"], &[], false)?;
    out.push_str(&format!("fact {{ {init} }}\n"));
    let step = lowering.action(&definitions[&options.step]["expr"], &[], true)?;
    let stutter: Vec<String> = lowering
        .vars
        .keys()
        .map(|name| format!("(State.v_{name})' = State.v_{name}"))
        .collect();
    out.push_str(&format!(
        "fact {{ always {{ {step} or ({}) }} }}\n",
        stutter.join(" and ")
    ));
    let scope: Vec<String> = sums
        .iter()
        .map(|(sum, tags)| format!("exactly {} T_{sum}", tags.len()))
        .chain([
            format!("{} Int", bounds.bitwidth),
            format!("{} steps", bounds.steps),
        ])
        .collect();
    for invariant in &options.invariants {
        let formula = lowering.formula(&definitions[invariant]["expr"], &[])?;
        out.push_str(&format!(
            "c_{invariant}: check {{ always {{ {formula} }} }} for {}\n",
            scope.join(", ")
        ));
    }
    let guard = if lowering.overflow.is_empty() {
        "(no none)".to_owned()
    } else {
        lowering.overflow.join(" and ")
    };
    out.push_str(&format!(
        "{OVERFLOW_LABEL}: check {{ always {{ {guard} }} }} for {}\n",
        scope.join(", ")
    ));
    Ok(out)
}

type Env = [(String, Binding)];

fn arguments(expression: &Json) -> &[Json] {
    expression["args"].as_array().map_or(&[], Vec::as_slice)
}

impl Lowering<'_> {
    fn lookup<'e>(&self, env: &'e Env, name: &str) -> Option<&'e Binding> {
        env.iter()
            .rev()
            .find(|(bound, _)| bound == name)
            .map(|(_, binding)| binding)
    }
    /// The value of a state-independent expression, or `None`.
    fn evaluate(&self, expression: &Json, env: &Env) -> Result<Option<Value>> {
        Ok(Some(match expression["kind"].as_str() {
            Some("int") => Value::Int(
                expression["value"]
                    .as_i64()
                    .ok_or_else(|| Error("integer literal beyond 64 bits".into()))?,
            ),
            Some("bool") => Value::Bool(expression["value"] == true),
            Some("name") => {
                let name = text(&expression["name"])?;
                match self.lookup(env, name) {
                    Some(Binding::Constant(value)) => return Ok(Some(value.clone())),
                    Some(Binding::Atom(_)) => return Ok(None),
                    None => {}
                }
                if self.vars.contains_key(name) {
                    return Ok(None);
                }
                match self.definitions.get(name) {
                    Some(definition)
                        if definition["qualifier"] == "val"
                            || definition["qualifier"] == "pure" =>
                    {
                        return self.evaluate(&definition["expr"], &[]);
                    }
                    _ => return Ok(None),
                }
            }
            Some("app") => {
                let opcode = text(&expression["opcode"])?;
                let args = arguments(expression);
                if opcode == "variant" {
                    return Ok(Some(Value::Variant(text(&args[0]["value"])?.to_owned())));
                }
                if matches!(opcode, "fold" | "foldl") {
                    let (Some(collection), Some(initial)) =
                        (self.evaluate(&args[0], env)?, self.evaluate(&args[1], env)?)
                    else {
                        return Ok(None);
                    };
                    let elements: Vec<Value> = match collection {
                        Value::Set(set) => set.into_iter().collect(),
                        Value::List(list) => list,
                        _ => return unsupported("fold over a non-collection"),
                    };
                    let mut accumulator = initial;
                    for element in elements {
                        let Some(next) = self.apply(&args[2], env, vec![accumulator, element])?
                        else {
                            return Ok(None);
                        };
                        accumulator = next;
                    }
                    return Ok(Some(accumulator));
                }
                let mut values = Vec::new();
                for argument in args {
                    match self.evaluate(argument, env)? {
                        Some(value) => values.push(value),
                        None => return Ok(None),
                    }
                }
                let int = |index: usize| match values.get(index) {
                    Some(Value::Int(value)) => Ok(*value),
                    other => Err(Error(format!("{opcode} expects integers, got {other:?}"))),
                };
                let boolean = |index: usize| match values.get(index) {
                    Some(Value::Bool(value)) => Ok(*value),
                    other => Err(Error(format!("{opcode} expects booleans, got {other:?}"))),
                };
                match opcode {
                    "iadd" => Value::Int(int(0)? + int(1)?),
                    "isub" => Value::Int(int(0)? - int(1)?),
                    "imul" => Value::Int(int(0)? * int(1)?),
                    "iuminus" => Value::Int(-int(0)?),
                    "ilt" => Value::Bool(int(0)? < int(1)?),
                    "ilte" => Value::Bool(int(0)? <= int(1)?),
                    "igt" => Value::Bool(int(0)? > int(1)?),
                    "igte" => Value::Bool(int(0)? >= int(1)?),
                    "eq" => Value::Bool(values[0] == values[1]),
                    "neq" => Value::Bool(values[0] != values[1]),
                    "and" => {
                        Value::Bool((0..values.len()).all(|index| boolean(index).unwrap_or(false)))
                    }
                    "or" => {
                        Value::Bool((0..values.len()).any(|index| boolean(index).unwrap_or(false)))
                    }
                    "not" => Value::Bool(!boolean(0)?),
                    "implies" => Value::Bool(!boolean(0)? || boolean(1)?),
                    "ite" => {
                        if boolean(0)? {
                            values[1].clone()
                        } else {
                            values[2].clone()
                        }
                    }
                    "Set" => Value::Set(values.into_iter().collect()),
                    "List" => Value::List(values),
                    "Tup" => Value::Tuple(values),
                    _ => return Ok(None),
                }
            }
            _ => return Ok(None),
        }))
    }
    /// A lambda applied to constant arguments.
    fn apply(&self, lambda: &Json, env: &Env, values: Vec<Value>) -> Result<Option<Value>> {
        if lambda["kind"] != "lambda" {
            return unsupported("an operator argument that is not a lambda");
        }
        let params = lambda["params"].as_array().map_or(&[][..], Vec::as_slice);
        if params.len() != values.len() {
            return Err(Error("lambda arity".into()));
        }
        let mut inner: Vec<(String, Binding)> = env.to_vec();
        for (param, value) in params.iter().zip(values) {
            inner.push((text(&param["name"])?.to_owned(), Binding::Constant(value)));
        }
        self.evaluate(&lambda["expr"], &inner)
    }
    fn literal(&self, value: &Value) -> Result<String> {
        match value {
            Value::Int(value) => {
                let half = 1i64 << (self.bounds.bitwidth - 1);
                if !(-half..half).contains(value) {
                    return Err(Error(format!(
                        "integer {value} outside the {}-bit range",
                        self.bounds.bitwidth
                    )));
                }
                Ok(if *value < 0 {
                    format!("negate[{}]", -value)
                } else {
                    value.to_string()
                })
            }
            Value::Variant(tag) if self.tags.contains_key(tag) => Ok(format!("V_{tag}")),
            Value::Bool(true) => Ok("(no none)".into()),
            Value::Bool(false) => Ok("(some univ)".into()),
            other => unsupported(format!("value {other:?} for SAT")),
        }
    }
    /// An integer or variant valued expression.
    fn term(&mut self, expression: &Json, env: &Env) -> Result<String> {
        if let Some(value) = self.evaluate(expression, env)? {
            return self.literal(&value);
        }
        match expression["kind"].as_str() {
            Some("name") => {
                let name = text(&expression["name"])?;
                if let Some(Binding::Atom(atom)) = self.lookup(env, name) {
                    return Ok(atom.clone());
                }
                if self.vars.contains_key(name) {
                    return Ok(format!("State.v_{name}"));
                }
                unsupported(format!("name {name} as a SAT term"))
            }
            Some("app") => {
                let opcode = text(&expression["opcode"])?;
                let args = arguments(expression);
                let minimum = -(1i64 << (self.bounds.bitwidth - 1));
                match opcode {
                    "iuminus" => {
                        let value = self.term(&args[0], env)?;
                        self.guard(format!("({value} != {minimum})"));
                        Ok(format!("negate[{value}]"))
                    }
                    "iadd" | "isub" => {
                        let left = self.term(&args[0], env)?;
                        let right = self.term(&args[1], env)?;
                        let function = if opcode == "iadd" { "plus" } else { "minus" };
                        let result = format!("{function}[{left}, {right}]");
                        // Two's complement: a sum of same-signed operands,
                        // or a difference of opposite-signed ones, keeps the
                        // left operand's sign.
                        let (nonnegative, negative) = if opcode == "iadd" {
                            ((">= 0", ">= 0"), ("< 0", "< 0"))
                        } else {
                            ((">= 0", "< 0"), ("< 0", ">= 0"))
                        };
                        self.guard(format!(
                            "(({left} {} and {right} {}) implies {result} >= 0) and (({left} {} and {right} {}) implies {result} < 0)",
                            nonnegative.0, nonnegative.1, negative.0, negative.1
                        ));
                        Ok(result)
                    }
                    "imul" => unsupported("multiplication of state-dependent integers for SAT"),
                    other => unsupported(format!("operator {other} as a SAT term")),
                }
            }
            other => unsupported(format!("expression {other:?} as a SAT term")),
        }
    }
    /// Records a no-overflow condition, closed over the nondet atoms in scope.
    fn guard(&mut self, condition: String) {
        let mut closed = condition;
        for (atom, membership) in self.binders.iter().rev() {
            closed = format!("(all {atom}: Int | ({membership}) implies ({closed}))");
        }
        self.overflow.push(format!("({closed})"));
    }
    fn formula(&mut self, expression: &Json, env: &Env) -> Result<String> {
        if let Some(value) = self.evaluate(expression, env)? {
            return match value {
                Value::Bool(_) => self.literal(&value),
                other => Err(Error(format!("{other:?} is not a formula"))),
            };
        }
        match expression["kind"].as_str() {
            Some("name") => {
                let name = text(&expression["name"])?;
                match self.definitions.get(name) {
                    Some(definition) => self.formula(&definition["expr"], &[]),
                    None => unsupported(format!("name {name} as a SAT formula")),
                }
            }
            Some("app") => {
                let opcode = text(&expression["opcode"])?;
                let args = arguments(expression);
                Ok(match opcode {
                    "and" | "or" => {
                        let parts = args
                            .iter()
                            .map(|argument| self.formula(argument, env))
                            .collect::<Result<Vec<_>>>()?;
                        let joiner = if opcode == "and" { " and " } else { " or " };
                        format!("({})", parts.join(joiner))
                    }
                    "not" => format!("(not {})", self.formula(&args[0], env)?),
                    "implies" => format!(
                        "({} implies {})",
                        self.formula(&args[0], env)?,
                        self.formula(&args[1], env)?
                    ),
                    "iff" => format!(
                        "({} iff {})",
                        self.formula(&args[0], env)?,
                        self.formula(&args[1], env)?
                    ),
                    "eq" | "neq" | "ilt" | "ilte" | "igt" | "igte" => {
                        let symbol = match opcode {
                            "eq" => "=",
                            "neq" => "!=",
                            "ilt" => "<",
                            "ilte" => "<=",
                            "igt" => ">",
                            _ => ">=",
                        };
                        format!(
                            "({} {symbol} {})",
                            self.term(&args[0], env)?,
                            self.term(&args[1], env)?
                        )
                    }
                    "ite" => {
                        let condition = self.formula(&args[0], env)?;
                        format!(
                            "(({condition} and {}) or (not {condition} and {}))",
                            self.formula(&args[1], env)?,
                            self.formula(&args[2], env)?
                        )
                    }
                    "matchVariant" => {
                        let subject = self.term(&args[0], env)?;
                        self.cases(&subject, &args[1..], |lowering, body| {
                            lowering.formula(body, env)
                        })?
                    }
                    other => return unsupported(format!("operator {other} as a SAT formula")),
                })
            }
            other => unsupported(format!("expression {other:?} as a SAT formula")),
        }
    }
    /// `match` as a disjunction over tags, `_` as the remaining ones.
    fn cases(
        &mut self,
        subject: &str,
        pairs: &[Json],
        mut body: impl FnMut(&mut Self, &Json) -> Result<String>,
    ) -> Result<String> {
        let mut covered = Vec::new();
        let mut parts = Vec::new();
        for pair in pairs.chunks(2) {
            let [label, handler] = pair else {
                return Err(Error("matchVariant needs label/handler pairs".into()));
            };
            let label = text(&label["value"])?;
            let inner = body(self, &handler["expr"])?;
            if label == "_" {
                let others = if covered.is_empty() {
                    "(no none)".to_owned()
                } else {
                    format!("not ({subject} in {})", covered.join(" + "))
                };
                parts.push(format!("({others} and {inner})"));
            } else {
                covered.push(format!("V_{label}"));
                parts.push(format!("({subject} = V_{label} and {inner})"));
            }
        }
        Ok(format!("({})", parts.join(" or ")))
    }
    /// An action: assignments are equalities on the next state (`primed`) or
    /// the first state.
    fn action(&mut self, expression: &Json, env: &Env, primed: bool) -> Result<String> {
        match expression["kind"].as_str() {
            Some("name") => {
                let name = text(&expression["name"])?;
                match self.definitions.get(name) {
                    Some(definition) if definition["qualifier"] == "action" => {
                        self.action(&definition["expr"], &[], primed)
                    }
                    _ => self.formula(expression, env),
                }
            }
            Some("let") => {
                let definition = &expression["opdef"];
                let value = &definition["expr"];
                if definition["qualifier"] != "nondet" || value["opcode"] != "oneOf" {
                    return unsupported("let other than nondet oneOf for SAT");
                }
                let Some(Value::Set(set)) = self.evaluate(&arguments(value)[0], env)? else {
                    return unsupported("oneOf over a non-constant set for SAT");
                };
                self.fresh += 1;
                let atom = format!("q_{}", self.fresh);
                let members = set
                    .iter()
                    .map(|member| Ok(format!("{atom} = {}", self.literal(member)?)))
                    .collect::<Result<Vec<_>>>()?;
                if members.is_empty() {
                    return Ok("(some univ)".into());
                }
                let mut inner = env.to_vec();
                inner.push((
                    text(&definition["name"])?.to_owned(),
                    Binding::Atom(atom.clone()),
                ));
                let membership = members.join(" or ");
                self.binders.push((atom.clone(), membership.clone()));
                let body = self.action(&expression["expr"], &inner, primed);
                self.binders.pop();
                Ok(format!("(some {atom}: Int | ({membership}) and {})", body?))
            }
            Some("app") => {
                let opcode = text(&expression["opcode"])?;
                let args = arguments(expression);
                match opcode {
                    "assign" => {
                        let variable = text(&args[0]["name"])?;
                        let target = if primed {
                            format!("(State.v_{variable})'")
                        } else {
                            format!("State.v_{variable}")
                        };
                        self.assignment(&target, &args[1], env)
                    }
                    "and" | "actionAll" | "or" | "actionAny" => {
                        let parts = args
                            .iter()
                            .map(|argument| self.action(argument, env, primed))
                            .collect::<Result<Vec<_>>>()?;
                        let joiner = if matches!(opcode, "and" | "actionAll") {
                            " and "
                        } else {
                            " or "
                        };
                        Ok(format!("({})", parts.join(joiner)))
                    }
                    "ite" => {
                        let condition = self.formula(&args[0], env)?;
                        Ok(format!(
                            "(({condition} and {}) or (not {condition} and {}))",
                            self.action(&args[1], env, primed)?,
                            self.action(&args[2], env, primed)?
                        ))
                    }
                    _ => self.formula(expression, env),
                }
            }
            _ => self.formula(expression, env),
        }
    }
    fn assignment(&mut self, target: &str, value: &Json, env: &Env) -> Result<String> {
        if self.evaluate(value, env)?.is_none() && value["kind"] == "app" {
            let args = arguments(value);
            match value["opcode"].as_str() {
                Some("ite") => {
                    let condition = self.formula(&args[0], env)?;
                    return Ok(format!(
                        "(({condition} and {}) or (not {condition} and {}))",
                        self.assignment(target, &args[1], env)?,
                        self.assignment(target, &args[2], env)?
                    ));
                }
                Some("matchVariant") => {
                    let subject = self.term(&args[0], env)?;
                    let target = target.to_owned();
                    return self.cases(&subject, &args[1..], |lowering, body| {
                        lowering.assignment(&target, body, env)
                    });
                }
                _ => {}
            }
        }
        Ok(format!("{target} = {}", self.term(value, env)?))
    }
}
