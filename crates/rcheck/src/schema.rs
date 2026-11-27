//! A validator for the JSON Schema keywords `rmodel::bindings::json_schema`
//! writes (plan 0102): `$ref` into `$defs`, `enum`, `const`, `type`,
//! `properties`, `required`, `additionalProperties: false`, `items`,
//! `uniqueItems`, `minItems`, `maxItems`, `minimum`, `maximum`, `anyOf` and `oneOf`. Any other keyword is an
//! error rather than ignored. Boolean schemas accept or reject every value.
//! The tests compare it with ajv.
use serde_json::Value;

pub fn validate(schema: &Value, definition: &str, value: &Value) -> Result<(), String> {
    let root = schema;
    let target = root
        .pointer(&format!("/$defs/{definition}"))
        .ok_or_else(|| format!("no definition {definition}"))?;
    check(root, target, value, &format!("/{definition}"))
}

fn check(root: &Value, schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    if let Some(allowed) = schema.as_bool() {
        return allowed
            .then_some(())
            .ok_or_else(|| format!("{path}: no value is allowed"));
    }
    let schema = schema
        .as_object()
        .ok_or_else(|| format!("{path}: schema is not an object"))?;
    for (keyword, argument) in schema {
        match keyword.as_str() {
            "$ref" => {
                let reference = argument.as_str().unwrap_or_default();
                let pointer = reference
                    .strip_prefix('#')
                    .ok_or_else(|| format!("{path}: unsupported $ref {reference}"))?;
                let target = root
                    .pointer(pointer)
                    .ok_or_else(|| format!("{path}: unresolved $ref {reference}"))?;
                check(root, target, value, path)?;
            }
            "enum" => {
                if !argument
                    .as_array()
                    .is_some_and(|options| options.contains(value))
                {
                    return Err(format!("{path}: {value} is not one of {argument}"));
                }
            }
            "const" => {
                if argument != value {
                    return Err(format!("{path}: {value} is not {argument}"));
                }
            }
            "type" => {
                let fits = match argument.as_str().unwrap_or_default() {
                    "integer" => value.is_i64() || value.is_u64(),
                    "boolean" => value.is_boolean(),
                    "object" => value.is_object(),
                    "array" => value.is_array(),
                    "null" => value.is_null(),
                    "string" => value.is_string(),
                    other => return Err(format!("{path}: unsupported type {other}")),
                };
                if !fits {
                    return Err(format!("{path}: {value} is not of type {argument}"));
                }
            }
            "minimum" | "maximum" => {
                let integer = |number: &Value| {
                    number
                        .as_i64()
                        .map(i128::from)
                        .or_else(|| number.as_u64().map(i128::from))
                };
                let limit =
                    integer(argument).ok_or_else(|| format!("{path}: malformed {keyword}"))?;
                if let Some(number) = integer(value)
                    && (keyword == "minimum" && number < limit
                        || keyword == "maximum" && number > limit)
                {
                    return Err(format!("{path}: {value} exceeds {keyword} {limit}"));
                }
            }
            "properties" => {
                if let (Some(properties), Some(object)) = (argument.as_object(), value.as_object())
                {
                    for (name, inner) in properties {
                        if let Some(field) = object.get(name) {
                            check(root, inner, field, &format!("{path}/{name}"))?;
                        }
                    }
                }
            }
            "required" => {
                if let Some(object) = value.as_object() {
                    for name in argument.as_array().into_iter().flatten() {
                        let name = name.as_str().unwrap_or_default();
                        if !object.contains_key(name) {
                            return Err(format!("{path}: missing {name}"));
                        }
                    }
                }
            }
            "additionalProperties" => {
                if argument != &Value::Bool(false) {
                    return Err(format!("{path}: unsupported additionalProperties"));
                }
                if let Some(object) = value.as_object() {
                    let known = schema.get("properties").and_then(Value::as_object);
                    for name in object.keys() {
                        if !known.is_some_and(|known| known.contains_key(name)) {
                            return Err(format!("{path}: unexpected property {name}"));
                        }
                    }
                }
            }
            "items" => {
                for (index, item) in value.as_array().into_iter().flatten().enumerate() {
                    check(root, argument, item, &format!("{path}/{index}"))?;
                }
            }
            "uniqueItems" => {
                if let Some(items) = value.as_array() {
                    for (index, item) in items.iter().enumerate() {
                        if items[..index].contains(item) {
                            return Err(format!("{path}: {item} appears twice"));
                        }
                    }
                }
            }
            "maxItems" => {
                let maximum = argument.as_u64().unwrap_or_default() as usize;
                if value.as_array().is_some_and(|items| items.len() > maximum) {
                    return Err(format!("{path}: more than {maximum} items"));
                }
            }
            "minItems" => {
                let minimum = argument.as_u64().unwrap_or_default() as usize;
                if value.as_array().is_some_and(|items| items.len() < minimum) {
                    return Err(format!("{path}: fewer than {minimum} items"));
                }
            }
            "anyOf" | "oneOf" => {
                let options = argument.as_array().cloned().unwrap_or_default();
                let passing = options
                    .iter()
                    .filter(|option| check(root, option, value, path).is_ok())
                    .count();
                let ok = if keyword == "anyOf" {
                    passing >= 1
                } else {
                    passing == 1
                };
                if !ok {
                    return Err(format!("{path}: {value} matches {passing} of {keyword}"));
                }
            }
            "$schema" | "title" | "$defs" => {}
            other => return Err(format!("{path}: unsupported keyword {other}")),
        }
    }
    Ok(())
}
