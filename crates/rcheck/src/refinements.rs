//! Decide whether a data type has a value, independently of reachability.
use rmodel::{CheckedModel, INT, Model, NAT, Record, RefinementOperand};
use serde::Serialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fmt::Write};

pub const DEFAULT_WORK: u32 = 1_000_000;

#[derive(Serialize)]
pub struct Report {
    pub model: String,
    pub solver: &'static str,
    pub int_scope: Option<(i64, i64)>,
    pub types: Vec<TypeReport>,
}

impl Report {
    pub fn exit_code(&self) -> i32 {
        if self
            .types
            .iter()
            .any(|entry| matches!(entry.domain, Answer::Unsat))
        {
            return 1;
        }
        if self.types.is_empty()
            || self.types.iter().any(|entry| {
                matches!(entry.domain, Answer::Unknown { .. })
                    || matches!(entry.scope, Answer::Unknown { .. })
            })
        {
            return 3;
        }
        0
    }
}

#[derive(Serialize)]
pub struct TypeReport {
    #[serde(rename = "type")]
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub domain: Answer,
    pub scope: Answer,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum Answer {
    Sat { value: Value },
    Unsat,
    Unknown { reason: String },
}

pub fn check(
    model: &CheckedModel,
    selected: Option<&str>,
    max_work: u32,
) -> Result<Report, String> {
    if let Some(name) = selected
        && !model
            .record(name)
            .is_some_and(|record| record.data.is_some())
    {
        return Err(format!("no data type named {name}"));
    }

    let types = model
        .records
        .iter()
        .filter(|record| {
            selected.map_or(!record.refinements.is_empty(), |name| record.name == name)
        })
        .map(|record| {
            let domain = query(model, record, None, max_work)?;
            let scope = if matches!(domain, Answer::Unsat) {
                Answer::Unsat
            } else {
                query(model, record, model.scope.ints, max_work)?
            };
            Ok(TypeReport {
                name: record.name.clone(),
                location: None,
                domain,
                scope,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(Report {
        model: model.name.clone(),
        solver: "rusmt",
        int_scope: model.scope.ints,
        types,
    })
}

/// A JSON value whose numeric leaves and variant choice are supplied by SMT.
enum Template {
    Literal(Value),
    Integer(String),
    Record(Vec<(String, Template)>),
    Variant {
        selector: String,
        cases: Vec<(String, Option<Template>)>,
    },
}

impl Template {
    fn symbol(&self, path: &[String]) -> Option<&str> {
        match (self, path.split_first()) {
            (Self::Integer(symbol), None) => Some(symbol),
            (Self::Record(fields), Some((field, rest))) => fields
                .iter()
                .find(|(name, _)| name == field)?
                .1
                .symbol(rest),
            _ => None,
        }
    }

    fn value(self, values: &BTreeMap<String, String>) -> Result<Value, String> {
        let read = |name: &str| {
            values
                .get(name)
                .and_then(|text| text.parse::<i64>().ok())
                .ok_or_else(|| {
                    format!(
                        "rusmt witness value {name} is missing or outside the wire integer range"
                    )
                })
        };
        match self {
            Self::Literal(value) => Ok(value),
            Self::Integer(name) => Ok(Value::from(read(&name)?)),
            Self::Record(fields) => Ok(Value::Object(
                fields
                    .into_iter()
                    .map(|(name, template)| Ok((name, template.value(values)?)))
                    .collect::<Result<_, String>>()?,
            )),
            Self::Variant { selector, cases } => {
                let index = usize::try_from(read(&selector)?).map_err(|error| error.to_string())?;
                let (tag, payload) = cases.into_iter().nth(index).ok_or_else(|| {
                    format!("rusmt witness selector {selector} is outside its cases")
                })?;
                match payload {
                    Some(payload) => Ok(json!({"tag": tag, "value": payload.value(values)?})),
                    None => Ok(json!({"tag": tag})),
                }
            }
        }
    }
}

struct Encoding<'a> {
    model: &'a Model,
    bounds: Option<(i64, i64)>,
    declarations: String,
    variables: usize,
    remaining_nodes: usize,
}

fn integer(value: i64) -> String {
    if value < 0 {
        format!("(- {})", value.unsigned_abs())
    } else {
        value.to_string()
    }
}

impl Encoding<'_> {
    fn variable(&mut self) -> String {
        let symbol = format!("v{}", self.variables);
        self.variables += 1;
        writeln!(self.declarations, "(declare-const {symbol} Int)").expect("writing to String");
        symbol
    }

    fn encode(&mut self, sort: &str, constraints: &mut Vec<String>) -> Result<Template, String> {
        self.remaining_nodes = self
            .remaining_nodes
            .checked_sub(1)
            .ok_or("data type exceeds the SMT encoding node limit")?;
        if matches!(sort, INT | NAT) {
            let symbol = self.variable();
            if sort == NAT {
                constraints.push(format!("(>= {symbol} 0)"));
            }
            if let Some((low, high)) = self.bounds {
                constraints.push(format!("(>= {symbol} {})", integer(low)));
                constraints.push(format!("(<= {symbol} {})", integer(high)));
            }
            return Ok(Template::Integer(symbol));
        }
        if let Some(record) = self.model.record(sort) {
            let fields = record
                .fields
                .iter()
                .map(|(name, sort)| Ok((name.clone(), self.encode(sort, constraints)?)))
                .collect::<Result<_, String>>()?;
            let template = Template::Record(fields);
            for clause in &record.refinements {
                let operand = |value: &RefinementOperand| match value {
                    RefinementOperand::Literal(n) => Ok(integer(*n)),
                    RefinementOperand::Field(path) => {
                        template.symbol(path).map(str::to_owned).ok_or_else(|| {
                            format!("{sort}: unresolved where field {}", path.join("."))
                        })
                    }
                };
                let left = operand(&clause.left)?;
                let right = operand(&clause.right)?;
                let predicate = match clause.symbol() {
                    "!=" => format!("(not (= {left} {right}))"),
                    op => format!("({op} {left} {right})"),
                };
                constraints.push(predicate);
            }
            return Ok(template);
        }
        if let Some(variant) = self.model.variant(sort) {
            let selector = self.variable();
            let mut alternatives = Vec::new();
            let cases = variant
                .cases
                .iter()
                .enumerate()
                .map(|(index, case)| {
                    let mut branch = vec![format!("(= {selector} {index})")];
                    let payload = case
                        .payload
                        .as_deref()
                        .map(|payload| self.encode(payload, &mut branch))
                        .transpose()?;
                    alternatives.push(format!("(and {})", branch.join(" ")));
                    Ok((case.tag.clone(), payload))
                })
                .collect::<Result<_, String>>()?;
            constraints.push(format!("(or {})", alternatives.join(" ")));
            return Ok(Template::Variant { selector, cases });
        }
        if let Some((_, members)) = self.model.sums.iter().find(|(name, _)| name == sort) {
            return self.encode(&members[0], constraints);
        }
        let literal = if sort == rmodel::BOOL {
            json!(false)
        } else if self.model.sequence(sort).is_some() {
            json!({"$sequence": []})
        } else if let Some((_, members)) = self.model.enums.iter().find(|(name, _)| name == sort) {
            json!(members[0])
        } else if self.model.sigs.iter().any(|name| name == sort) {
            json!(format!("{sort}0"))
        } else {
            return Err(format!("unknown SMT value sort {sort}"));
        };
        Ok(Template::Literal(literal))
    }
}

fn query(
    model: &Model,
    record: &Record,
    bounds: Option<(i64, i64)>,
    max_work: u32,
) -> Result<Answer, String> {
    let mut encoding = Encoding {
        model,
        bounds,
        declarations: String::new(),
        variables: 0,
        remaining_nodes: 1024,
    };
    let mut constraints = Vec::new();
    let template = match encoding.encode(&record.name, &mut constraints) {
        Ok(template) => template,
        Err(reason) if encoding.remaining_nodes == 0 => return Ok(Answer::Unknown { reason }),
        Err(error) => return Err(error),
    };
    if constraints.is_empty() {
        constraints.push("true".into());
    }
    for constraint in constraints {
        writeln!(encoding.declarations, "(assert {constraint})").expect("writing to String");
    }
    let problem = match rusmt::Problem::parse(&encoding.declarations) {
        Ok(problem) => problem,
        Err(error) if error.code == "resource" => {
            return Ok(Answer::Unknown {
                reason: error.message,
            });
        }
        Err(error) => {
            return Err(format!(
                "{}: rusmt rejected generated constraints: {error}",
                record.name
            ));
        }
    };
    let solution = problem.solve(max_work);
    match solution.status.as_str() {
        "sat" => Ok(Answer::Sat {
            value: template.value(&solution.values)?,
        }),
        "unsat" => Ok(Answer::Unsat),
        "unknown" => Ok(Answer::Unknown {
            reason: solution
                .reason
                .unwrap_or_else(|| "rusmt returned unknown".into()),
        }),
        other => Err(format!("unexpected rusmt status {other}")),
    }
}
