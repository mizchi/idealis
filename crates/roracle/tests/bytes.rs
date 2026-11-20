use roracle::{dispatch, dispatch_json};
use serde_json::{Value, json};

#[test]
fn byte_output_preserves_complete_reports_for_values_and_failure_observations() {
    for observation in [
        json!({"kind":"value","value":{"nested":[1,true,null,{"field":"x"}]}}),
        json!({"kind":"allowed","values":[1,2]}),
        json!({"kind":"unknown","reason":"budget"}),
        json!({"kind":"invalid","reason":"input"}),
        json!({"kind":"error","message":"crash"}),
    ] {
        let input = json!({"version":1,"group":"bytes","reference":"oracle","implementations":["target"],"cases":[{"id":"input","input":{"source":"retained"},"reference":observation,"actual":[{"kind":"value","value":2}]}]});
        let bytes = serde_json::to_vec(&input).unwrap();
        let output: Value = serde_json::from_slice(&dispatch_json(&bytes)).unwrap();
        assert_eq!(output, dispatch(&bytes));
        assert_eq!(
            output["report"]["cases"][0]["case"]["input"],
            json!({"source":"retained"})
        );
    }
}
#[test]
fn malformed_or_empty_batches_are_errors() {
    let invalid = dispatch_json(b"invalid");
    assert_eq!(
        serde_json::from_slice::<Value>(&invalid).unwrap(),
        dispatch(b"invalid")
    );
    let empty = dispatch_json(
        br#"{"version":1,"group":"empty","reference":"oracle","implementations":[],"cases":[]}"#,
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&empty).unwrap()["kind"],
        "error"
    );
}
