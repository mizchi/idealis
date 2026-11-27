//! Recover reference commands from the explicit checker's lasso.
use super::{command, relations};
use crate::{Budget, Kind, Question, Verdict, checkers::Explicit};
use idealis_core::Value as Portable;
use rmodel::reference::{Data, Machine, Next};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Lasso {
    trace: Vec<TraceState>,
    loop_start: usize,
    loop_action: String,
}

#[derive(Deserialize)]
struct TraceState {
    action: Option<String>,
    values: Vec<Portable>,
}

pub(super) fn witness(
    model: &rmodel::Model,
    question: &str,
    states: usize,
) -> Result<Value, String> {
    let lowered =
        rmodel::to_tla_temporal_witness(model, question).map_err(|error| error.to_string())?;
    let answer = Explicit.answer_lowered(
        model,
        &Question {
            name: question.into(),
            kind: Kind::Temporal,
        },
        Budget {
            states,
            ..Budget::default()
        },
        || Ok((lowered.source, lowered.config)),
    );
    if answer.verdict == Verdict::Error {
        return Err(answer
            .detail
            .unwrap_or_else(|| "explicit checker failed".into()));
    }
    let trace = if answer.verdict == Verdict::Violated {
        let raw = answer.lasso.ok_or("the explicit violation has no loop")?;
        let lasso = serde_json::from_value(raw).map_err(|err| format!("invalid lasso: {err}"))?;
        replay_trace(model, lasso, &lowered.actions)?
    } else {
        Value::Null
    };
    let mut response = json!({
        "question": question, "verdict": answer.verdict,
        "complete": answer.verdict != Verdict::Unknown, "trace": trace
    });
    if !answer.diagnostics.is_empty() {
        response["diagnostics"] = json!(answer.diagnostics);
    }
    Ok(response)
}

pub(super) fn rprob_witness(
    model: &rmodel::Model,
    name: &str,
    states: usize,
) -> Result<Value, String> {
    let question = Question {
        name: name.into(),
        kind: Kind::Temporal,
    };
    let answer = rprob_temporal_answer(
        model,
        &question,
        Budget {
            states,
            ..Budget::default()
        },
    )?;
    let mut response = json!({
        "question": name,
        "verdict": answer.verdict,
        "complete": answer.verdict != Verdict::Unknown,
        "trace": answer.lasso,
    });
    if !answer.diagnostics.is_empty() {
        response["diagnostics"] = json!(answer.diagnostics);
    }
    if let Some(reason) = answer.detail {
        response["verificationReason"] = json!(reason);
    }
    Ok(response)
}

pub(crate) fn rprob_temporal_answer(
    model: &rmodel::Model,
    question: &Question,
    budget: Budget,
) -> Result<crate::Answer, String> {
    use rmodel::reference::{LassoError, Outcome};
    use rprob::TemporalOutcome;
    let mut answer = crate::checkers::answer(
        "rprob",
        question,
        crate::Coverage::Complete,
        Verdict::Unknown,
    );
    if budget.work == 0 {
        answer.diagnostics.push(crate::Diagnostic::budget("work"));
        return Ok(answer);
    }
    let result = rprob::temporal_witness(model, &question.name, budget.states)
        .map_err(|error| error.to_string())?;
    match result {
        TemporalOutcome::Empty => answer.verdict = Verdict::Empty,
        TemporalOutcome::Holds => answer.verdict = Verdict::Holds,
        TemporalOutcome::Unknown {
            scope,
            declarations,
            stop,
        } => {
            answer.diagnostics = crate::reference_diagnostics(scope, stop.as_ref());
            if let Outcome::Violated { depth } = declarations {
                answer
                    .diagnostics
                    .push(crate::Diagnostic::DeclarationViolation { depth });
            }
        }
        TemporalOutcome::Violated(result) => {
            let mut trace = super::encode_witness(model, result.witness);
            let mut operations = trace["commands"]
                .as_array_mut()
                .map(std::mem::take)
                .expect("encoded commands")
                .into_iter();
            trace["commands"] = Value::Array(
                result
                    .stutters
                    .into_iter()
                    .map(|stutter| {
                        if stutter {
                            Value::Null
                        } else {
                            operations.next().expect("encoded operation")
                        }
                    })
                    .collect(),
            );
            trace["loopStart"] = json!(result.loop_start);
            let concrete =
                serde_json::from_value(trace.clone()).map_err(|error| error.to_string())?;
            match super::validation::verify_lasso(
                model,
                question,
                &concrete,
                result.loop_start,
                budget.work,
            ) {
                Ok(()) => {
                    answer.verdict = Verdict::Violated;
                    answer.lasso = Some(trace);
                }
                Err(LassoError::Invalid(reason)) => return Err(reason),
                Err(LassoError::Unverified(reason)) => answer.detail = Some(reason),
            }
        }
    }
    Ok(answer)
}

