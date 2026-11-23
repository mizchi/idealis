//! Quint JSON IR (`quint compile --target json`) to standard TLA+ and a TLC
//! configuration for rtlc, without Apalache (plan 0089).
//!
//! The output uses only `Integers`, `Sequences`, `FiniteSets` and `TLC`.
//! Set folds pick elements with `CHOOSE x \in S: TRUE`, the order of
//! Apalache's TLC definition of `ApaFoldSet`; variants are records
//! `[tag |-> "Name", value |-> v]`; Quint's 0-based list indices become
//! TLA+'s 1-based ones. Constructs outside the subset are rejected, never
//! given another meaning.
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;
fn unsupported<T>(what: impl fmt::Display) -> Result<T> {
    Err(Error(format!("unsupported Quint construct: {what}")))
}

/// Which definitions to check.
#[derive(Debug, Clone)]
pub struct Options {
    pub init: String,
    pub step: String,
    pub invariants: Vec<String>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            init: "init".into(),
            step: "step".into(),
            invariants: Vec::new(),
        }
    }
}

/// A TLA+ module and its TLC configuration.
#[derive(Debug, Clone)]
pub struct Translation {
    pub source: String,
    pub config: String,
}

pub fn translate(ir: &str, options: &Options) -> Result<Translation> {
    let ir: Value = serde_json::from_str(ir).map_err(|error| Error(format!("IR JSON: {error}")))?;
    if let Some(errors) = ir["errors"].as_array()
        && !errors.is_empty()
    {
        return Err(Error(format!("Quint reported {} errors", errors.len())));
    }
    let main = ir["main"]
        .as_str()
        .ok_or_else(|| Error("IR without a main module".into()))?;
    let module = ir["modules"]
        .as_array()
        .and_then(|modules| modules.iter().find(|module| module["name"] == main))
        .ok_or_else(|| Error(format!("main module {main} not in the IR")))?;
    let mut variables = Vec::new();
    let mut definitions = BTreeMap::new();
    for declaration in module["declarations"].as_array().into_iter().flatten() {
        match declaration["kind"].as_str() {
            Some("var") => variables.push(text(&declaration["name"])?.to_owned()),
            Some("def") => {
                let name = text(&declaration["name"])?;
                // Quint's own aliases for the entry points.
                if name.contains("::") {
                    continue;
                }
                if let Some(qualifier @ ("temporal" | "run")) = declaration["qualifier"].as_str() {
                    return unsupported(format!("{qualifier} definition {name}"));
                }
                definitions.insert(name.to_owned(), declaration.clone());
            }
            Some("typedef") => {}
            Some(other) => return unsupported(format!("declaration {other}")),
            None => return Err(Error("declaration without kind".into())),
        }
    }
    for name in [&options.init, &options.step]
        .into_iter()
        .chain(&options.invariants)
    {
        if !definitions.contains_key(name) {
            return Err(Error(format!("definition {name} not found")));
        }
    }
    let globals: BTreeSet<String> = variables
        .iter()
        .cloned()
        .chain(definitions.keys().cloned())
        .collect();
    let mut emitter = Emitter {
        globals: &globals,
        definitions: &definitions,
        scopes: Vec::new(),
        fresh: 0,
        initial: false,
    };
    let mut emitted = BTreeMap::new();
    for (name, declaration) in &definitions {
        // TLA+'s Init is a predicate over unprimed variables; Quint's init
        // assigns them.
        emitter.initial = name == &options.init;
        emitted.insert(name.clone(), emitter.definition(name, declaration)?);
    }
    let mut source = format!(
        "---- MODULE {} ----\nEXTENDS Integers, Sequences, FiniteSets, TLC\n",
        identifier(main)
    );
    if !variables.is_empty() {
        let names: Vec<String> = variables.iter().map(|name| identifier(name)).collect();
        source.push_str(&format!("VARIABLES {}\n", names.join(", ")));
    }
    for name in order(&definitions)? {
        source.push_str(&emitted[&name]);
        source.push('\n');
    }
    source.push_str("====\n");
    let mut config = format!(
        "INIT {}\nNEXT {}\n",
        identifier(&options.init),
        identifier(&options.step)
    );
    for invariant in &options.invariants {
        config.push_str(&format!("INVARIANT {}\n", identifier(invariant)));
    }
    Ok(Translation { source, config })
}

