//! Witness pairs for property tests, derived from the reference model.
use super::{command, relations};
use rmodel::reference::{Machine, Next, Outcome, Snapshot, Step, analyze};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

type Actions = BTreeMap<String, (Value, Step)>;
type Witnesses = BTreeMap<(String, &'static str), Value>;

fn actions(machine: &Machine<'_>, config: &Snapshot, state: &Snapshot) -> Actions {
    machine
        .steps(config, state)
        .into_iter()
        .map(|step| {
            let input = command(&step.action, &step.args);
            (input.to_string(), (input, step))
        })
        .collect()
}

// The command wire form represents integers directly, and records as objects.
fn increments(value: &Value) -> Vec<Value> {
    if let Some(number) = value.as_i64() {
        return number.checked_add(1).map(Value::from).into_iter().collect();
    }
    if let Some(items) = value.as_array() {
        let mut neighbors = Vec::new();
        for (index, item) in items.iter().enumerate() {
            for increment in increments(item) {
                let mut changed = items.clone();
                changed[index] = increment;
                neighbors.push(Value::Array(changed));
            }
        }
        return neighbors;
    }
    let Some(fields) = value.as_object() else {
        return Vec::new();
    };
    let mut neighbors = Vec::new();
    for (key, field) in fields {
        for increment in increments(field) {
            let mut changed = fields.clone();
            changed.insert(key.clone(), increment);
            neighbors.push(Value::Object(changed));
        }
    }
    neighbors
}

#[derive(Clone, Copy)]
struct At<'a> {
    state: &'a Snapshot,
    prefix: &'a [Value],
}

fn record(
    witnesses: &mut Witnesses,
    kind: &'static str,
    (input, step): &(Value, Step),
    left: At<'_>,
    (other, next): &(Value, Step),
    right: At<'_>,
    encode: &impl Fn(&Snapshot) -> Value,
) {
    if step.next.is_some() == next.next.is_some() {
        return;
    }
    let case = |input: &Value, step: &Step, at: At<'_>| {
        let after = match &step.next {
            None => at.state,
            Some(Next::State(state)) => state,
            _ => unreachable!("checked a complete and valid model"),
        };
        json!({
            "prefix": at.prefix,
            "command": input,
            "accepted": step.next.is_some(),
            "state": encode(after)
        })
    };
    witnesses
        .entry((step.action.clone(), kind))
        .or_insert_with(|| {
            json!({
                "action": step.action,
                "kind": kind,
                "examples": [case(input, step, left), case(other, next, right)]
            })
        });
}

