//! Host types and strict codecs for named sequences and payload variants.
use super::{go_type, rust_type, ts_type};
use crate::Model;

pub(super) fn rust(model: &Model, out: &mut String) {
    for seq in &model.sequences {
        let name = &seq.name;
        let item = &seq.element;
        let ty = rust_type(item);
        let max = seq.max;
        out.push_str(&format!(
            r#"pub type {name} = Vec<{ty}>;
fn to_{name}(x: &{name}) -> Value {{
    serde_json::json!({{"$sequence": x.iter().map(to_{item}).collect::<Vec<_>>()}})
}}
fn from_{name}(v: &Value) -> Result<{name}, String> {{
    let items = v.as_object().filter(|o| o.len() == 1)
        .and_then(|o| o.get("$sequence")).and_then(Value::as_array)
        .filter(|a| a.len() <= {max}).ok_or_else(|| format!("not a {name}: {{v}}"))?;
    items.iter().map(from_{item}).collect()
}}

"#
        ));
    }
    for v in &model.variants {
        let name = &v.name;
        let mut cases = String::new();
        let mut encoders = String::new();
        let mut decoders = String::new();
        for case in &v.cases {
            let tag = &case.tag;
            if let Some(item) = &case.payload {
                let ty = rust_type(item);
                cases.push_str(&format!("    {tag}({ty}),\n"));
                encoders.push_str(&format!(
                    "        {name}::{tag}(value) => serde_json::json!({{\"tag\":\"{tag}\",\"value\":to_{item}(value)}}),\n"
                ));
                decoders.push_str(&format!(
                    "        Some(\"{tag}\") if o.len() == 2 => Ok({name}::{tag}(from_{item}(o.get(\"value\").ok_or(\"missing value\")?)?)),\n"
                ));
            } else {
                cases.push_str(&format!("    {tag},\n"));
                encoders.push_str(&format!(
                    "        {name}::{tag} => serde_json::json!({{\"tag\":\"{tag}\"}}),\n"
                ));
                decoders.push_str(&format!(
                    "        Some(\"{tag}\") if o.len() == 1 => Ok({name}::{tag}),\n"
                ));
            }
        }
        out.push_str(&format!(
            r#"#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum {name} {{
{cases}}}
fn to_{name}(x: &{name}) -> Value {{
    match x {{
{encoders}    }}
}}
fn from_{name}(v: &Value) -> Result<{name}, String> {{
    let o = v.as_object().ok_or_else(|| format!("not a {name}: {{v}}"))?;
    match o.get("tag").and_then(Value::as_str) {{
{decoders}        _ => Err(format!("not a {name}: {{v}}")),
    }}
}}

"#
        ));
    }
}

