use roracle::{dispatch, dispatch_value, dispatch_value_to_json};
use serde_json::{Value, json};

fn request(reference: Value, actual: Value) -> Value {
    json!({"version":1,"group":"value-bridge","reference":"reference","implementations":["target"],
        "cases":[{"id":"case","input":{"kept":[null,"雪",7]},"reference":reference,"actual":[actual]}]})
}

#[test]
fn value_and_bytes_keep_every_report_and_status() {
    let observations = [
        json!({"kind":"value","value":null}),
        json!({"kind":"value","value":{"nested":[1,true,"雪"]}}),
        json!({"kind":"allowed","values":[null,7]}),
        json!({"kind":"allowed","values":[]}),
        json!({"kind":"unknown","reason":"work"}),
        json!({"kind":"invalid","reason":"scope"}),
        json!({"kind":"error","message":"checker"}),
    ];
    for reference in &observations {
        for actual in &observations {
            let input = request(reference.clone(), actual.clone());
            let expected = dispatch(&serde_json::to_vec(&input).unwrap());
            assert_eq!(dispatch_value(input.clone()), expected);
            let direct: Value = serde_json::from_str(&dispatch_value_to_json(input)).unwrap();
            assert_eq!(direct, expected);
        }
    }
}

#[test]
fn malformed_values_remain_errors() {
    for input in [
        json!(null),
        json!({}),
        request(json!({"kind":"value"}), json!({"kind":"value","value":7})),
    ] {
        let expected = dispatch_value(input.clone());
        assert_eq!(expected["kind"], "error");
        let direct: Value = serde_json::from_str(&dispatch_value_to_json(input.clone())).unwrap();
        assert_eq!(direct, expected);
        assert_eq!(
            dispatch(&serde_json::to_vec(&input).unwrap())["kind"],
            "error"
        );
    }
}

#[test]
fn value_bridge_enforces_the_canonical_size_limit() {
    let input = request(
        json!({"kind":"value","value":7}),
        json!({"kind":"value","value":7}),
    );
    let mut too_large = input;
    too_large["cases"][0]["input"] = json!("x".repeat(16 * 1024 * 1024));
    assert_eq!(dispatch_value(too_large)["kind"], "error");
}

#[test]
fn value_bridge_rejects_depth_beyond_the_byte_parser() {
    let mut nested = json!(null);
    for _ in 0..140 {
        nested = Value::Array(vec![nested]);
    }
    let mut input = request(
        json!({"kind":"value","value":7}),
        json!({"kind":"value","value":7}),
    );
    input["cases"][0]["input"] = nested;
    assert_eq!(dispatch_value(input)["kind"], "error");
}

#[test]
fn deeply_nested_values_are_discarded_without_a_recursive_drop() {
    let mut value = json!(null);
    for _ in 0..4096 {
        value = Value::Array(vec![value]);
    }
    assert_eq!(dispatch_value(value)["kind"], "error");
}
