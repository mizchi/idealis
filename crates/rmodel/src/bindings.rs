//! Bindings generated from a model for its implementations (plan 0102):
//! JSON Schema of the configuration, the state and the commands, a Rust
//! trait with its types, and TypeScript types with a driver.
//!
//! The JSON form is the contract every language shares:
//!
//! - an atom of a signature is its name with its index (`"User0"`), an enum
//!   member its name (`"JPY"`), a Bool a JSON boolean, an Int a JSON
//!   integer, and a record an object of its fields;
//! - a unary relation is an array of elements (`set`, `some`), an element
//!   or `null` (`lone`), or an element (`one`);
//! - a binary relation is an object with a key for every atom of its
//!   domain, each holding the value a unary relation of the same
//!   multiplicity would;
//! - the configuration and the state are objects of their relations, and a
//!   command is `{"kind": action, param: element, ...}`.
use crate::syntax::{BOOL, INT, Model, Mult, Record, Shape};

/// What a sort is, for the bindings.
enum Kind<'a> {
    Sig(usize),
    Enum(&'a [String]),
    Bool,
    Int,
    Sum(&'a [String]),
    Record(&'a Record),
}

fn kind<'a>(model: &'a Model, sort: &str) -> Kind<'a> {
    if sort == INT {
        return Kind::Int;
    }
    if sort == BOOL {
        return Kind::Bool;
    }
    if let Some(record) = model.record(sort) {
        return Kind::Record(record);
    }
    if let Some((_, members)) = model.enums.iter().find(|(name, _)| name == sort) {
        return Kind::Enum(members);
    }
    if let Some((_, children)) = model.sums.iter().find(|(name, _)| name == sort) {
        return Kind::Sum(children);
    }
    Kind::Sig(model.scope.sizes[sort])
}

/// The names of a sort's atoms, as JSON keys.
pub fn keys(model: &Model, sort: &str) -> Vec<String> {
    match kind(model, sort) {
        Kind::Sig(size) => (0..size).map(|index| format!("{sort}{index}")).collect(),
        Kind::Enum(members) => members.to_vec(),
        Kind::Bool => vec!["false".into(), "true".into()],
        Kind::Sum(children) => children
            .iter()
            .flat_map(|child| keys(model, child))
            .collect(),
        Kind::Int | Kind::Record(_) => unreachable!("values are not keys"),
    }
}

fn quote(text: &str) -> String {
    format!("\"{text}\"")
}

fn target(shape: &Shape) -> (&str, Mult) {
    match shape {
        Shape::Unary { sig, mult } => (sig, *mult),
        Shape::Binary { to, mult, .. } => (to, *mult),
    }
}

/// JSON Schema (draft 2020-12) of `Config`, `State` and `Command`.
pub fn json_schema(model: &Model) -> String {
    let element = |sort: &str| format!("{{\"$ref\":\"#/$defs/{sort}\"}}");
    let held = |sort: &str, mult: Mult| match mult {
        Mult::Set => format!(
            "{{\"type\":\"array\",\"items\":{},\"uniqueItems\":true}}",
            element(sort)
        ),
        Mult::Some => format!(
            "{{\"type\":\"array\",\"items\":{},\"uniqueItems\":true,\"minItems\":1}}",
            element(sort)
        ),
        Mult::Lone => format!("{{\"anyOf\":[{},{{\"type\":\"null\"}}]}}", element(sort)),
        Mult::One => element(sort),
    };
    let object = |properties: Vec<(String, String)>| {
        let names: Vec<String> = properties.iter().map(|(name, _)| quote(name)).collect();
        let fields: Vec<String> = properties
            .iter()
            .map(|(name, schema)| format!("{}:{schema}", quote(name)))
            .collect();
        format!(
            "{{\"type\":\"object\",\"properties\":{{{}}},\"required\":[{}],\"additionalProperties\":false}}",
            fields.join(","),
            names.join(",")
        )
    };
    let relation = |shape: &Shape| match shape {
        Shape::Unary { sig, mult } => held(sig, *mult),
        Shape::Binary { from, to, mult } => object(
            keys(model, from)
                .into_iter()
                .map(|key| (key, held(to, *mult)))
                .collect(),
        ),
    };
    let mut defs: Vec<(String, String)> = Vec::new();
    let mut sorts: Vec<&str> = model.sigs.iter().map(String::as_str).collect();
    sorts.extend(model.enums.iter().map(|(name, _)| name.as_str()));
    sorts.extend(model.sums.iter().map(|(name, _)| name.as_str()));
    sorts.extend(model.records.iter().map(|record| record.name.as_str()));
    if model.scope.ints.is_some() {
        sorts.push(INT);
    }
    for sort in sorts {
        let schema = match kind(model, sort) {
            Kind::Int => "{\"type\":\"integer\"}".to_owned(),
            Kind::Bool => "{\"type\":\"boolean\"}".to_owned(),
            Kind::Sig(_) | Kind::Enum(_) => {
                let names: Vec<String> = keys(model, sort).iter().map(|key| quote(key)).collect();
                format!("{{\"enum\":[{}]}}", names.join(","))
            }
            Kind::Sum(children) => {
                let options: Vec<String> = children.iter().map(|child| element(child)).collect();
                format!("{{\"anyOf\":[{}]}}", options.join(","))
            }
            Kind::Record(record) => object(
                record
                    .fields
                    .iter()
                    .map(|(field, sort)| (field.clone(), element(sort)))
                    .collect(),
            ),
        };
        defs.push((sort.to_owned(), schema));
    }
    let part = |var: bool| {
        object(
            model
                .relations
                .iter()
                .filter(|relation| relation.var == var)
                .map(|r| (r.name.clone(), relation(&r.shape)))
                .collect(),
        )
    };
    defs.push(("Config".into(), part(false)));
    defs.push(("State".into(), part(true)));
    let commands: Vec<String> = model
        .actions
        .iter()
        .map(|action| {
            let mut properties = vec![(
                "kind".to_owned(),
                format!("{{\"const\":{}}}", quote(&action.name)),
            )];
            properties.extend(
                action
                    .params
                    .iter()
                    .map(|(param, sort)| (param.clone(), element(sort))),
            );
            object(properties)
        })
        .collect();
    defs.push((
        "Command".into(),
        format!("{{\"oneOf\":[{}]}}", commands.join(",")),
    ));
    let defs: Vec<String> = defs
        .iter()
        .map(|(name, schema)| format!("{}:{schema}", quote(name)))
        .collect();
    format!(
        "{{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"title\":{},\"$defs\":{{{}}}}}",
        quote(&model.name),
        defs.join(",")
    )
}

