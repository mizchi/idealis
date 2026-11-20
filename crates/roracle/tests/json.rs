use roracle::dispatch;
use serde_json::{Value, json};
fn request() -> Value {
    json!({"version":1,"group":"wire","reference":"oracle","implementations":["native","wasm"],"cases":[
        {"id":"one","input":{"source":"model"},"reference":{"kind":"value","value":7},"actual":[{"kind":"value","value":7},{"kind":"value","value":8}]}
    ]})
}
#[test]
fn json_batches_use_the_same_group_and_preserve_inputs_and_failures() {
    let reply = dispatch(&serde_json::to_vec(&request()).unwrap());
    assert_eq!(reply["kind"], "checked");
    assert_eq!(reply["report"]["status"], "mismatch");
    assert_eq!(
        reply["report"]["cases"][0]["case"]["input"],
        json!({"source":"model"})
    );
    assert_eq!(
        reply["report"]["cases"][0]["comparisons"][0]["status"],
        "matched"
    );
    let mut input = request();
    input["cases"][0]["reference"] = json!({"kind":"unknown","reason":"budget"});
    let reply = dispatch(&serde_json::to_vec(&input).unwrap());
    assert_eq!(reply["report"]["status"], "unknown");
}
#[test]
fn malformed_or_vacuous_batches_cannot_match() {
    for path in ["version", "cases", "actual", "extra", "duplicate"] {
        let mut input = request();
        match path {
            "version" => input["version"] = json!(2),
            "cases" => input["cases"] = json!([]),
            "actual" => input["cases"][0]["actual"] = json!([]),
            "extra" => input["typo"] = json!(true),
            "duplicate" => input["implementations"] = json!(["native", "native"]),
            _ => unreachable!(),
        }
        assert_eq!(
            dispatch(&serde_json::to_vec(&input).unwrap())["kind"],
            "error",
            "{path}"
        );
    }
    assert_eq!(dispatch(b"not json")["kind"], "error");
}

#[test]
fn observations_require_exact_variant_fields_in_any_key_order() {
    for observation in [
        json!({"value":null,"kind":"value"}),
        json!({"values":[null,1],"kind":"allowed"}),
        json!({"reason":"budget","kind":"unknown"}),
        json!({"reason":"input","kind":"invalid"}),
        json!({"message":"crash","kind":"error"}),
    ] {
        let mut input = request();
        input["cases"][0]["reference"] = observation.clone();
        assert_eq!(
            dispatch(&serde_json::to_vec(&input).unwrap())["kind"],
            "checked"
        );
        for field in ["value", "values", "reason", "message", "extra"] {
            if observation.get(field).is_some() {
                continue;
            }
            let mut invalid = input.clone();
            invalid["cases"][0]["reference"][field] = json!(null);
            assert_eq!(
                dispatch(&serde_json::to_vec(&invalid).unwrap())["kind"],
                "error",
                "{invalid}"
            );
        }
    }
    for observation in [
        json!({"kind":"value"}),
        json!({"kind":"allowed"}),
        json!({"kind":"unknown","reason":null}),
        json!({"kind":"error","message":3}),
        json!({"kind":"allowed","values":null}),
        json!({"kind":"other","value":1}),
    ] {
        let mut input = request();
        input["cases"][0]["reference"] = observation;
        assert_eq!(
            dispatch(&serde_json::to_vec(&input).unwrap())["kind"],
            "error",
            "{input}"
        );
    }
}

#[test]
fn duplicate_observation_fields_are_rejected() {
    for observation in [
        r#"{"kind":"value","kind":"value","value":7}"#,
        r#"{"kind":"value","value":null,"value":7}"#,
        r#"{"kind":"allowed","values":[],"values":[]}"#,
        r#"{"kind":"unknown","reason":"a","reason":"b"}"#,
        r#"{"kind":"error","message":"a","message":"b"}"#,
    ] {
        let input = format!(
            r#"{{"version":1,"group":"duplicates","reference":"oracle","implementations":["target"],"cases":[{{"id":"one","input":null,"reference":{observation},"actual":[{{"kind":"value","value":7}}]}}]}}"#
        );
        assert_eq!(dispatch(input.as_bytes())["kind"], "error");
    }
}

#[test]
fn wire_observation_acceptance_matches_the_public_serde_contract() {
    let mut observations = vec![json!(null), json!(7), json!("value"), json!([])];
    for kind in [
        json!("value"),
        json!("allowed"),
        json!("unknown"),
        json!("invalid"),
        json!("error"),
        json!("other"),
        json!(0),
        json!(null),
    ] {
        for payload in [
            json!(null),
            json!(7),
            json!([]),
            json!([7]),
            json!("reason"),
            json!({"nested":[1,null]}),
            json!(1.25),
            json!(-0.0),
            json!(u64::MAX),
            json!(i64::MIN),
        ] {
            observations.push(json!([kind, payload]));
            observations.push(json!([kind, payload, 7]));
            for name in ["value", "values", "reason", "message", "extra"] {
                observations.push(json!({"kind":kind, (name):payload}));
            }
        }
    }
    for observation in observations {
        let expected = serde_json::from_slice::<roracle::Observation<Value>>(
            &serde_json::to_vec(&observation).unwrap(),
        )
        .ok()
        .map(|observation| serde_json::to_value(observation).unwrap());
        for field in ["reference", "actual"] {
            let mut input = request();
            if field == "reference" {
                input["cases"][0][field] = observation.clone();
            } else {
                input["cases"][0][field] = json!([observation, {"kind":"value","value":7}]);
            }
            let response = dispatch(&serde_json::to_vec(&input).unwrap());
            assert_eq!(response["kind"] == "checked", expected.is_some(), "{input}");
            if let Some(expected) = &expected {
                let observation = if field == "reference" {
                    &response["report"]["cases"][0]["reference"]
                } else {
                    &response["report"]["cases"][0]["comparisons"][0]["actual"]
                };
                assert_eq!(observation, expected, "{input}");
            }
        }
    }
}

#[test]
fn observation_tag_may_follow_its_payload() {
    let input = serde_json::to_string(&request()).unwrap().replace(
        r#"{"kind":"value","value":7}"#,
        r#"{"value":7,"kind":"value"}"#,
    );
    assert_eq!(
        dispatch(input.as_bytes()),
        dispatch(&serde_json::to_vec(&request()).unwrap())
    );
}