fn text(value: &Value) -> Result<&str> {
    value
        .as_str()
        .ok_or_else(|| Error(format!("expected a string, got {value}")))
}
/// A TLA+ identifier for a Quint name.
fn identifier(name: &str) -> String {
    name.replace("::", "_")
}

/// Definitions in an order where each one follows the ones it uses.
fn order(definitions: &BTreeMap<String, Value>) -> Result<Vec<String>> {
    fn uses(value: &Value, names: &BTreeMap<String, Value>, found: &mut BTreeSet<String>) {
        match value {
            Value::Object(object) => {
                if object.get("kind").and_then(Value::as_str) == Some("name")
                    && let Some(name) = object.get("name").and_then(Value::as_str)
                    && names.contains_key(name)
                {
                    found.insert(name.to_owned());
                }
                for child in object.values() {
                    uses(child, names, found);
                }
            }
            Value::Array(values) => values.iter().for_each(|child| uses(child, names, found)),
            _ => {}
        }
    }
    let mut pending: BTreeMap<String, BTreeSet<String>> = definitions
        .iter()
        .map(|(name, declaration)| {
            let mut found = BTreeSet::new();
            uses(&declaration["expr"], definitions, &mut found);
            found.remove(name);
            (name.clone(), found)
        })
        .collect();
    let mut ordered = Vec::new();
    while !pending.is_empty() {
        let ready: Vec<String> = pending
            .iter()
            .filter(|(_, uses)| uses.iter().all(|name| !pending.contains_key(name)))
            .map(|(name, _)| name.clone())
            .collect();
        if ready.is_empty() {
            return unsupported("recursive definitions");
        }
        for name in ready {
            pending.remove(&name);
            ordered.push(name);
        }
    }
    Ok(ordered)
}

struct Emitter<'a> {
    globals: &'a BTreeSet<String>,
    definitions: &'a BTreeMap<String, Value>,
    /// Emitting the initializer: assignments are unprimed equalities and
    /// the actions it calls are inlined.
    initial: bool,
    /// Bound names (lambda, let, nondet) with their unique TLA+ names.
    scopes: Vec<(String, String)>,
    fresh: usize,
}