fn replay_trace(
    model: &rmodel::Model,
    mut lasso: Lasso,
    actions: &[rmodel::GroundAction],
) -> Result<Value, String> {
    let actions: BTreeMap<_, _> = actions
        .iter()
        .map(|action| (action.label.as_str(), action))
        .collect();
    let slots: BTreeMap<_, _> = model
        .relations
        .iter()
        .enumerate()
        .map(|(index, relation)| (relation.name.as_str(), index))
        .collect();
    let enum_members: BTreeMap<_, _> = model
        .enums
        .iter()
        .flat_map(|(sort, members)| {
            members
                .iter()
                .map(move |member| (member.as_str(), sort.as_str()))
        })
        .collect();
    for state in &mut lasso.trace {
        if state.values.len() != slots.len().max(1) {
            return Err("wrong number of relations in the lasso".into());
        }
        for value in &mut state.values {
            *value = normalized(std::mem::replace(value, Portable::Null))?;
        }
    }
    let entry = lasso
        .trace
        .get(lasso.loop_start)
        .ok_or("loopStart is outside the lasso")?;
    let initial = &lasso.trace[0];
    if initial.action.is_some() {
        return Err("initial lasso state has an incoming action".into());
    }
    if slots.is_empty() && initial.values != [Portable::Bool(false)] {
        return Err("unexpected initial value for the synthetic unit variable".into());
    }
    if lasso.trace.iter().any(|trace| {
        if slots.is_empty() {
            return trace.values != initial.values;
        }
        model
            .relations
            .iter()
            .enumerate()
            .any(|(index, relation)| !relation.var && trace.values[index] != initial.values[index])
    }) {
        return Err("lasso changes a static relation".into());
    }
    let matches = |trace: &TraceState, name: &str, tuples: Vec<Vec<Data>>| {
        let value = Portable::Set(
            tuples
                .into_iter()
                .map(|tuple| {
                    let mut items = tuple
                        .into_iter()
                        .map(|data| lower_value(&enum_members, data));
                    let first = items.next().expect("relations have positive arity");
                    match items.next() {
                        Some(second) => Portable::List(vec![first, second]),
                        None => first,
                    }
                })
                .collect(),
        );
        normalized(value).is_ok_and(|value| value == trace.values[slots[name]])
    };
    let machine = Machine::new(model)?;
    let config = machine
        .match_configuration(|name, tuples| matches(initial, name, tuples))
        .ok_or("initial lasso configuration failed reference validation")?;
    let mut state = machine
        .match_initial(&config, |name, tuples| matches(initial, name, tuples))
        .ok_or("initial lasso state failed reference validation")?;
    let initial_json = relations(model, machine.state_data(&state));
    let mut commands = Vec::with_capacity(lasso.trace.len());
    let mut previous = initial;
    let mut loop_state = state.clone();
    for (index, (label, target)) in lasso
        .trace
        .iter()
        .skip(1)
        .map(|target| (target.action.as_deref(), target))
        .chain(std::iter::once((Some(lasso.loop_action.as_str()), entry)))
        .enumerate()
    {
        if index == lasso.loop_start {
            loop_state = state.clone();
        }
        let label = label.ok_or("lasso edge has no action")?;
        if is_stutter(label) {
            if previous.values != target.values {
                return Err(format!("stuttering edge {} changes the state", index + 1));
            }
            commands.push(Value::Null);
        } else {
            let call = actions
                .get(label)
                .ok_or_else(|| format!("unknown lasso action {label}"))?;
            let Some(Some(Next::State(next))) =
                machine.match_step(&config, &state, &call.action, |name, value| {
                    call.args
                        .iter()
                        .any(|(param, arg)| param == name && *arg == value)
                })
            else {
                return Err(format!(
                    "no reference step for {label} at lasso edge {}",
                    index + 1
                ));
            };
            if !machine
                .state_data(&next)
                .into_iter()
                .all(|(name, tuples)| matches(target, &name, tuples))
            {
                return Err(format!(
                    "reference state differs for {label} at lasso edge {}",
                    index + 1
                ));
            }
            commands.push(command(&call.action, &call.args));
            state = next;
        }
        previous = target;
    }
    if state != loop_state {
        return Err("last reference step does not close the loop".into());
    }
    Ok(json!({
        "config": relations(model, machine.configuration_data(&config)),
        "initial": initial_json, "commands": commands, "loopStart": lasso.loop_start,
        "finalState": relations(model, machine.state_data(&state)),
    }))
}

fn is_stutter(label: &str) -> bool {
    label == "<stuttering>"
        || label == "Next"
        || label
            .strip_prefix("Next#")
            .is_some_and(|index| index.parse::<usize>().is_ok())
}

fn normalized(value: Portable) -> Result<Portable, String> {
    value.normalize().map_err(|error| error.to_string())
}

// Mirror rmodel's TLA+ value representation, before checking each step
// independently in the reference machine.
fn lower_value(enums: &BTreeMap<&str, &str>, data: Data) -> Portable {
    match data {
        Data::Atom(name) => Portable::String(match enums.get(name.as_str()) {
            Some(sort) => format!("{sort}.{name}"),
            None => name,
        }),
        Data::Int(value) => Portable::Int(value.to_string()),
        Data::Record(_, fields) => Portable::Record(
            fields
                .into_iter()
                .map(|(name, value)| (format!("f_{name}"), lower_value(enums, value)))
                .collect(),
        ),
        Data::Sequence(items) => Portable::List(
            items
                .into_iter()
                .map(|value| lower_value(enums, value))
                .collect(),
        ),
        Data::Variant(tag, value) => Portable::Record(vec![
            ("tag".into(), Portable::String(tag)),
            (
                "value".into(),
                value.map_or_else(
                    || Portable::String(String::new()),
                    |value| lower_value(enums, *value),
                ),
            ),
        ]),
    }
}