/// A deterministic corpus of reachable guard boundaries. Reject incomplete
/// models rather than presenting partial discovery as full coverage.
pub fn boundary_cases(model: &rmodel::Model, max_states: usize) -> Result<Value, String> {
    let report = analyze(model, max_states);
    if report.scope != Outcome::Holds || report.declarations != Outcome::Holds {
        return Err(format!(
            "boundary cases require a complete, nonempty model: scope {:?}, declarations {:?}",
            report.scope, report.declarations
        ));
    }
    let machine = Machine::new(model)?;
    let mut corpus = Vec::new();
    let encode = |state: &Snapshot| relations(model, machine.state_data(state));
    for config in machine.configurations() {
        let initial = machine.initial(&config);
        if initial.len() != 1 {
            return Err("boundary cases require one initial state per configuration".into());
        }
        let mut seen: BTreeSet<_> = initial.iter().cloned().collect();
        let mut queue: VecDeque<_> = initial
            .into_iter()
            .map(|state| (state, Vec::new()))
            .collect();
        let mut witnesses = Witnesses::new();
        while let Some((state, prefix)) = queue.pop_front() {
            let current = actions(&machine, &config, &state);
            for pair @ (input, step) in current.values() {
                let at = At {
                    state: &state,
                    prefix: &prefix,
                };
                for neighbor in increments(input) {
                    let Some(other) = current.get(&neighbor.to_string()) else {
                        continue;
                    };
                    record(&mut witnesses, "integer", pair, at, other, at, &encode);
                    let (Some(Next::State(left)), Some(Next::State(right))) =
                        (&step.next, &other.1.next)
                    else {
                        continue;
                    };
                    if left == right {
                        continue;
                    }
                    let mut left_prefix = prefix.clone();
                    left_prefix.push(input.clone());
                    let mut right_prefix = prefix.clone();
                    right_prefix.push(other.0.clone());
                    let right_actions = actions(&machine, &config, right);
                    for (key, later) in actions(&machine, &config, left) {
                        if let Some(alternative) = right_actions.get(&key) {
                            record(
                                &mut witnesses,
                                "continuation",
                                &later,
                                At {
                                    state: left,
                                    prefix: &left_prefix,
                                },
                                alternative,
                                At {
                                    state: right,
                                    prefix: &right_prefix,
                                },
                                &encode,
                            );
                        }
                    }
                }
            }
            for (input, step) in current.values() {
                let Some(Next::State(next)) = &step.next else {
                    continue;
                };
                let mut path = prefix.clone();
                path.push(input.clone());
                let following = actions(&machine, &config, next);
                for (key, other) in &following {
                    if !witnesses.contains_key(&(other.1.action.clone(), "transition"))
                        && let Some(pair) = current.get(key)
                    {
                        record(
                            &mut witnesses,
                            "transition",
                            pair,
                            At {
                                state: &state,
                                prefix: &prefix,
                            },
                            other,
                            At {
                                state: next,
                                prefix: &path,
                            },
                            &encode,
                        );
                    }
                }
                if seen.insert(next.clone()) {
                    queue.push_back((next.clone(), path));
                }
            }
        }
        let config = relations(model, machine.configuration_data(&config));
        corpus.extend(witnesses.into_values().map(|mut pair| {
            pair["config"] = config.clone();
            pair
        }));
    }
    if corpus.is_empty() {
        return Err("no guard boundary pairs found".into());
    }
    Ok(json!({ "version": 1, "pairs": corpus }))
}

fn require_safe_scope(model: &rmodel::Model) -> Result<(), String> {
    const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;
    if let Some((low, high)) = model.scope.ints
        && (low < -MAX_SAFE_INTEGER || high > MAX_SAFE_INTEGER)
    {
        return Err("pbt-typescript requires a scope within JavaScript's safe integers".into());
    }
    Ok(())
}

/// Emit a portable source module containing the reference cases and a fast-check property.
pub fn boundary_typescript(model: &rmodel::Model, max_states: usize) -> Result<String, String> {
    require_safe_scope(model)?;
    let cases = boundary_cases(model, max_states)?;
    let source = include_str!("boundary_property.ts.in");
    Ok(format!(
        "{source}\nexport const boundaryCases = {};\n",
        serde_json::to_string_pretty(&cases).expect("JSON")
    ))
}

/// Emit a Go test companion to the model's generated bindings.
pub fn boundary_go(model: &rmodel::Model, max_states: usize) -> Result<String, String> {
    let cases = boundary_cases(model, max_states)?;
    let source = include_str!("boundary_property.go.in")
        .replace("@PACKAGE@", &model.name.to_lowercase())
        .replace("@MODEL@", &model.name);
    let cases = serde_json::to_string_pretty(&cases)
        .expect("JSON")
        .replace('`', "\\u0060");
    Ok(format!("{source}\nconst boundaryCorpus = `{cases}`\n"))
}

/// Value boundaries probe decoders. In particular, invalid values never reach
/// the implementation of an action.
pub fn data_boundary_cases(model: &rmodel::Model) -> Result<Value, String> {
    let cases = rmodel::reference::data_boundaries(model)
        .into_iter()
        .map(|case| {
            json!({ "sort": case.sort, "input": super::element(&case.input), "valid": case.valid })
        })
        .collect::<Vec<_>>();
    if cases.is_empty() {
        return Err("no data refinement boundary cases found".into());
    }
    Ok(json!({ "version": 1, "cases": cases }))
}

pub fn data_boundary_typescript(model: &rmodel::Model) -> Result<String, String> {
    require_safe_scope(model)?;
    let cases = data_boundary_cases(model)?;
    let data = serde_json::to_string_pretty(&cases).expect("JSON");
    let source = include_str!("data_boundary_property.ts.in");
    Ok(format!(
        "{source}\nexport const dataBoundaryCases = {data};\n"
    ))
}
