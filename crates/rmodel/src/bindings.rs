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
