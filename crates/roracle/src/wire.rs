use crate::wire_observation::WireObservation;
use crate::{Case, CaseReport, CheckReport, Error, Observation, OracleGroup};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(rename = "OracleRequest"))]
pub(crate) struct Request {
    version: u32,
    group: String,
    reference: String,
    implementations: Vec<String>,
    cases: Vec<Input>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(rename = "OracleInput"))]
pub(crate) struct Input {
    id: String,
    input: Value,
    reference: WireObservation,
    actual: Vec<WireObservation>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(rename = "OracleResponse"))]
pub(crate) enum Response<'a> {
    Checked {
        report: CheckReport<&'a Value, &'a Value>,
    },
    Error {
        message: String,
    },
}

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;

/// Evaluate an observed JSON batch with the same semantics as `OracleGroup::check`.
/// Callbacks and clocks remain on the host; this boundary does not time implementations.
pub fn dispatch(bytes: &[u8]) -> Value {
    emit_response(parse_bytes(bytes), response_value)
}

/// Encode the complete response for byte-oriented hosts without changing the JSON contract.
pub fn dispatch_json(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    dispatch_into(bytes, &mut output);
    output
}

fn valid_value_depth(input: &Value) -> bool {
    let mut pending = vec![(input, 0)];
    while let Some((value, depth)) = pending.pop() {
        if matches!(value, Value::Array(_) | Value::Object(_)) && depth + 1 >= 128 {
            return false;
        }
        match value {
            Value::Array(values) => pending.extend(values.iter().map(|value| (value, depth + 1))),
            Value::Object(values) => {
                pending.extend(values.values().map(|value| (value, depth + 1)))
            }
            _ => {}
        }
    }
    true
}

fn discard_value(input: Value) {
    let mut pending = vec![input];
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(values) => pending.extend(values),
            Value::Object(values) => pending.extend(values.into_values()),
            _ => {}
        }
    }
}

/// Evaluate canonical JSON data without a JSON text roundtrip.
/// The serialized size and container depth retain the byte API limits.
pub fn dispatch_value(input: Value) -> Value {
    emit_response(parse_value(input), response_value)
}

/// Encode a Value request's full response directly as JSON text.
pub fn dispatch_value_to_json(input: Value) -> String {
    emit_response(parse_value(input), |response| {
        serde_json::to_string(&response).expect("serializable oracle response")
    })
}

fn response_value(response: Response<'_>) -> Value {
    serde_json::to_value(response).expect("serializable oracle response")
}

fn parse_value(input: Value) -> Result<Request, Error> {
    if !valid_value_depth(&input) {
        discard_value(input);
        return Err(Error("JSON batch exceeds parser depth".into()));
    }
    struct SizeLimit(usize);
    impl std::io::Write for SizeLimit {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > MAX_INPUT_BYTES.saturating_sub(self.0) {
                return Err(std::io::Error::other("JSON batch exceeds 16 MiB"));
            }
            self.0 += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    if serde_json::to_writer(SizeLimit(0), &input).is_err() {
        return Err(Error("JSON batch exceeds 16 MiB".into()));
    }
    serde_json::from_value(input).map_err(|error| Error(error.to_string()))
}

pub(crate) fn dispatch_into(bytes: &[u8], output: &mut Vec<u8>) {
    output.clear();
    emit_response(parse_bytes(bytes), |response| {
        serde_json::to_writer(&mut *output, &response).expect("serializable oracle response");
    });
}

fn borrow(observation: &Observation<Value>) -> Observation<&Value> {
    match observation {
        Observation::Value { value } => Observation::Value { value },
        Observation::Allowed { values } => Observation::Allowed {
            values: values.iter().collect(),
        },
        Observation::Unknown { reason } => Observation::Unknown {
            reason: reason.clone(),
        },
        Observation::Invalid { reason } => Observation::Invalid {
            reason: reason.clone(),
        },
        Observation::Error { message } => Observation::Error {
            message: message.clone(),
        },
    }
}

fn parse_bytes(bytes: &[u8]) -> Result<Request, Error> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(Error("JSON batch exceeds 16 MiB".into()));
    }
    serde_json::from_slice(bytes).map_err(|error| Error(error.to_string()))
}

fn emit_response<R>(request: Result<Request, Error>, mut emit: impl FnMut(Response<'_>) -> R) -> R {
    request
        .and_then(|request| execute_request(request, |report| emit(Response::Checked { report })))
        .unwrap_or_else(|error| {
            emit(Response::Error {
                message: error.to_string(),
            })
        })
}

fn execute_request<R>(
    request: Request,
    emit: impl FnOnce(CheckReport<&Value, &Value>) -> R,
) -> Result<R, Error> {
    if request.version != 1
        || request.cases.len() > 4096
        || request.implementations.len() > 256
        || request
            .cases
            .iter()
            .any(|case| case.actual.len() != request.implementations.len())
    {
        return Err(Error(
            "invalid version, batch dimensions or observation count".into(),
        ));
    }
    let mut group = OracleGroup::new(request.group);
    for (index, case) in request.cases.iter().enumerate() {
        group.case(&case.id, index);
    }
    group.reference(request.reference, |index| {
        borrow(&request.cases[*index].reference.0)
    });
    for (implementation, id) in request.implementations.iter().enumerate() {
        let cases = &request.cases;
        group.test_function(id, move |index| {
            borrow(&cases[*index].actual[implementation].0)
        });
    }
    let report = group.check()?;
    Ok(emit(CheckReport {
        version: report.version,
        group: report.group,
        reference: report.reference,
        implementations: report.implementations,
        status: report.status,
        cases: report
            .cases
            .into_iter()
            .map(|case| CaseReport {
                case: Case {
                    id: case.case.id,
                    input: &request.cases[case.case.input].input,
                },
                reference: case.reference,
                comparisons: case.comparisons,
            })
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::dispatch_into;
    use serde_json::Value;

    #[test]
    fn reused_output_replaces_success_and_error_without_trailing_bytes() {
        let request = br#"{"version":1,"group":"reused","reference":"oracle","implementations":["target"],"cases":[{"id":"one","input":null,"reference":{"kind":"value","value":7},"actual":[{"kind":"value","value":7}]}]}"#;
        let mut output = Vec::new();
        for input in [request.as_slice(), b"invalid", request.as_slice()] {
            dispatch_into(input, &mut output);
            let response: Value = serde_json::from_slice(&output).unwrap();
            assert_eq!(response, super::dispatch(input));
            assert_eq!(
                response["kind"],
                if input == b"invalid" {
                    "error"
                } else {
                    "checked"
                }
            );
        }
    }
}