pub(super) fn typescript(model: &Model, out: &mut String, converters: &mut String) {
    for seq in &model.sequences {
        let name = &seq.name;
        let item = &seq.element;
        let ty = ts_type(item);
        let max = seq.max;
        out.push_str(&format!("export type {name} = readonly {ty}[];\n"));
        converters.push_str(&format!(
            r#"function from_{name}(v: unknown): {name} {{
  if (typeof v !== "object" || v === null || Object.keys(v).length !== 1 || !("$sequence" in v) || !Array.isArray(v.$sequence) || v.$sequence.length > {max}) throw new Error("not a {name}");
  return v.$sequence.map(from_{item});
}}
const to_{name} = (x: {name}): unknown => ({{ $sequence: x.map(to_{item}) }});
"#));
    }
    for variant in &model.variants {
        let name = &variant.name;
        let mut options = Vec::new();
        let mut encoders = String::new();
        let mut decoders = String::new();
        for case in &variant.cases {
            let tag = &case.tag;
            if let Some(item) = &case.payload {
                let ty = ts_type(item);
                options.push(format!(
                    "{{ readonly tag: \"{tag}\"; readonly value: {ty} }}"
                ));
                encoders.push_str(&format!(
                    "    case \"{tag}\": return {{tag: x.tag, value: to_{item}(x.value)}};\n"
                ));
                decoders.push_str(&format!("    case \"{tag}\": if (Object.keys(v).length === 2 && \"value\" in v) return {{tag: v.tag, value: from_{item}(v.value)}}; break;\n"));
            } else {
                options.push(format!("{{ readonly tag: \"{tag}\" }}"));
                encoders.push_str(&format!("    case \"{tag}\": return {{tag: x.tag}};\n"));
                decoders.push_str(&format!("    case \"{tag}\": if (Object.keys(v).length === 1) return {{tag: v.tag}}; break;\n"));
            }
        }
        out.push_str(&format!("export type {name} = {};\n", options.join(" | ")));
        converters.push_str(&format!(
            r#"function from_{name}(v: unknown): {name} {{
  if (typeof v !== "object" || v === null || !("tag" in v)) throw new Error("not a {name}");
  switch (v.tag) {{
{decoders}  }}
  throw new Error("not a {name}");
}}
function to_{name}(x: {name}): unknown {{
  switch (x.tag) {{
{encoders}  }}
}}
"#
        ));
    }
}

pub(super) fn go(model: &Model, out: &mut String) {
    for seq in &model.sequences {
        let name = &seq.name;
        let item = &seq.element;
        let ty = go_type(item);
        let max = seq.max;
        out.push_str(&format!(
            r#"// {name} is a bounded sequence of {item}.
type {name} []{ty}

func encode{name}(x {name}) any {{
    items := make([]any, len(x))
    for i, item := range x {{
        items[i] = encode{item}(item)
    }}
    return map[string]any{{"$sequence": items}}
}}

func decode{name}(raw json.RawMessage) ({name}, error) {{
    fields, err := object(raw, "$sequence")
    if err != nil {{
        return nil, err
    }}
    items, err := array(fields["$sequence"])
    if err != nil || len(items) > {max} {{
        return nil, fmt.Errorf("not a {name}: %s", raw)
    }}
    seq := make({name}, len(items))
    for i, item := range items {{
        value, err := decode{item}(item)
        if err != nil {{
            return nil, err
        }}
        seq[i] = value
    }}
    return seq, nil
}}

"#
        ));
    }
    for variant in &model.variants {
        let name = &variant.name;
        let mut cases = String::new();
        let mut encoders = String::new();
        let mut decoders = String::new();
        for case in &variant.cases {
            let tag = &case.tag;
            let case_type = format!("{name}{tag}");
            if let Some(item) = &case.payload {
                let ty = go_type(item);
                cases.push_str(&format!("type {case_type} struct {{\nValue {ty}\n}}\n\n"));
                encoders.push_str(&format!("case {case_type}:\nreturn map[string]any{{\"tag\": \"{tag}\", \"value\": encode{item}(v.Value)}}\n"));
                decoders.push_str(&format!(
                    r#"case "{tag}":
    fields, err := object(raw, "tag", "value")
    if err != nil {{
        return nil, err
    }}
    value, err := decode{item}(fields["value"])
    if err != nil {{
        return nil, err
    }}
    return {case_type}{{Value: value}}, nil
"#
                ));
            } else {
                cases.push_str(&format!("type {case_type} struct{{}}\n\n"));
                encoders.push_str(&format!(
                    "case {case_type}:\nreturn map[string]any{{\"tag\": \"{tag}\"}}\n"
                ));
                decoders.push_str(&format!(
                    "case \"{tag}\":\nif _, err := object(raw, \"tag\"); err != nil {{\nreturn nil, err\n}}\nreturn {case_type}{{}}, nil\n"
                ));
            }
            cases.push_str(&format!("func ({case_type}) is{name}() {{}}\n\n"));
        }
        let encode_switch = if variant.cases.iter().any(|case| case.payload.is_some()) {
            "v := x.(type)"
        } else {
            "x.(type)"
        };
        out.push_str(&format!(
            r#"// {name} is a closed variant.
type {name} interface{{ is{name}() }}

{cases}func encode{name}(x {name}) any {{
    switch {encode_switch} {{
{encoders}    }}
    panic("not a {name}")
}}

func decode{name}(raw json.RawMessage) ({name}, error) {{
    var head struct {{
        Tag string `json:"tag"`
    }}
    if err := json.Unmarshal(raw, &head); err != nil {{
        return nil, err
    }}
    switch head.Tag {{
{decoders}    }}
    return nil, fmt.Errorf("not a {name}: %s", raw)
}}

"#
        ));
    }
}

