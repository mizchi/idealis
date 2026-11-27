//! The finite rmodel replay profile of ITF, shared with the example recorder.

use super::{Validator, canonical};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

const VARIABLES: [&str; 3] = ["config", "command", "observation"];

type Record = Map<String, Value>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recording {
    config: Record,
    initial: Record,
    commands: Vec<Record>,
    observations: Vec<Record>,
}

fn check_length(commands: usize, observations: usize) -> Result<(), String> {
    if observations == 0 || observations - 1 > commands {
        return Err("invalid ITF replay length".into());
    }
    Ok(())
}

fn fields(record: &Record, convert: fn(&Value) -> Result<Value, String>) -> Result<Record, String> {
    record
        .iter()
        .map(|(name, value)| {
            if name.starts_with('#') {
                return Err(format!("unsupported ITF field {name}"));
            }
            Ok((name.clone(), convert(value)?))
        })
        .collect()
}

fn encode(value: &Value) -> Result<Value, String> {
    Ok(match value {
        Value::Null => json!({"#tup":[]}),
        Value::Number(number) => {
            let integer = number
                .as_i64()
                .ok_or("ITF replay expects a signed 64-bit integer")?;
            json!({"#bigint":integer.to_string()})
        }
        Value::Array(items) => Value::Array(items.iter().map(encode).collect::<Result<_, _>>()?),
        Value::Object(record) => Value::Object(fields(record, encode)?),
        other => other.clone(),
    })
}

fn decode(value: &Value) -> Result<Value, String> {
    Ok(match value {
        Value::Object(record) => {
            if record.len() == 1
                && let Some(value) = record.get("#bigint")
            {
                let decimal = value.as_str().ok_or("invalid ITF replay integer")?;
                let integer = decimal
                    .parse::<i64>()
                    .ok()
                    .filter(|integer| integer.to_string() == decimal)
                    .ok_or("invalid ITF replay integer")?;
                return Ok(json!(integer));
            }
            if record.len() == 1
                && record
                    .get("#tup")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
            {
                return Ok(Value::Null);
            }
            Value::Object(fields(record, decode)?)
        }
        Value::Array(items) => Value::Array(items.iter().map(decode).collect::<Result<_, _>>()?),
        Value::Bool(_) | Value::String(_) => value.clone(),
        _ => return Err("unsupported ITF replay value".into()),
    })
}

fn record(value: &Value) -> Result<&Record, String> {
    value
        .as_object()
        .ok_or_else(|| "expected a JSON record in the ITF replay".into())
}

/// Encode observations and their planned command sequence without running an implementation.
pub fn record_itf(source: &str, recording: &Recording) -> Result<Value, String> {
    check_length(recording.commands.len(), recording.observations.len())?;
    let config = fields(&recording.config, encode)?;
    let commands = recording
        .commands
        .iter()
        .map(|command| fields(command, encode).map(Value::Object))
        .collect::<Result<Vec<_>, _>>()?;
    let init = json!({"kind":"init"});
    let states = recording
        .observations
        .iter()
        .enumerate()
        .map(|(index, observation)| {
            let command = index
                .checked_sub(1)
                .map_or(&init, |previous| &commands[previous]);
            Ok::<_, String>(json!({
                "#meta":{"index":index}, "config":config, "command":command,
                "observation":fields(observation, encode)?,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "#meta":{
            "format":"ITF", "source":source,
            "rmodelReplay":{"version":1, "initial": fields(&recording.initial, encode)?, "commands":commands},
        },
        "vars":VARIABLES,
        "states":states,
    }))
}

fn parse_trace(trace: &Value) -> Result<Recording, String> {
    let metadata = &trace["#meta"];
    let replay = &metadata["rmodelReplay"];
    if metadata["format"] != "ITF"
        || !metadata["source"].is_string()
        || replay["version"] != 1
        || trace.get("loop").is_some()
        || trace["vars"] != json!(VARIABLES)
    {
        return Err("expected a finite rmodel ITF replay".into());
    }
    let states = trace["states"]
        .as_array()
        .ok_or("missing ITF replay states")?;
    let commands = replay["commands"]
        .as_array()
        .ok_or("missing ITF replay commands")?;
    check_length(commands.len(), states.len())?;
    let commands = commands
        .iter()
        .map(|value| fields(record(value)?, decode))
        .collect::<Result<Vec<_>, _>>()?;
    let config = fields(record(&states[0]["config"])?, decode)?;
    let initial = fields(record(&replay["initial"])?, decode)?;
    let init = Record::from_iter([("kind".to_owned(), json!("init"))]);
    let observations = states
        .iter()
        .enumerate()
        .map(|(index, state)| {
            let command = index
                .checked_sub(1)
                .map_or(&init, |previous| &commands[previous]);
            if state["#meta"]["index"] != index
                || fields(record(&state["config"])?, decode)? != config
                || fields(record(&state["command"])?, decode)? != *command
            {
                return Err(format!("inconsistent ITF replay frame {index}"));
            }
            fields(record(&state["observation"])?, decode)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Recording {
        config,
        initial,
        commands,
        observations,
    })
}

#[derive(Serialize)]
#[serde(tag = "verdict", rename_all = "lowercase")]
pub enum ReplayResult {
    Matched { steps: usize },
    Mismatch { difference: Difference },
}

#[derive(Serialize)]
pub struct Difference {
    step: usize,
    command: Option<Value>,
    expected: Value,
    observed: Value,
}

/// Recheck the recorded prefix against one model, with no connection to its implementation.
pub fn replay_itf(model: &rmodel::Model, trace: &Value) -> Result<ReplayResult, String> {
    let Recording {
        config,
        initial,
        commands,
        observations,
    } = parse_trace(trace)?;
    let validator = Validator::new(model)?;
    let mut session = validator
        .start(&Value::Object(config), &Value::Object(initial))
        .map_err(|error| format!("step 0: {error}"))?;
    let steps = commands.len();
    let recorded_steps = observations.len() - 1;
    let requests = std::iter::once(None).chain(
        commands
            .into_iter()
            .map(|command| Some(Value::Object(command))),
    );
    for (step, (command, observed)) in requests.zip(observations).enumerate() {
        if let Some(command) = &command {
            session
                .replay(command)
                .map_err(|error| format!("step {step}: {error}"))?;
        }
        let expected = session.snapshot();
        let observed = Value::Object(observed);
        if expected != canonical(&observed) {
            return Ok(ReplayResult::Mismatch {
                difference: Difference {
                    step,
                    command,
                    expected,
                    observed,
                },
            });
        }
    }
    if recorded_steps != steps {
        return Err(format!(
            "missing observation at step {}",
            recorded_steps + 1
        ));
    }
    Ok(ReplayResult::Matched { steps })
}
