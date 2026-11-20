use rtlc::{SafetyBounds, SafetyProgram};
use serde_json::{Value, json};

fn program() -> Value {
    json!({"format":"modelt.safety-program","version":2,"name":"TemporalCalls","variables":["x"],"assumptions":[],"invariants":[],"checkDeadlock":false,
      "initial":[{"name":"Init","steps":[{"kind":"assign","slot":0,"value":{"op":"literal","value":{"type":"bool","value":true}}}]}],
      "next":[{"name":"Next","steps":[{"kind":"assign","slot":0,"value":{"op":"read","slot":0,"next":false}}]}],
      "properties":[{"name":"Live","formula":{"kind":"call","definition":0,"arguments":[{"op":"read","slot":0,"next":false}],"captures":[]}}],
      "definitions":[{"id":0,"parameterCount":1,"captureCount":0,"localScope":0,"bodyKind":"temporal","body":{"kind":"always","formula":{"kind":"atom","expression":{"op":"parameter","slot":0}}}}]})
}

#[test]
fn temporal_call_wire_checks_kind_arity_cycles_and_v1() {
    let bounds = SafetyBounds {
        max_depth: 10,
        max_states: 100,
        max_work: 10000,
        max_temporal_nodes: Some(1000),
    };
    let wire = program();
    let program: SafetyProgram = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(program.check(bounds).unwrap().status, "complete");
    for fault in ["arity", "kind", "cycle", "v1", "expression-kind"] {
        let mut invalid = wire.clone();
        match fault {
            "arity" => invalid["properties"][0]["formula"]["arguments"] = json!([]),
            "kind" => {
                invalid["next"][0]["steps"] = json!([{"kind":"call","definition":0,"arguments":[{"op":"literal","value":{"type":"bool","value":true}}],"captures":[]}])
            }
            "cycle" => {
                invalid["definitions"][0]["recursive"] = json!(true);
                invalid["definitions"][0]["body"] = invalid["properties"][0]["formula"].clone();
            }
            "v1" => invalid["version"] = json!(1),
            "expression-kind" => {
                invalid["definitions"][0]["bodyKind"] = json!("expression");
                invalid["definitions"][0]["body"] =
                    json!({"op":"literal","value":{"type":"bool","value":true}});
            }
            _ => unreachable!(),
        }
        if fault == "v1" {
            assert!(serde_json::from_value::<SafetyProgram>(invalid).is_err());
            continue;
        }
        let invalid: SafetyProgram = serde_json::from_value(invalid).unwrap();
        assert_eq!(
            invalid.check(bounds).unwrap_err().code,
            "protocol",
            "{fault}"
        );
    }
}