fn indent(text: &str, width: usize) -> String {
    text.lines()
        .map(|line| format!("{}{line}", " ".repeat(width)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The Rust type of an element of a sort.
fn rust_type(sort: &str) -> String {
    match sort {
        INT => "i64".into(),
        BOOL => "bool".into(),
        other => other.into(),
    }
}

fn rust_held(sort: &str, mult: Mult) -> String {
    let element = rust_type(sort);
    match mult {
        Mult::Set | Mult::Some => format!("BTreeSet<{element}>"),
        Mult::Lone => format!("Option<{element}>"),
        Mult::One => element,
    }
}

fn rust_relation(shape: &Shape) -> String {
    match shape {
        Shape::Unary { sig, mult } => rust_held(sig, *mult),
        // `lone`: a missing key holds no value.
        Shape::Binary { from, to, mult } => {
            let value = match mult {
                Mult::Lone | Mult::One => rust_type(to),
                Mult::Set | Mult::Some => format!("BTreeSet<{}>", rust_type(to)),
            };
            format!("BTreeMap<{}, {value}>", rust_type(from))
        }
    }
}

/// A Rust module: the sorts as types, `Config`, `State`, `Command`, the
/// model's trait with one method per action, JSON conversions, and a
/// conformance check against the reference semantics through rcheck.
/// `source` is the model's text, embedded for the conformance check.
pub fn rust(model: &Model, source: &str) -> String {
    let mut out = format!(
        "//! Generated by rmodel from the model {} (plan 0102). Do not edit.\n\
         #![allow(dead_code, unused_imports, unused_mut, unused_variables, non_snake_case, non_camel_case_types, clippy::all)]\n\
         use serde_json::{{Map, Value}};\n\
         use std::collections::{{BTreeMap, BTreeSet}};\n\n",
        model.name
    );
    out.push_str(&format!(
        "pub const MODEL: &str = r####\"{}\"####;\n\n",
        source.trim_end()
    ));
    let derive = "#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]";
    out.push_str(
        "fn to_Int(x: &i64) -> Value {\n    Value::from(*x)\n}\n\
         fn from_Int(v: &Value) -> Result<i64, String> {\n    v.as_i64().ok_or_else(|| format!(\"not an integer: {v}\"))\n}\n\
         fn to_Bool(x: &bool) -> Value {\n    Value::Bool(*x)\n}\n\
         fn from_Bool(v: &Value) -> Result<bool, String> {\n    v.as_bool().ok_or_else(|| format!(\"not a boolean: {v}\"))\n}\n\
         fn key_Bool(k: &str) -> Result<bool, String> {\n    match k {\n        \"true\" => Ok(true),\n        \"false\" => Ok(false),\n        _ => Err(format!(\"not a Bool key: {k}\")),\n    }\n}\n\n",
    );
    for sig in &model.sigs {
        let size = model.scope.sizes[sig];
        let all: Vec<String> = (0..size).map(|index| format!("{sig}({index})")).collect();
        out.push_str(&format!(
            "{derive}\npub struct {sig}(pub u8);\n\
             impl {sig} {{\n    pub const ALL: [{sig}; {size}] = [{}];\n}}\n\
             fn to_{sig}(x: &{sig}) -> Value {{\n    Value::String(format!(\"{sig}{{}}\", x.0))\n}}\n\
             fn from_{sig}(v: &Value) -> Result<{sig}, String> {{\n    let s = v.as_str().ok_or_else(|| format!(\"{sig}: not a string: {{v}}\"))?;\n    key_{sig}(s)\n}}\n\
             fn key_{sig}(s: &str) -> Result<{sig}, String> {{\n    s.strip_prefix(\"{sig}\")\n        .and_then(|n| n.parse::<u8>().ok())\n        .filter(|n| (*n as usize) < {size})\n        .map({sig})\n        .ok_or_else(|| format!(\"not a {sig}: {{s}}\"))\n}}\n\n",
            all.join(", ")
        ));
    }
    for (name, members) in &model.enums {
        if name == BOOL {
            continue;
        }
        let variants: Vec<String> = members.iter().map(|m| format!("{name}::{m}")).collect();
        let to: Vec<String> = members
            .iter()
            .map(|m| format!("        {name}::{m} => \"{m}\","))
            .collect();
        let from: Vec<String> = members
            .iter()
            .map(|m| format!("        \"{m}\" => Ok({name}::{m}),"))
            .collect();
        out.push_str(&format!(
            "{derive}\npub enum {name} {{\n    {}\n}}\n\
             impl {name} {{\n    pub const ALL: [{name}; {}] = [{}];\n}}\n\
             fn to_{name}(x: &{name}) -> Value {{\n    Value::String(\n        match x {{\n{}\n        }}\n        .into(),\n    )\n}}\n\
             fn from_{name}(v: &Value) -> Result<{name}, String> {{\n    let s = v.as_str().ok_or_else(|| format!(\"{name}: not a string: {{v}}\"))?;\n    key_{name}(s)\n}}\n\
             fn key_{name}(s: &str) -> Result<{name}, String> {{\n    match s {{\n{}\n        _ => Err(format!(\"not a {name}: {{s}}\")),\n    }}\n}}\n\n",
            members.join(",\n    "),
            members.len(),
            variants.join(", "),
            to.join("\n"),
            from.join("\n")
        ));
    }
    for (name, children) in &model.sums {
        let variants: Vec<String> = children.iter().map(|c| format!("{c}({c})")).collect();
        let to: Vec<String> = children
            .iter()
            .map(|c| format!("        {name}::{c}(x) => to_{c}(x),"))
            .collect();
        let from: Vec<String> = children
            .iter()
            .map(|c| {
                format!(
                    "    if let Ok(x) = key_{c}(s) {{\n        return Ok({name}::{c}(x));\n    }}"
                )
            })
            .collect();
        let all: Vec<String> = children
            .iter()
            .map(|c| format!("{c}::ALL.iter().map(|x| {name}::{c}(*x))"))
            .collect();
        out.push_str(&format!(
            "{derive}\npub enum {name} {{\n    {}\n}}\n\
             impl {name} {{\n    pub fn all() -> Vec<{name}> {{\n        std::iter::empty(){}.collect()\n    }}\n}}\n\
             fn to_{name}(x: &{name}) -> Value {{\n    match x {{\n{}\n    }}\n}}\n\
             fn from_{name}(v: &Value) -> Result<{name}, String> {{\n    let s = v.as_str().ok_or_else(|| format!(\"{name}: not a string: {{v}}\"))?;\n    key_{name}(s)\n}}\n\
             fn key_{name}(s: &str) -> Result<{name}, String> {{\n{}\n    Err(format!(\"not a {name}: {{s}}\"))\n}}\n\n",
            variants.join(",\n    "),
            all.iter().map(|a| format!(".chain({a})")).collect::<String>(),
            to.join("\n"),
            from.join("\n")
        ));
    }
    for record in &model.records {
        let fields: Vec<String> = record
            .fields
            .iter()
            .map(|(field, sort)| format!("    pub {field}: {},", rust_type(sort)))
            .collect();
        let to: Vec<String> = record
            .fields
            .iter()
            .map(|(field, sort)| {
                format!("    m.insert(\"{field}\".into(), to_{sort}(&x.{field}));")
            })
            .collect();
        let from: Vec<String> = record
            .fields
            .iter()
            .map(|(field, sort)| {
                format!(
                    "        {field}: from_{sort}(o.get(\"{field}\").ok_or(\"{}: missing {field}\")?)?,",
                    record.name
                )
            })
            .collect();
        out.push_str(&format!(
            "{derive}\npub struct {name} {{\n{}\n}}\n\
             fn to_{name}(x: &{name}) -> Value {{\n    let mut m = Map::new();\n{}\n    Value::Object(m)\n}}\n\
             fn from_{name}(v: &Value) -> Result<{name}, String> {{\n    let o = v.as_object().ok_or_else(|| format!(\"{name}: not an object: {{v}}\"))?;\n    if o.len() != {} {{\n        return Err(format!(\"{name}: unexpected fields: {{v}}\"));\n    }}\n    Ok({name} {{\n{}\n    }})\n}}\n\n",
            fields.join("\n"),
            to.join("\n"),
            record.fields.len(),
            from.join("\n"),
            name = record.name
        ));
    }
    // Every atom of a sort, for the keys of binary relations.
    let all = |sort: &str| -> String {
        match kind(model, sort) {
            Kind::Bool => "[false, true].to_vec()".into(),
            Kind::Sum(_) => format!("{sort}::all()"),
            _ => format!("{sort}::ALL.to_vec()"),
        }
    };
    let key_fn = |sort: &str| format!("key_{sort}");
    for var in [false, true] {
        let name = if var { "State" } else { "Config" };
        let relations: Vec<_> = model.relations.iter().filter(|r| r.var == var).collect();
        let fields: Vec<String> = relations
            .iter()
            .map(|r| format!("    pub {}: {},", r.name, rust_relation(&r.shape)))
            .collect();
        let mut to = Vec::new();
        let mut from = Vec::new();
        for relation in &relations {
            let field = &relation.name;
            let (sort, mult) = target(&relation.shape);
            let held_to = |value: &str| match mult {
                Mult::Set | Mult::Some => {
                    format!("Value::Array({value}.iter().map(to_{sort}).collect())")
                }
                Mult::Lone => format!("{value}.as_ref().map_or(Value::Null, to_{sort})"),
                Mult::One => format!("to_{sort}(&{value})"),
            };
            let held_from = |value: &str| match mult {
                Mult::Set | Mult::Some => format!(
                    "{value}.as_array().ok_or(\"{field}: not an array\")?.iter().map(from_{sort}).collect::<Result<BTreeSet<_>, _>>()?"
                ),
                Mult::Lone => format!(
                    "if {value}.is_null() {{ None }} else {{ Some(from_{sort}({value})?) }}"
                ),
                Mult::One => format!("from_{sort}({value})?"),
            };
            match &relation.shape {
                Shape::Unary { .. } => {
                    to.push(format!(
                        "    m.insert(\"{field}\".into(), {});",
                        held_to(&format!("self.{field}"))
                    ));
                    from.push(format!(
                        "        {field}: {{\n            let v = o.get(\"{field}\").ok_or(\"missing {field}\")?;\n            {}\n        }},",
                        held_from("v")
                    ));
                }
                Shape::Binary { from: domain, .. } => {
                    let empty = match mult {
                        Mult::Set | Mult::Some => "Value::Array(Vec::new())".to_owned(),
                        _ => "Value::Null".to_owned(),
                    };
                    let present = match mult {
                        Mult::Set | Mult::Some => "Value::Array(x.iter().map(to_TARGET).collect())"
                            .replace("TARGET", sort),
                        _ => format!("to_{sort}(x)"),
                    };
                    to.push(format!(
                        "    {{\n        let mut r = Map::new();\n        for k in {} {{\n            let key = match to_{domain}(&k) {{\n                Value::String(s) => s,\n                other => other.to_string(),\n            }};\n            r.insert(key, self.{field}.get(&k).map_or({empty}, |x| {present}));\n        }}\n        m.insert(\"{field}\".into(), Value::Object(r));\n    }}",
                        all(domain)
                    ));
                    let insert = match mult {
                        Mult::Set | Mult::Some => format!(
                            "r.insert(k, v.as_array().ok_or(\"{field}: not an array\")?.iter().map(from_{sort}).collect::<Result<BTreeSet<_>, _>>()?);"
                        ),
                        Mult::Lone => format!(
                            "if !v.is_null() {{\n                    r.insert(k, from_{sort}(v)?);\n                }}"
                        ),
                        Mult::One => format!("r.insert(k, from_{sort}(v)?);"),
                    };
                    from.push(format!(
                        "        {field}: {{\n            let mut r = BTreeMap::new();\n            let o = o.get(\"{field}\").and_then(Value::as_object).ok_or(\"{field}: not an object\")?;\n            for (key, v) in o {{\n                let k = {}(key)?;\n                {insert}\n            }}\n            r\n        }},",
                        key_fn(domain)
                    ));
                }
            }
        }
        out.push_str(&format!(
            "#[derive(Clone, Debug, PartialEq, Eq)]\npub struct {name} {{\n{}\n}}\n\
             impl {name} {{\n    pub fn to_json(&self) -> Value {{\n        let mut m = Map::new();\n{}\n        Value::Object(m)\n    }}\n\
             pub fn from_json(v: &Value) -> Result<Self, String> {{\n        let o = v.as_object().ok_or(\"{name}: not an object\")?;\n        Ok(Self {{\n{}\n        }})\n    }}\n}}\n\n",
            fields.join("\n"),
            to.iter()
                .map(|block| indent(block, 4))
                .collect::<Vec<_>>()
                .join("\n"),
            from.join("\n")
        ));
    }
    // Commands and the trait.
    let variants: Vec<String> = model
        .actions
        .iter()
        .map(|action| {
            let params: Vec<String> = action
                .params
                .iter()
                .map(|(param, sort)| format!("{param}: {}", rust_type(sort)))
                .collect();
            format!("    {} {{ {} }},", action.name, params.join(", "))
        })
        .collect();
    let from: Vec<String> = model
        .actions
        .iter()
        .map(|action| {
            let params: Vec<String> = action
                .params
                .iter()
                .map(|(param, sort)| {
                    format!(
                        "{param}: from_{sort}(o.get(\"{param}\").ok_or(\"{}: missing {param}\")?)?",
                        action.name
                    )
                })
                .collect();
            format!(
                "            \"{0}\" => Command::{0} {{ {1} }},",
                action.name,
                params.join(", ")
            )
        })
        .collect();
    out.push_str(&format!(
        "#[derive(Clone, Debug, PartialEq, Eq)]\npub enum Command {{\n{}\n}}\n\
         impl Command {{\n    pub fn from_json(v: &Value) -> Result<Self, String> {{\n        let o = v.as_object().ok_or(\"Command: not an object\")?;\n        let kind = o.get(\"kind\").and_then(Value::as_str).ok_or(\"Command: no kind\")?;\n        Ok(match kind {{\n{}\n            other => return Err(format!(\"no action {{other}}\")),\n        }})\n    }}\n}}\n\n",
        variants.join("\n"),
        from.join("\n")
    ));
    let methods: Vec<String> = model
        .actions
        .iter()
        .map(|action| {
            let params: Vec<String> = action
                .params
                .iter()
                .map(|(param, sort)| format!(", {param}: {}", rust_type(sort)))
                .collect();
            format!(
                "    /// Applies `{0}` when its guards hold, and says whether it did.\n    fn {0}(&mut self{1}) -> bool;",
                action.name,
                params.concat()
            )
        })
        .collect();
    let calls: Vec<String> = model
        .actions
        .iter()
        .map(|action| {
            let params: Vec<&str> = action.params.iter().map(|(p, _)| p.as_str()).collect();
            format!(
                "            Command::{0} {{ {1} }} => domain.{0}({1}),",
                action.name,
                params.join(", ")
            )
        })
        .collect();
    out.push_str(&format!(
        "/// The model {0}: an implementation keeps the state and applies the\n\
         /// actions; `conformance` checks it against the model.\n\
         pub trait {0}: Sized {{\n    fn new(config: &Config) -> Self;\n    fn state(&self) -> State;\n{1}\n}}\n\n\
         /// Drives an implementation for rcheck's conformance check.\n\
         pub struct Driver<I>(pub Option<I>);\n\
         impl<I: {0}> rcheck::conform::Driver for Driver<I> {{\n    fn reset(&mut self, config: &Value) -> Result<Value, String> {{\n        let domain = I::new(&Config::from_json(config)?);\n        let state = domain.state().to_json();\n        self.0 = Some(domain);\n        Ok(state)\n    }}\n    fn apply(&mut self, command: &Value) -> Result<(bool, Value), String> {{\n        let domain = self.0.as_mut().ok_or(\"apply before reset\")?;\n        let accepted = match Command::from_json(command)? {{\n{2}\n        }};\n        Ok((accepted, domain.state().to_json()))\n    }}\n}}\n\n\
         /// Checks an implementation against the model's reference semantics.\n\
         pub fn conformance<I: {0}>(budget: rcheck::conform::Budget) -> rcheck::conform::Report {{\n    let model = rmodel::parse(MODEL).expect(\"the generated model parses\");\n    rcheck::conform::conform(&model, &mut Driver::<I>(None), budget)\n}}\n",
        model.name,
        methods.join("\n"),
        calls.join("\n")
    ));
    out
}

/// The TypeScript type of an element of a sort.
fn ts_type(sort: &str) -> String {
    match sort {
        INT => "number".into(),
        BOOL => "boolean".into(),
        other => other.into(),
    }
}

fn ts_relation(shape: &Shape) -> String {
    let held = |sort: &str, mult: Mult| match mult {
        Mult::Set | Mult::Some => format!("ReadonlySet<{}>", ts_type(sort)),
        Mult::Lone => format!("{} | undefined", ts_type(sort)),
        Mult::One => ts_type(sort),
    };
    match shape {
        Shape::Unary { sig, mult } => held(sig, *mult),
        Shape::Binary { from, to, mult } => {
            let value = match mult {
                Mult::Lone | Mult::One => ts_type(to),
                _ => format!("ReadonlySet<{}>", ts_type(to)),
            };
            format!("ReadonlyMap<{}, {value}>", ts_type(from))
        }
    }
}

/// A TypeScript module: the sorts as types, `Config`, `State`, `Command`,
/// the `Domain` an implementation provides, JSON conversions, and `serve`,
/// which speaks rcheck's JSON Lines driver protocol on stdin and stdout.
pub fn typescript(model: &Model) -> String {
    let mut out = format!(
        "// Generated by rmodel from the model {} (plan 0102). Do not edit.\nimport {{ createInterface }} from \"node:readline\";\n\n",
        model.name
    );
    let mut converters = String::new();
    for sig in &model.sigs {
        let names: Vec<String> = keys(model, sig).iter().map(|k| quote(k)).collect();
        out.push_str(&format!(
            "export type {sig} = string & {{ readonly __sort: \"{sig}\" }};\nexport const {sig}s: readonly {sig}[] = [{}] as readonly string[] as readonly {sig}[];\n",
            names.join(", ")
        ));
        converters.push_str(&format!(
            "function from_{sig}(v: unknown): {sig} {{\n  if (typeof v !== \"string\" || !({sig}s as readonly string[]).includes(v)) throw new Error(`not a {sig}: ${{JSON.stringify(v)}}`);\n  return v as {sig};\n}}\nconst to_{sig} = (x: {sig}): unknown => x;\n"
        ));
    }
    for (name, members) in &model.enums {
        if name == BOOL {
            continue;
        }
        let names: Vec<String> = members.iter().map(|m| quote(m)).collect();
        out.push_str(&format!(
            "export type {name} = {};\nexport const {name}s: readonly {name}[] = [{}];\n",
            names.join(" | "),
            names.join(", ")
        ));
        converters.push_str(&format!(
            "function from_{name}(v: unknown): {name} {{\n  if (!({name}s as readonly unknown[]).includes(v)) throw new Error(`not a {name}: ${{JSON.stringify(v)}}`);\n  return v as {name};\n}}\nconst to_{name} = (x: {name}): unknown => x;\n"
        ));
    }
    converters.push_str(
        "function from_Bool(v: unknown): boolean {\n  if (typeof v !== \"boolean\") throw new Error(`not a boolean: ${JSON.stringify(v)}`);\n  return v;\n}\nconst to_Bool = (x: boolean): unknown => x;\nconst key_Bool = (k: string): boolean => {\n  if (k !== \"true\" && k !== \"false\") throw new Error(`not a Bool key: ${k}`);\n  return k === \"true\";\n};\n\
         function from_Int(v: unknown): number {\n  if (typeof v !== \"number\" || !Number.isInteger(v)) throw new Error(`not an integer: ${JSON.stringify(v)}`);\n  return v;\n}\nconst to_Int = (x: number): unknown => x;\n",
    );
    for (name, children) in &model.sums {
        out.push_str(&format!(
            "export type {name} = {};\nexport const {name}s: readonly {name}[] = [{}];\n",
            children.join(" | "),
            children
                .iter()
                .map(|c| format!("...{c}s"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        converters.push_str(&format!(
            "function from_{name}(v: unknown): {name} {{\n  if (!({name}s as readonly unknown[]).includes(v)) throw new Error(`not a {name}: ${{JSON.stringify(v)}}`);\n  return v as {name};\n}}\nconst to_{name} = (x: {name}): unknown => x;\n"
        ));
    }
    for record in &model.records {
        let fields: Vec<String> = record
            .fields
            .iter()
            .map(|(field, sort)| format!("readonly {field}: {}", ts_type(sort)))
            .collect();
        out.push_str(&format!(
            "export interface {} {{ {} }}\n",
            record.name,
            fields.join("; ")
        ));
        let from: Vec<String> = record
            .fields
            .iter()
            .map(|(field, sort)| format!("{field}: from_{sort}(o.{field})"))
            .collect();
        let to: Vec<String> = record
            .fields
            .iter()
            .map(|(field, sort)| format!("{field}: to_{sort}(x.{field})"))
            .collect();
        converters.push_str(&format!(
            "function from_{0}(v: unknown): {0} {{\n  if (typeof v !== \"object\" || v === null || Object.keys(v).length !== {1}) throw new Error(`not a {0}: ${{JSON.stringify(v)}}`);\n  const o = v as Record<string, unknown>;\n  return {{ {2} }};\n}}\nconst to_{0} = (x: {0}): unknown => ({{ {3} }});\n",
            record.name,
            record.fields.len(),
            from.join(", "),
            to.join(", ")
        ));
    }
    let key = |sort: &str| -> String {
        match kind(model, sort) {
            Kind::Bool => "key_Bool".into(),
            _ => format!("from_{sort}"),
        }
    };
    let all = |sort: &str| -> String {
        match kind(model, sort) {
            Kind::Bool => "[false, true]".into(),
            _ => format!("{sort}s"),
        }
    };
    for var in [false, true] {
        let name = if var { "State" } else { "Config" };
        let relations: Vec<_> = model.relations.iter().filter(|r| r.var == var).collect();
        let fields: Vec<String> = relations
            .iter()
            .map(|r| format!("  readonly {}: {};", r.name, ts_relation(&r.shape)))
            .collect();
        out.push_str(&format!(
            "export interface {name} {{\n{}\n}}\n",
            fields.join("\n")
        ));
        let mut from = Vec::new();
        let mut to = Vec::new();
        for relation in &relations {
            let field = &relation.name;
            let (sort, mult) = target(&relation.shape);
            let held_from = |value: &str| match mult {
                Mult::Set | Mult::Some => {
                    format!("new Set((({value}) as unknown[]).map(from_{sort}))")
                }
                Mult::Lone => format!("({value}) === null ? undefined : from_{sort}({value})"),
                Mult::One => format!("from_{sort}({value})"),
            };
            match &relation.shape {
                Shape::Unary { .. } => {
                    from.push(format!(
                        "    {field}: {},",
                        held_from(&format!("o.{field}"))
                    ));
                    to.push(match mult {
                        Mult::Set | Mult::Some => {
                            format!("    {field}: [...x.{field}].map(to_{sort}),")
                        }
                        Mult::Lone => format!(
                            "    {field}: x.{field} === undefined ? null : to_{sort}(x.{field}),"
                        ),
                        Mult::One => format!("    {field}: to_{sort}(x.{field}),"),
                    });
                }
                Shape::Binary { from: domain, .. } => {
                    let insert = match mult {
                        Mult::Lone => {
                            format!("if (v !== null) m.set({}(k), from_{sort}(v));", key(domain))
                        }
                        _ => format!("m.set({}(k), {});", key(domain), held_from("v")),
                    };
                    from.push(format!(
                        "    {field}: (() => {{\n      const m = new Map();\n      for (const [k, v] of Object.entries(o.{field} as Record<string, unknown>)) {insert}\n      return m;\n    }})(),"
                    ));
                    let present = match mult {
                        Mult::Set | Mult::Some => format!("[...v].map(to_{sort})"),
                        _ => format!("to_{sort}(v)"),
                    };
                    let missing = match mult {
                        Mult::Set | Mult::Some => "[]",
                        _ => "null",
                    };
                    to.push(format!(
                        "    {field}: Object.fromEntries({}.map((k) => {{\n      const v = x.{field}.get(k as never);\n      return [String(k), v === undefined ? {missing} : {present}];\n    }})),",
                        all(domain)
                    ));
                }
            }
        }
        converters.push_str(&format!(
            "export function {lower}FromJson(v: unknown): {name} {{\n  const o = v as Record<string, unknown>;\n  return {{\n{}\n  }};\n}}\nexport function {lower}ToJson(x: {name}): unknown {{\n  return {{\n{}\n  }};\n}}\n",
            from.join("\n"),
            to.join("\n"),
            lower = name.to_lowercase()
        ));
    }
    let variants: Vec<String> = model
        .actions
        .iter()
        .map(|action| {
            let params: Vec<String> = action
                .params
                .iter()
                .map(|(param, sort)| format!("readonly {param}: {}", ts_type(sort)))
                .collect();
            let mut parts = vec![format!("readonly kind: \"{}\"", action.name)];
            parts.extend(params);
            format!("  | {{ {} }}", parts.join("; "))
        })
        .collect();
    out.push_str(&format!(
        "export type Command =\n{};\n",
        variants.join("\n")
    ));
    let cases: Vec<String> = model
        .actions
        .iter()
        .map(|action| {
            let params: Vec<String> = action
                .params
                .iter()
                .map(|(param, sort)| format!(", {param}: from_{sort}(o.{param})"))
                .collect();
            format!(
                "    case \"{0}\":\n      return {{ kind: \"{0}\"{1} }};",
                action.name,
                params.concat()
            )
        })
        .collect();
    converters.push_str(&format!(
        "export function commandFromJson(v: unknown): Command {{\n  const o = v as Record<string, unknown>;\n  switch (o.kind) {{\n{}\n    default:\n      throw new Error(`no action ${{String(o.kind)}}`);\n  }}\n}}\n",
        cases.join("\n")
    ));
    out.push_str(
        "/** An implementation of the model: it keeps the state and applies commands. */\n\
         export interface Domain {\n  readonly state: State;\n  /** Applies the command when its guards hold, and says whether it did. */\n  apply(command: Command): boolean;\n}\n\
         export type Create = (config: Config) => Domain;\n\n",
    );
    out.push_str(&converters);
    out.push_str(
        "\n/** Serves rcheck's JSON Lines driver protocol on stdin and stdout. */\n\
         export function serve(create: Create): void {\n  let domain: Domain | undefined;\n  const lines = createInterface({ input: process.stdin });\n  lines.on(\"line\", (line) => {\n    let reply: unknown;\n    try {\n      const message = JSON.parse(line) as { op: string; config?: unknown; command?: unknown };\n      if (message.op === \"reset\") {\n        domain = create(configFromJson(message.config));\n        reply = { state: stateToJson(domain.state) };\n      } else if (message.op === \"apply\" && domain !== undefined) {\n        const accepted = domain.apply(commandFromJson(message.command));\n        reply = { accepted, state: stateToJson(domain.state) };\n      } else {\n        reply = { error: `unexpected message: ${line}` };\n      }\n    } catch (error) {\n      reply = { error: String(error) };\n    }\n    process.stdout.write(`${JSON.stringify(reply)}\\n`);\n  });\n}\n",
    );
    out
}

fn capital(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or(String::new(), |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The Go type of an element of a sort.
fn go_type(sort: &str) -> String {
    match sort {
        INT => "int64".into(),
        BOOL => "bool".into(),
        other => other.into(),
    }
}

fn go_relation(shape: &Shape) -> String {
    let held = |sort: &str, mult: Mult| match mult {
        Mult::Set | Mult::Some => format!("Set[{}]", go_type(sort)),
        Mult::Lone => format!("*{}", go_type(sort)),
        Mult::One => go_type(sort),
    };
    match shape {
        Shape::Unary { sig, mult } => held(sig, *mult),
        Shape::Binary { from, to, mult } => {
            let value = match mult {
                Mult::Lone | Mult::One => go_type(to),
                _ => format!("Set[{}]", go_type(to)),
            };
            format!("map[{}]{value}", go_type(from))
        }
    }
}

/// Brace depth, ignoring braces in strings, raw strings and comments.
fn braces(line: &str) -> (usize, usize) {
    let (mut opens, mut closes) = (0usize, 0usize);
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == '\\' && q == '"' {
                    chars.next();
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '`' => quote = Some(c),
                '/' if chars.peek() == Some(&'/') => break,
                '{' | '(' => opens += 1,
                '}' | ')' => {
                    if opens > 0 {
                        opens -= 1;
                    } else {
                        closes += 1;
                    }
                }
                _ => {}
            },
        }
    }
    (opens, closes)
}

/// The gofmt layout of the generated text: indentation by brace depth,
/// `case` one level out, and aligned columns in struct and const blocks.
fn gofmt(text: &str) -> String {
    let mut depth = 0usize;
    let mut lines: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            // gofmt drops a blank line right after an opening brace.
            if !lines.last().is_some_and(|last| last.ends_with('{')) {
                lines.push(String::new());
            }
            continue;
        }
        let (opens, closes) = braces(line);
        let starts_closed = line.starts_with('}') || line.starts_with(')');
        let level = if starts_closed {
            depth.saturating_sub(1)
        } else {
            depth
        };
        let level = if line.starts_with("case ") || line.starts_with("default:") {
            level.saturating_sub(1)
        } else {
            level
        };
        lines.push(format!("{}{line}", "\t".repeat(level)));
        depth = depth + opens - closes.min(depth + opens);
    }
    // Align columns in runs of field or constant lines.
    let mut out: Vec<String> = Vec::new();
    let mut index = 0;
    let field = |line: &str| -> Option<(String, Vec<String>)> {
        let indentation: String = line.chars().take_while(|c| *c == '\t').collect();
        let body = line.trim_start();
        let words: Vec<String> = body.split(' ').map(str::to_owned).collect();
        let simple = words.len() >= 2
            && words[0].chars().all(|c| c.is_alphanumeric() || c == '_')
            && !matches!(
                words[0].as_str(),
                "return"
                    | "if"
                    | "for"
                    | "func"
                    | "var"
                    | "case"
                    | "switch"
                    | "type"
                    | "panic"
                    | "default"
                    | "else"
                    | "go"
                    | "package"
                    | "import"
            )
            && !body.contains(":=")
            && !body.ends_with('{')
            && !body.contains('(')
            && (words.len() == 2 || words[2] == "=" || words[2].starts_with('`'));
        simple.then_some((indentation, words))
    };
    while index < lines.len() {
        let Some((indentation, _)) = field(&lines[index]) else {
            out.push(lines[index].clone());
            index += 1;
            continue;
        };
        let mut run = Vec::new();
        while index < lines.len() {
            match field(&lines[index]) {
                Some((level, words)) if level == indentation => {
                    run.push(words);
                    index += 1;
                }
                _ => break,
            }
        }
        let columns = run.iter().map(Vec::len).max().unwrap_or(0);
        let mut widths = vec![0usize; columns];
        for words in &run {
            for (column, word) in words.iter().enumerate().take(words.len() - 1) {
                widths[column] = widths[column].max(word.chars().count());
            }
        }
        for words in run {
            let mut line = indentation.clone();
            for (column, word) in words.iter().enumerate() {
                if column + 1 == words.len() {
                    line.push_str(word);
                } else {
                    line.push_str(word);
                    line.push_str(&" ".repeat(widths[column] - word.chars().count() + 1));
                }
            }
            out.push(line);
        }
    }
    let mut text = out.join("\n");
    text.push('\n');
    text
}

/// A Go package (plan 0104): the sorts as types, `Config`, `State`, the
/// `Command` structs, the model's interface with one method per action,
/// strict JSON decoders, and `Serve`, rcheck's JSON Lines driver.
pub fn go(model: &Model, package: &str) -> String {
    gofmt(&go_text(model, package))
}

fn go_text(model: &Model, package: &str) -> String {
    let mut out = format!(
        "// Code generated by rmodel from the model {} (plans 0102, 0104). DO NOT EDIT.\n\n\
         package {package}\n\n\
         import (\n\t\"bufio\"\n\t\"bytes\"\n\t\"encoding/json\"\n\t\"fmt\"\n\t\"io\"\n\t\"strconv\"\n\t\"strings\"\n)\n\n\
         var _ = strconv.Itoa\nvar _ = strings.HasPrefix\n\n\
         // Set is a set of atoms or values.\ntype Set[T comparable] map[T]struct{{}}\n\n\
         // object decodes a JSON object with exactly the given fields.\n\
         func object(raw json.RawMessage, names ...string) (map[string]json.RawMessage, error) {{\n\
         \tvar fields map[string]json.RawMessage\n\
         \tif err := json.Unmarshal(raw, &fields); err != nil || fields == nil {{\n\t\treturn nil, fmt.Errorf(\"not an object: %s\", raw)\n\t}}\n\
         \tif len(fields) != len(names) {{\n\t\treturn nil, fmt.Errorf(\"want the fields %v: %s\", names, raw)\n\t}}\n\
         \tfor _, name := range names {{\n\t\tif _, ok := fields[name]; !ok {{\n\t\t\treturn nil, fmt.Errorf(\"missing %s: %s\", name, raw)\n\t\t}}\n\t}}\n\
         \treturn fields, nil\n}}\n\n\
         func array(raw json.RawMessage) ([]json.RawMessage, error) {{\n\
         \tvar items []json.RawMessage\n\
         \tif err := json.Unmarshal(raw, &items); err != nil || items == nil {{\n\t\treturn nil, fmt.Errorf(\"not an array: %s\", raw)\n\t}}\n\
         \treturn items, nil\n}}\n\n\
         func isNull(raw json.RawMessage) bool {{ return bytes.Equal(bytes.TrimSpace(raw), []byte(\"null\")) }}\n\n\
         func encodeInt(x int64) any {{ return x }}\n\n\
         func decodeInt(raw json.RawMessage) (int64, error) {{\n\tvar x int64\n\tif err := json.Unmarshal(raw, &x); err != nil {{\n\t\treturn 0, fmt.Errorf(\"not an integer: %s\", raw)\n\t}}\n\treturn x, nil\n}}\n\n\
         func encodeBool(x bool) any {{ return x }}\n\n\
         func decodeBool(raw json.RawMessage) (bool, error) {{\n\tvar x bool\n\tif err := json.Unmarshal(raw, &x); err != nil {{\n\t\treturn false, fmt.Errorf(\"not a boolean: %s\", raw)\n\t}}\n\treturn x, nil\n}}\n\n\
         func keyBool(x bool) string {{ return strconv.FormatBool(x) }}\n\n\
         func decodeBoolKey(s string) (bool, error) {{\n\tswitch s {{\n\tcase \"true\":\n\t\treturn true, nil\n\tcase \"false\":\n\t\treturn false, nil\n\t}}\n\treturn false, fmt.Errorf(\"not a Bool key: %q\", s)\n}}\n\n\
         func allBool() []bool {{ return []bool{{false, true}} }}\n\n",
        model.name
    );
    // Atom sorts share key, decodeKey, encode, decode and all.
    let atom_codec = |name: &str| {
        format!(
            "func encode{name}(x {name}) any {{ return key{name}(x) }}\n\n\
             func decode{name}(raw json.RawMessage) ({name}, error) {{\n\tvar s string\n\tif err := json.Unmarshal(raw, &s); err != nil {{\n\t\tvar zero {name}\n\t\treturn zero, fmt.Errorf(\"{name}: not a string: %s\", raw)\n\t}}\n\treturn decode{name}Key(s)\n}}\n\n"
        )
    };
    for sig in &model.sigs {
        let size = model.scope.sizes[sig];
        let all: Vec<String> = (0..size).map(|index| index.to_string()).collect();
        out.push_str(&format!(
            "// {sig} is an atom of the signature {sig}.\ntype {sig} uint8\n\n\
             func key{sig}(x {sig}) string {{ return \"{sig}\" + strconv.Itoa(int(x)) }}\n\n\
             func decode{sig}Key(s string) ({sig}, error) {{\n\tn, err := strconv.Atoi(strings.TrimPrefix(s, \"{sig}\"))\n\tif err != nil || n < 0 || n >= {size} || key{sig}({sig}(n)) != s {{\n\t\treturn 0, fmt.Errorf(\"not a {sig}: %q\", s)\n\t}}\n\treturn {sig}(n), nil\n}}\n\n\
             func all{sig}() []{sig} {{ return []{sig}{{{}}} }}\n\n{}",
            all.join(", "),
            atom_codec(sig)
        ));
    }
    for (name, members) in &model.enums {
        if name == BOOL {
            continue;
        }
        let constants: Vec<String> = members
            .iter()
            .map(|m| format!("\t{name}{m} {name} = \"{m}\""))
            .collect();
        let all: Vec<String> = members.iter().map(|m| format!("{name}{m}")).collect();
        out.push_str(&format!(
            "// {name} is an enum.\ntype {name} string\n\nconst (\n{}\n)\n\n\
             func key{name}(x {name}) string {{ return string(x) }}\n\n\
             func decode{name}Key(s string) ({name}, error) {{\n\tfor _, x := range all{name}() {{\n\t\tif string(x) == s {{\n\t\t\treturn x, nil\n\t\t}}\n\t}}\n\treturn \"\", fmt.Errorf(\"not a {name}: %q\", s)\n}}\n\n\
             func all{name}() []{name} {{ return []{name}{{{}}} }}\n\n{}",
            constants.join("\n"),
            all.join(", "),
            atom_codec(name)
        ));
    }
    for (name, children) in &model.sums {
        let marks: String = children
            .iter()
            .map(|c| format!("func ({c}) is{name}() {{}}\n\n"))
            .collect();
        let keys: Vec<String> = children
            .iter()
            .map(|c| format!("\tcase {c}:\n\t\treturn key{c}(v)"))
            .collect();
        let decoders: Vec<String> = children
            .iter()
            .map(|c| {
                format!("\tif v, err := decode{c}Key(s); err == nil {{\n\t\treturn v, nil\n\t}}")
            })
            .collect();
        let alls: Vec<String> = children
            .iter()
            .map(|c| format!("\tfor _, x := range all{c}() {{\n\t\tall = append(all, x)\n\t}}"))
            .collect();
        out.push_str(&format!(
            "// {name} is one of {}.\ntype {name} interface{{ is{name}() }}\n\n{marks}\
             func key{name}(x {name}) string {{\n\tswitch v := x.(type) {{\n{}\n\t}}\n\tpanic(\"not a {name}\")\n}}\n\n\
             func decode{name}Key(s string) ({name}, error) {{\n{}\n\treturn nil, fmt.Errorf(\"not a {name}: %q\", s)\n}}\n\n\
             func all{name}() []{name} {{\n\tvar all []{name}\n{}\n\treturn all\n}}\n\n{}",
            children.join(", "),
            keys.join("\n"),
            decoders.join("\n"),
            alls.join("\n"),
            atom_codec(name)
        ));
    }
    for record in &model.records {
        let name = &record.name;
        let fields: Vec<String> = record
            .fields
            .iter()
            .map(|(field, sort)| format!("\t{} {}", capital(field), go_type(sort)))
            .collect();
        let encoded: Vec<String> = record
            .fields
            .iter()
            .map(|(field, sort)| format!("\"{field}\": encode{sort}(x.{})", capital(field)))
            .collect();
        let names: Vec<String> = record.fields.iter().map(|(f, _)| quote(f)).collect();
        let decoded: Vec<String> = record
            .fields
            .iter()
            .map(|(field, sort)| {
                format!(
                    "\tif x.{0}, err = decode{sort}(fields[\"{field}\"]); err != nil {{\n\t\treturn x, fmt.Errorf(\"{name}.{field}: %w\", err)\n\t}}",
                    capital(field)
                )
            })
            .collect();
        out.push_str(&format!(
            "// {name} is a record, equal when its fields are.\ntype {name} struct {{\n{}\n}}\n\n\
             func encode{name}(x {name}) any {{ return map[string]any{{{}}} }}\n\n\
             func decode{name}(raw json.RawMessage) ({name}, error) {{\n\tvar x {name}\n\tfields, err := object(raw, {})\n\tif err != nil {{\n\t\treturn x, fmt.Errorf(\"{name}: %w\", err)\n\t}}\n{}\n\treturn x, nil\n}}\n\n",
            fields.join("\n"),
            encoded.join(", "),
            names.join(", "),
            decoded.join("\n")
        ));
    }
    // Config and State.
    for var in [false, true] {
        let name = if var { "State" } else { "Config" };
        let relations: Vec<_> = model.relations.iter().filter(|r| r.var == var).collect();
        let fields: Vec<String> = relations
            .iter()
            .map(|r| format!("\t{} {}", capital(&r.name), go_relation(&r.shape)))
            .collect();
        let mut encode = Vec::new();
        let mut decode = Vec::new();
        for relation in &relations {
            let field = capital(&relation.name);
            let json = &relation.name;
            let (sort, mult) = target(&relation.shape);
            let held_encode = |value: &str| match mult {
                Mult::Set | Mult::Some => format!(
                    "func() any {{\n\t\titems := []any{{}}\n\t\tfor e := range {value} {{\n\t\t\titems = append(items, encode{sort}(e))\n\t\t}}\n\t\treturn items\n\t}}()"
                ),
                Mult::Lone => format!(
                    "func() any {{\n\t\tif {value} == nil {{\n\t\t\treturn nil\n\t\t}}\n\t\treturn encode{sort}(*{value})\n\t}}()"
                ),
                Mult::One => format!("encode{sort}({value})"),
            };
            // Decodes `raw` into `target`, returning an error from the
            // surrounding function.
            let held_decode = |raw: &str, assign: &str| match mult {
                Mult::Set | Mult::Some => format!(
                    "items, err := array({raw})\n\tif err != nil {{\n\t\treturn x, fmt.Errorf(\"{json}: %w\", err)\n\t}}\n\tset := Set[{t}]{{}}\n\tfor _, item := range items {{\n\t\te, err := decode{sort}(item)\n\t\tif err != nil {{\n\t\t\treturn x, fmt.Errorf(\"{json}: %w\", err)\n\t\t}}\n\t\tset[e] = struct{{}}{{}}\n\t}}\n\t{assign}set",
                    t = go_type(sort)
                ),
                Mult::Lone => format!(
                    "if !isNull({raw}) {{\n\t\te, err := decode{sort}({raw})\n\t\tif err != nil {{\n\t\t\treturn x, fmt.Errorf(\"{json}: %w\", err)\n\t\t}}\n\t\t{assign}&e\n\t}}"
                ),
                Mult::One => format!(
                    "e, err := decode{sort}({raw})\n\tif err != nil {{\n\t\treturn x, fmt.Errorf(\"{json}: %w\", err)\n\t}}\n\t{assign}e"
                ),
            };
            match &relation.shape {
                Shape::Unary { .. } => {
                    encode.push(format!(
                        "\tm[\"{json}\"] = {}",
                        held_encode(&format!("x.{field}"))
                    ));
                    decode.push(format!(
                        "\t{{\n\t{}\n\t}}",
                        indent(
                            &held_decode(&format!("fields[\"{json}\"]"), &format!("x.{field} = ")),
                            1
                        )
                        .replace("\n ", "\n\t")
                        .trim_start()
                    ));
                }
                Shape::Binary { from: domain, .. } => {
                    let missing = match mult {
                        Mult::Set | Mult::Some => "[]any{}",
                        _ => "nil",
                    };
                    let present = match mult {
                        Mult::Set | Mult::Some => held_encode("v"),
                        _ => format!("encode{sort}(v)"),
                    };
                    let value_type = match mult {
                        Mult::Lone | Mult::One => go_type(sort),
                        _ => format!("Set[{}]", go_type(sort)),
                    };
                    encode.push(format!(
                        "\t{{\n\t\tr := map[string]any{{}}\n\t\tfor _, k := range all{domain}() {{\n\t\t\tif v, ok := x.{field}[k]; ok {{\n\t\t\t\tr[key{domain}(k)] = {}\n\t\t\t}} else {{\n\t\t\t\tr[key{domain}(k)] = {missing}\n\t\t\t}}\n\t\t}}\n\t\tm[\"{json}\"] = r\n\t}}",
                        present.replace("\n", "\n\t\t\t")
                    ));
                    let inner = match mult {
                        Mult::Lone => format!(
                            "if !isNull(raw) {{\n\t\t\te, err := decode{sort}(raw)\n\t\t\tif err != nil {{\n\t\t\t\treturn x, fmt.Errorf(\"{json}: %w\", err)\n\t\t\t}}\n\t\t\tr[k] = e\n\t\t}}"
                        ),
                        Mult::One => format!(
                            "e, err := decode{sort}(raw)\n\t\tif err != nil {{\n\t\t\treturn x, fmt.Errorf(\"{json}: %w\", err)\n\t\t}}\n\t\tr[k] = e"
                        ),
                        _ => format!(
                            "items, err := array(raw)\n\t\tif err != nil {{\n\t\t\treturn x, fmt.Errorf(\"{json}: %w\", err)\n\t\t}}\n\t\tset := Set[{t}]{{}}\n\t\tfor _, item := range items {{\n\t\t\te, err := decode{sort}(item)\n\t\t\tif err != nil {{\n\t\t\t\treturn x, fmt.Errorf(\"{json}: %w\", err)\n\t\t\t}}\n\t\t\tset[e] = struct{{}}{{}}\n\t\t}}\n\t\tr[k] = set",
                            t = go_type(sort)
                        ),
                    };
                    decode.push(format!(
                        "\t{{\n\t\tvar names []string\n\t\tfor _, k := range all{domain}() {{\n\t\t\tnames = append(names, key{domain}(k))\n\t\t}}\n\t\tentries, err := object(fields[\"{json}\"], names...)\n\t\tif err != nil {{\n\t\t\treturn x, fmt.Errorf(\"{json}: %w\", err)\n\t\t}}\n\t\tr := map[{}]{value_type}{{}}\n\t\tfor _, k := range all{domain}() {{\n\t\t\traw := entries[key{domain}(k)]\n\t\t{}\n\t\t}}\n\t\tx.{field} = r\n\t}}",
                        go_type(domain),
                        inner
                    ));
                }
            }
        }
        let names: Vec<String> = relations.iter().map(|r| quote(&r.name)).collect();
        let fields_line = if relations.is_empty() {
            String::new()
        } else {
            fields.join("\n")
        };
        out.push_str(&format!(
            "// {name} holds the model's {}.\ntype {name} struct {{\n{fields_line}\n}}\n\n\
             // Encode writes the JSON form shared with the other bindings.\n\
             func (x {name}) Encode() map[string]any {{\n\tm := map[string]any{{}}\n{}\n\treturn m\n}}\n\n\
             // Decode{name} reads the JSON form, rejecting unknown and missing fields.\n\
             func Decode{name}(raw json.RawMessage) ({name}, error) {{\n\tvar x {name}\n\tfields, err := object(raw{})\n\tif err != nil {{\n\t\treturn x, fmt.Errorf(\"{name}: %w\", err)\n\t}}\n\t_ = fields\n{}\n\treturn x, nil\n}}\n\n",
            if var { "state (vars)" } else { "configuration (rels)" },
            encode.join("\n"),
            names.iter().map(|n| format!(", {n}")).collect::<String>(),
            decode.join("\n")
        ));
    }
    // Commands.
    out.push_str(
        "// Command is one action with its arguments.\ntype Command interface{ isCommand() }\n\n",
    );
    let mut cases = Vec::new();
    let mut methods = Vec::new();
    let mut applies = Vec::new();
    for action in &model.actions {
        let type_name = format!("Command{}", capital(&action.name));
        let fields: Vec<String> = action
            .params
            .iter()
            .map(|(param, sort)| format!("\t{} {}", capital(param), go_type(sort)))
            .collect();
        out.push_str(&format!(
            "// {type_name} is the action {}.\ntype {type_name} struct {{\n{}\n}}\n\nfunc ({type_name}) isCommand() {{}}\n\n",
            action.name,
            fields.join("\n")
        ));
        let mut names = vec![quote("kind")];
        names.extend(action.params.iter().map(|(p, _)| quote(p)));
        let decoded: Vec<String> = action
            .params
            .iter()
            .map(|(param, sort)| {
                format!(
                    "\t\tif c.{0}, err = decode{sort}(fields[\"{param}\"]); err != nil {{\n\t\t\treturn nil, fmt.Errorf(\"{1}.{param}: %w\", err)\n\t\t}}",
                    capital(param),
                    action.name
                )
            })
            .collect();
        cases.push(format!(
            "\tcase \"{0}\":\n\t\tfields, err := object(raw, {1})\n\t\tif err != nil {{\n\t\t\treturn nil, fmt.Errorf(\"{0}: %w\", err)\n\t\t}}\n\t\t_ = fields\n\t\tvar c {type_name}\n{2}\n\t\treturn c, nil",
            action.name,
            names.join(", "),
            decoded.join("\n")
        ));
        let params: Vec<String> = action
            .params
            .iter()
            .map(|(param, sort)| format!("{param} {}", go_type(sort)))
            .collect();
        methods.push(format!(
            "\t// {} applies the action when its guards hold, and says whether it did.\n\t{}({}) bool",
            capital(&action.name),
            capital(&action.name),
            params.join(", ")
        ));
        let args: Vec<String> = action
            .params
            .iter()
            .map(|(param, _)| format!("c.{}", capital(param)))
            .collect();
        applies.push(format!(
            "\tcase {type_name}:\n\t\treturn domain.{}({})",
            capital(&action.name),
            args.join(", ")
        ));
    }
    out.push_str(&format!(
        "// DecodeCommand reads `{{\"kind\": action, param: value}}`.\n\
         func DecodeCommand(raw json.RawMessage) (Command, error) {{\n\tvar head struct {{\n\t\tKind string `json:\"kind\"`\n\t}}\n\tif err := json.Unmarshal(raw, &head); err != nil {{\n\t\treturn nil, fmt.Errorf(\"Command: %w\", err)\n\t}}\n\tswitch head.Kind {{\n{}\n\t}}\n\treturn nil, fmt.Errorf(\"no action %q\", head.Kind)\n}}\n\n",
        cases.join("\n")
    ));
    let model_name = &model.name;
    out.push_str(&format!(
        "// {model_name} is the model: an implementation keeps the state and applies the actions.\n\
         type {model_name} interface {{\n\tState() State\n{}\n}}\n\n\
         // Apply runs a command on an implementation.\n\
         func Apply(domain {model_name}, command Command) bool {{\n\tswitch c := command.(type) {{\n{}\n\t}}\n\tpanic(\"unknown command\")\n}}\n\n\
         // Serve speaks rcheck's JSON Lines driver protocol.\n\
         func Serve(create func(Config) {model_name}, in io.Reader, out io.Writer) error {{\n\
         \tscanner := bufio.NewScanner(in)\n\tscanner.Buffer(make([]byte, 1<<20), 1<<26)\n\tencoder := json.NewEncoder(out)\n\tvar domain {model_name}\n\
         \tfor scanner.Scan() {{\n\t\tvar message struct {{\n\t\t\tOp      string          `json:\"op\"`\n\t\t\tConfig  json.RawMessage `json:\"config\"`\n\t\t\tCommand json.RawMessage `json:\"command\"`\n\t\t}}\n\
         \t\tvar reply any\n\
         \t\tif err := json.Unmarshal(scanner.Bytes(), &message); err != nil {{\n\t\t\treply = map[string]any{{\"error\": err.Error()}}\n\t\t}} else if message.Op == \"reset\" {{\n\
         \t\t\tif config, err := DecodeConfig(message.Config); err != nil {{\n\t\t\t\treply = map[string]any{{\"error\": err.Error()}}\n\t\t\t}} else {{\n\t\t\t\tdomain = create(config)\n\t\t\t\treply = map[string]any{{\"state\": domain.State().Encode()}}\n\t\t\t}}\n\
         \t\t}} else if message.Op == \"apply\" && domain != nil {{\n\
         \t\t\tif command, err := DecodeCommand(message.Command); err != nil {{\n\t\t\t\treply = map[string]any{{\"error\": err.Error()}}\n\t\t\t}} else {{\n\t\t\t\taccepted := Apply(domain, command)\n\t\t\t\treply = map[string]any{{\"accepted\": accepted, \"state\": domain.State().Encode()}}\n\t\t\t}}\n\
         \t\t}} else {{\n\t\t\treply = map[string]any{{\"error\": \"unexpected message\"}}\n\t\t}}\n\
         \t\tif err := encoder.Encode(reply); err != nil {{\n\t\t\treturn err\n\t\t}}\n\t}}\n\treturn scanner.Err()\n}}\n",
        methods.join("\n"),
        applies.join("\n")
    ));
    out
}