impl Emitter<'_> {
    fn definition(&mut self, name: &str, declaration: &Value) -> Result<String> {
        let expression = &declaration["expr"];
        if expression["kind"] == "lambda" {
            let (params, body) = self.lambda(expression)?;
            let tla_params: Vec<String> = params.iter().map(|(_, tla)| tla.clone()).collect();
            let mark = self.scopes.len();
            self.scopes.extend(params);
            let body = self.expression(body);
            self.scopes.truncate(mark);
            return Ok(format!(
                "{}({}) == {}",
                identifier(name),
                tla_params.join(", "),
                body?
            ));
        }
        Ok(format!(
            "{} == {}",
            identifier(name),
            self.expression(expression)?
        ))
    }
    /// A lambda's parameters with fresh TLA+ names, and its body.
    fn lambda<'v>(&mut self, lambda: &'v Value) -> Result<(Vec<(String, String)>, &'v Value)> {
        if lambda["kind"] != "lambda" {
            return unsupported("an operator argument that is not a lambda");
        }
        let params = lambda["params"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|param| {
                let name = text(&param["name"])?.to_owned();
                Ok((name.clone(), self.bound(&name)))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((params, &lambda["expr"]))
    }
    fn bound(&mut self, name: &str) -> String {
        self.fresh += 1;
        let stem: String = name
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .collect();
        format!("q_{stem}_{}", self.fresh)
    }
    /// Emits `body` with one lambda's parameters bound to TLA+ expressions.
    fn apply(&mut self, lambda: &Value, arguments: &[String]) -> Result<String> {
        let (params, body) = self.lambda(lambda)?;
        if params.len() != arguments.len() {
            return Err(Error(format!(
                "lambda takes {} parameters, given {}",
                params.len(),
                arguments.len()
            )));
        }
        let bindings: Vec<String> = params
            .iter()
            .zip(arguments)
            .map(|((_, tla), argument)| format!("{tla} == {argument}"))
            .collect();
        let mark = self.scopes.len();
        self.scopes.extend(params);
        let body = self.expression(body);
        self.scopes.truncate(mark);
        Ok(format!("(LET {} IN {})", bindings.join("  "), body?))
    }
    /// A set comprehension or quantifier over one lambda parameter.
    fn binder(&mut self, set: &Value, lambda: &Value) -> Result<(String, String, String)> {
        let set = self.expression(set)?;
        let (params, body) = self.lambda(lambda)?;
        let [(name, tla)] = params.as_slice() else {
            return unsupported("a set operator lambda with several parameters");
        };
        let (name, tla) = (name.clone(), tla.clone());
        self.scopes.push((name, tla.clone()));
        let body = self.expression(body);
        self.scopes.pop();
        Ok((tla, set, body?))
    }

    fn expression(&mut self, expression: &Value) -> Result<String> {
        match expression["kind"].as_str() {
            Some("int") => Ok(match &expression["value"] {
                Value::Number(number) => {
                    let value = number.to_string();
                    if value.starts_with('-') {
                        format!("({value})")
                    } else {
                        value
                    }
                }
                Value::String(value) => value.clone(),
                other => return Err(Error(format!("integer literal {other}"))),
            }),
            Some("bool") => Ok(if expression["value"] == true {
                "TRUE".into()
            } else {
                "FALSE".into()
            }),
            Some("str") => Ok(format!("{:?}", text(&expression["value"])?)),
            Some("name") => {
                let name = text(&expression["name"])?;
                if let Some((_, tla)) = self.scopes.iter().rev().find(|(quint, _)| quint == name) {
                    return Ok(tla.clone());
                }
                if self.initial
                    && let Some(definition) = self.definitions.get(name)
                    && definition["qualifier"] == "action"
                {
                    return Ok(format!("({})", self.expression(&definition["expr"])?));
                }
                if self.globals.contains(name) {
                    return Ok(identifier(name));
                }
                match name {
                    "Bool" => Ok("BOOLEAN".into()),
                    "Int" => Ok("Int".into()),
                    "Nat" => Ok("Nat".into()),
                    _ => Err(Error(format!("unresolved name {name}"))),
                }
            }
            Some("app") => self.application(expression),
            Some("let") => self.let_in(expression),
            Some("lambda") => unsupported("a lambda outside an operator argument"),
            Some(other) => unsupported(format!("expression {other}")),
            None => Err(Error("expression without kind".into())),
        }
    }

    fn let_in(&mut self, expression: &Value) -> Result<String> {
        let definition = &expression["opdef"];
        let name = text(&definition["name"])?.to_owned();
        let tla = self.bound(&name);
        if definition["qualifier"] == "nondet" {
            let value = &definition["expr"];
            if value["kind"] != "app" || value["opcode"] != "oneOf" {
                return unsupported("nondet without oneOf");
            }
            let set = self.expression(&value["args"][0])?;
            self.scopes.push((name, tla.clone()));
            let body = self.expression(&expression["expr"]);
            self.scopes.pop();
            return Ok(format!("(\\E {tla} \\in {set}: {})", body?));
        }
        let value = if definition["expr"]["kind"] == "lambda" {
            let (params, body) = self.lambda(&definition["expr"])?;
            let tla_params: Vec<String> = params.iter().map(|(_, tla)| tla.clone()).collect();
            let mark = self.scopes.len();
            self.scopes.extend(params);
            let body = self.expression(body);
            self.scopes.truncate(mark);
            format!("{tla}({}) == {}", tla_params.join(", "), body?)
        } else {
            format!("{tla} == {}", self.expression(&definition["expr"])?)
        };
        self.scopes.push((name, tla));
        let body = self.expression(&expression["expr"]);
        self.scopes.pop();
        Ok(format!("(LET {value} IN {})", body?))
    }

    fn application(&mut self, expression: &Value) -> Result<String> {
        let opcode = text(&expression["opcode"])?;
        let arguments = expression["args"].as_array().cloned().unwrap_or_default();
        // A user operator applied to arguments.
        if self.globals.contains(opcode) || self.scopes.iter().any(|(quint, _)| quint == opcode) {
            let target = self
                .scopes
                .iter()
                .rev()
                .find(|(quint, _)| quint == opcode)
                .map_or_else(|| identifier(opcode), |(_, tla)| tla.clone());
            let values = self.all(&arguments)?;
            return Ok(format!("{target}({})", values.join(", ")));
        }
        let at = |index: usize| -> Result<&Value> {
            arguments
                .get(index)
                .ok_or_else(|| Error(format!("{opcode} needs argument {index}")))
        };
        let binary = |emitter: &mut Self, operator: &str| -> Result<String> {
            let left = emitter.expression(at(0)?)?;
            let right = emitter.expression(at(1)?)?;
            Ok(format!("({left} {operator} {right})"))
        };
        Ok(match opcode {
            "and" | "or" | "actionAll" | "actionAny" => {
                let values = self.all(&arguments)?;
                let (operator, empty) = if matches!(opcode, "and" | "actionAll") {
                    ("/\\", "TRUE")
                } else {
                    ("\\/", "FALSE")
                };
                if values.is_empty() {
                    empty.to_owned()
                } else {
                    format!("({})", values.join(&format!(" {operator} ")))
                }
            }
            "not" => format!("(~{})", self.expression(at(0)?)?),
            "implies" => binary(self, "=>")?,
            "iff" => binary(self, "<=>")?,
            "eq" => binary(self, "=")?,
            "neq" => binary(self, "#")?,
            "ite" => format!(
                "(IF {} THEN {} ELSE {})",
                self.expression(at(0)?)?,
                self.expression(at(1)?)?,
                self.expression(at(2)?)?
            ),
            "iadd" => binary(self, "+")?,
            "isub" => binary(self, "-")?,
            "imul" => binary(self, "*")?,
            "idiv" => binary(self, "\\div")?,
            "imod" => binary(self, "%")?,
            "ipow" => binary(self, "^")?,
            "iuminus" => format!("(-{})", self.expression(at(0)?)?),
            "ilt" => binary(self, "<")?,
            "ilte" => binary(self, "<=")?,
            "igt" => binary(self, ">")?,
            "igte" => binary(self, ">=")?,
            "assign" => {
                let target = &arguments
                    .first()
                    .ok_or_else(|| Error("assign without a variable".into()))?;
                let variable = text(&target["name"])?;
                let prime = if self.initial { "" } else { "'" };
                format!(
                    "({}{prime} = {})",
                    identifier(variable),
                    self.expression(at(1)?)?
                )
            }
            "Set" => format!("{{{}}}", self.all(&arguments)?.join(", ")),
            "contains" => {
                let set = self.expression(at(0)?)?;
                format!("({} \\in {set})", self.expression(at(1)?)?)
            }
            "in" => binary(self, "\\in")?,
            "union" => binary(self, "\\cup")?,
            "intersect" => binary(self, "\\cap")?,
            "exclude" => binary(self, "\\")?,
            "subseteq" => binary(self, "\\subseteq")?,
            "size" => format!("Cardinality({})", self.expression(at(0)?)?),
            "to" => binary(self, "..")?,
            "powerset" => format!("(SUBSET {})", self.expression(at(0)?)?),
            "flatten" => format!("(UNION {})", self.expression(at(0)?)?),
            "map" => {
                let (bound, set, body) = self.binder(at(0)?, at(1)?)?;
                format!("{{{body} : {bound} \\in {set}}}")
            }
            "filter" => {
                let (bound, set, body) = self.binder(at(0)?, at(1)?)?;
                format!("{{{bound} \\in {set} : {body}}}")
            }
            "exists" => {
                let (bound, set, body) = self.binder(at(0)?, at(1)?)?;
                format!("(\\E {bound} \\in {set} : {body})")
            }
            "forall" => {
                let (bound, set, body) = self.binder(at(0)?, at(1)?)?;
                format!("(\\A {bound} \\in {set} : {body})")
            }
            "fold" | "foldl" => self.fold(opcode == "fold", at(0)?, at(1)?, at(2)?)?,
            "List" | "Tup" => format!("<<{}>>", self.all(&arguments)?.join(", ")),
            "nth" => {
                let list = self.expression(at(0)?)?;
                format!("{list}[({}) + 1]", self.expression(at(1)?)?)
            }
            "item" => {
                let tuple = self.expression(at(0)?)?;
                format!("{tuple}[{}]", self.expression(at(1)?)?)
            }
            "length" => format!("Len({})", self.expression(at(0)?)?),
            "append" => format!(
                "Append({}, {})",
                self.expression(at(0)?)?,
                self.expression(at(1)?)?
            ),
            "concat" => binary(self, "\\o")?,
            "head" => format!("Head({})", self.expression(at(0)?)?),
            "tail" => format!("Tail({})", self.expression(at(0)?)?),
            "Rec" => {
                let mut fields = Vec::new();
                for pair in arguments.chunks(2) {
                    let [name, value] = pair else {
                        return Err(Error("Rec needs name/value pairs".into()));
                    };
                    fields.push(format!(
                        "{} |-> {}",
                        text(&name["value"])?,
                        self.expression(value)?
                    ));
                }
                format!("[{}]", fields.join(", "))
            }
            "field" => format!("{}.{}", self.expression(at(0)?)?, text(&at(1)?["value"])?),
            "with" => format!(
                "[{} EXCEPT !.{} = {}]",
                self.expression(at(0)?)?,
                text(&at(1)?["value"])?,
                self.expression(at(2)?)?
            ),
            "get" => {
                let map = self.expression(at(0)?)?;
                format!("{map}[{}]", self.expression(at(1)?)?)
            }
            "keys" => format!("(DOMAIN {})", self.expression(at(0)?)?),
            "set" => format!(
                "[{} EXCEPT ![{}] = {}]",
                self.expression(at(0)?)?,
                self.expression(at(1)?)?,
                self.expression(at(2)?)?
            ),
            "put" => {
                let map = self.expression(at(0)?)?;
                let key = self.expression(at(1)?)?;
                let value = self.expression(at(2)?)?;
                self.fresh += 1;
                let bound = format!("q_key_{}", self.fresh);
                format!(
                    "[{bound} \\in (DOMAIN {map}) \\cup {{{key}}} |-> IF {bound} = {key} THEN {value} ELSE {map}[{bound}]]"
                )
            }
            "mapBy" => {
                let (bound, set, body) = self.binder(at(0)?, at(1)?)?;
                format!("[{bound} \\in {set} |-> {body}]")
            }
            "variant" => format!(
                "[tag |-> {:?}, value |-> {}]",
                text(&at(0)?["value"])?,
                self.expression(at(1)?)?
            ),
            "matchVariant" => {
                let subject = self.expression(at(0)?)?;
                let mut cases = Vec::new();
                for pair in arguments[1..].chunks(2) {
                    let [label, handler] = pair else {
                        return Err(Error("matchVariant needs label/handler pairs".into()));
                    };
                    let label = text(&label["value"])?;
                    let value = format!("{subject}.value");
                    let body = self.apply(handler, &[value])?;
                    cases.push(if label == "_" {
                        format!("OTHER -> {body}")
                    } else {
                        format!("{subject}.tag = {label:?} -> {body}")
                    });
                }
                format!("(CASE {})", cases.join(" [] "))
            }
            other => return unsupported(format!("operator {other}")),
        })
    }
    fn all(&mut self, arguments: &[Value]) -> Result<Vec<String>> {
        arguments
            .iter()
            .map(|argument| self.expression(argument))
            .collect()
    }
    /// `fold` (sets, elements in CHOOSE order) and `foldl` (lists, in order)
    /// as a recursive operator local to the expression.
    fn fold(
        &mut self,
        set: bool,
        collection: &Value,
        initial: &Value,
        lambda: &Value,
    ) -> Result<String> {
        let collection = self.expression(collection)?;
        let initial = self.expression(initial)?;
        self.fresh += 1;
        let id = self.fresh;
        let (operator, accumulator, rest, element) = (
            format!("q_fold_{id}"),
            format!("q_acc_{id}"),
            format!("q_rest_{id}"),
            format!("q_elem_{id}"),
        );
        let step = self.apply(lambda, &[accumulator.clone(), element.clone()])?;
        let (empty, pick, remaining) = if set {
            (
                "{}".to_owned(),
                format!("CHOOSE q_pick_{id} \\in {rest} : TRUE"),
                format!("{rest} \\ {{{element}}}"),
            )
        } else {
            (
                "<<>>".to_owned(),
                format!("Head({rest})"),
                format!("Tail({rest})"),
            )
        };
        Ok(format!(
            "(LET RECURSIVE {operator}(_, _) {operator}({accumulator}, {rest}) == IF {rest} = {empty} THEN {accumulator} ELSE LET {element} == {pick} IN {operator}({step}, {remaining}) IN {operator}({initial}, {collection}))"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn module(declarations: Value) -> String {
        json!({"main": "M", "errors": [], "modules": [{"name": "M", "declarations": declarations}]})
            .to_string()
    }
    fn var(name: &str) -> Value {
        json!({"kind": "var", "name": name})
    }
    fn def(qualifier: &str, name: &str, expr: Value) -> Value {
        json!({"kind": "def", "qualifier": qualifier, "name": name, "expr": expr})
    }
    fn app(opcode: &str, args: Vec<Value>) -> Value {
        json!({"kind": "app", "opcode": opcode, "args": args})
    }
    fn name(name: &str) -> Value {
        json!({"kind": "name", "name": name})
    }
    fn int(value: i64) -> Value {
        json!({"kind": "int", "value": value})
    }
    fn string(value: &str) -> Value {
        json!({"kind": "str", "value": value})
    }
    fn lambda(params: &[&str], expr: Value) -> Value {
        let params: Vec<Value> = params.iter().map(|name| json!({"name": name})).collect();
        json!({"kind": "lambda", "params": params, "qualifier": "def", "expr": expr})
    }
    fn base(extra: Vec<Value>) -> Vec<Value> {
        let mut declarations = vec![
            var("x"),
            def("action", "init", app("assign", vec![name("x"), int(0)])),
            def("action", "step", app("assign", vec![name("x"), name("x")])),
        ];
        declarations.extend(extra);
        declarations
    }
    /// The TLA+ definition emitted for `val v = expr`.
    fn value(expr: Value) -> String {
        let translation = translate(
            &module(json!(base(vec![def("val", "v", expr)]))),
            &Options::default(),
        )
        .unwrap();
        translation
            .source
            .lines()
            .find_map(|line| line.strip_prefix("v == "))
            .unwrap()
            .to_owned()
    }

    #[test]
    fn module_and_config() {
        let translation = translate(
            &module(json!(base(vec![def(
                "val",
                "inv",
                app("igte", vec![name("x"), int(0)])
            )]))),
            &Options {
                invariants: vec!["inv".into()],
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(
            translation.source,
            "---- MODULE M ----\nEXTENDS Integers, Sequences, FiniteSets, TLC\nVARIABLES x\n\
             init == (x = 0)\ninv == (x >= 0)\nstep == (x' = x)\n====\n"
        );
        assert_eq!(translation.config, "INIT init\nNEXT step\nINVARIANT inv\n");
    }

    #[test]
    fn list_indices_are_shifted_to_one() {
        assert_eq!(
            value(app("nth", vec![app("List", vec![int(5), int(6)]), int(0)])),
            "<<5, 6>>[(0) + 1]"
        );
    }

    #[test]
    fn negative_literals_are_parenthesized() {
        assert_eq!(value(app("iadd", vec![int(1), int(-2)])), "(1 + (-2))");
    }

    #[test]
    fn folds_start_from_the_initial_value() {
        let fold = value(app(
            "foldl",
            vec![
                app("List", vec![int(1)]),
                int(7),
                lambda(&["a", "e"], app("iadd", vec![name("a"), name("e")])),
            ],
        ));
        assert!(fold.contains("IN q_fold_1(7, <<1>>))"), "{fold}");
        assert!(fold.contains("THEN q_acc_1 ELSE"), "{fold}");
        assert!(
            fold.contains("Head(q_rest_1)") && fold.contains("Tail(q_rest_1)"),
            "{fold}"
        );
        let fold = value(app(
            "fold",
            vec![
                app("Set", vec![int(1)]),
                int(0),
                lambda(&["a", "e"], app("iadd", vec![name("a"), name("e")])),
            ],
        ));
        assert!(
            fold.contains("CHOOSE q_pick_1 \\in q_rest_1 : TRUE"),
            "{fold}"
        );
        assert!(fold.contains("q_rest_1 \\ {q_elem_1}"), "{fold}");
    }

    #[test]
    fn match_compares_tags() {
        let matched = value(app(
            "matchVariant",
            vec![
                app("variant", vec![string("A"), app("Tup", vec![])]),
                string("A"),
                lambda(&["_"], int(1)),
                string("_"),
                lambda(&["_"], int(2)),
            ],
        ));
        assert_eq!(
            matched,
            "(CASE [tag |-> \"A\", value |-> <<>>].tag = \"A\" -> (LET q__1 == [tag |-> \"A\", value |-> <<>>].value IN 1) [] OTHER -> (LET q__2 == [tag |-> \"A\", value |-> <<>>].value IN 2))"
        );
    }

    #[test]
    fn nondet_is_an_existential() {
        let step = json!({"kind": "let", "opdef": def("nondet", "s", app("oneOf", vec![app("Set", vec![int(1), int(2)])])), "expr": app("assign", vec![name("x"), name("s")])});
        let translation = translate(
            &module(json!(vec![
                var("x"),
                def("action", "init", app("assign", vec![name("x"), int(0)])),
                def("action", "step", step),
            ])),
            &Options::default(),
        )
        .unwrap();
        assert!(
            translation
                .source
                .contains("step == (\\E q_s_1 \\in {1, 2}: (x' = q_s_1))"),
            "{}",
            translation.source
        );
    }

    #[test]
    fn maps_and_records() {
        assert_eq!(value(app("get", vec![name("x"), int(1)])), "x[1]");
        assert_eq!(
            value(app("put", vec![name("x"), int(1), int(2)])),
            "[q_key_1 \\in (DOMAIN x) \\cup {1} |-> IF q_key_1 = 1 THEN 2 ELSE x[q_key_1]]"
        );
        assert_eq!(
            value(app("with", vec![name("x"), string("a"), int(3)])),
            "[x EXCEPT !.a = 3]"
        );
        assert_eq!(value(app("Rec", vec![string("a"), int(1)])), "[a |-> 1]");
    }

    #[test]
    fn unsupported_constructs_are_rejected() {
        let rejected = |declarations: Vec<Value>| {
            translate(&module(json!(declarations)), &Options::default()).unwrap_err()
        };
        assert!(
            rejected(base(vec![json!({"kind": "const", "name": "N"})]))
                .0
                .contains("const")
        );
        assert!(
            rejected(base(vec![def("temporal", "live", name("x"))]))
                .0
                .contains("temporal")
        );
        assert!(
            rejected(base(vec![def("val", "v", app("always", vec![name("x")]))]))
                .0
                .contains("operator always")
        );
        assert!(
            rejected(base(vec![def("val", "v", name("undefined"))]))
                .0
                .contains("unresolved name")
        );
        let missing = translate(
            &module(json!(base(vec![]))),
            &Options {
                invariants: vec!["absent".into()],
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(missing.0.contains("absent"));
        let errors =
            json!({"main": "M", "errors": [{"code": "QNT404"}], "modules": []}).to_string();
        assert!(translate(&errors, &Options::default()).is_err());
    }
}