pub(super) fn moonbit(model: &Model, out: &mut String) {
    use super::{mbt_codec, mbt_type};
    let block = |s: String| format!("///|\n{s}\n\n");
    for seq in &model.sequences {
        let name = &seq.name;
        let c = mbt_codec(name);
        let item = mbt_codec(&seq.element);
        let ty = mbt_type(&seq.element);
        let max = seq.max;
        out.push_str(&block(format!(
            "pub(all) struct {name} {{\n  values : Array[{ty}]\n}}"
        )));
        out.push_str(&block(format!(
            r#"fn encode_{c}(x : {name}) -> Json {{
  let m : Map[String, Json] = Map([])
  m["$sequence"] = Json::array([for v in x.values => encode_{item}(v)])
  Json::object(m)
}}"#
        )));
        out.push_str(&block(format!(
            r#"fn decode_{c}(v : Json) -> {name} raise DecodeError {{
  let m = fields(v, ["$sequence"])
  let entries = items(m["$sequence"])
  if entries.length() > {max} {{
    raise DecodeError("not a {name}")
  }}
  {{ values: [for entry in entries => decode_{item}(entry)] }}
}}"#
        )));
    }
    for variant in &model.variants {
        let name = &variant.name;
        let c = mbt_codec(name);
        let mut cases = String::new();
        let mut encoders = String::new();
        let mut decoders = String::new();
        for case in &variant.cases {
            let tag = &case.tag;
            if let Some(sort) = &case.payload {
                let ty = mbt_type(sort);
                let item = mbt_codec(sort);
                cases.push_str(&format!("  {tag}({ty})\n"));
                encoders.push_str(&format!(
                    "    {tag}(value) => {{\n      m[\"tag\"] = Json::string(\"{tag}\")\n      m[\"value\"] = encode_{item}(value)\n    }}\n"
                ));
                decoders.push_str(&format!(
                    "    \"{tag}\" => {{\n      let m = fields(v, [\"tag\", \"value\"])\n      {name}::{tag}(decode_{item}(m[\"value\"]))\n    }}\n"
                ));
            } else {
                cases.push_str(&format!("  {tag}\n"));
                encoders.push_str(&format!(
                    "    {tag} => m[\"tag\"] = Json::string(\"{tag}\")\n"
                ));
                decoders.push_str(&format!(
                    "    \"{tag}\" => {{\n      ignore(fields(v, [\"tag\"]))\n      {name}::{tag}\n    }}\n"
                ));
            }
        }
        out.push_str(&block(format!("pub(all) enum {name} {{\n{cases}}}")));
        out.push_str(&block(format!(
            r#"fn encode_{c}(x : {name}) -> Json {{
  let m : Map[String, Json] = Map([])
  match x {{
{encoders}  }}
  Json::object(m)
}}"#
        )));
        out.push_str(&block(format!(
            r#"fn decode_{c}(v : Json) -> {name} raise DecodeError {{
  guard v is Object(o) && o.get("tag") is Some(String(tag)) else {{
    raise DecodeError("not a {name}")
  }}
  match tag {{
{decoders}    _ => raise DecodeError("not a {name}")
  }}
}}"#
        )));
    }
}
