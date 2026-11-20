use rtlc::{CoreError, SafetyBounds, SafetyProgram};
use serde_json::{Value, json};

fn program(depth: u32) -> Value {
    json!({
        "format":"modelt.safety-program", "version":2, "name":"RecursiveActions",
        "variables":["x"], "assumptions":[], "invariants":[], "checkDeadlock":false,
        "initial":[{"name":"Init","steps":[{"kind":"assign","slot":0,
            "value":{"op":"literal","value":{"type":"int","value":"0"}}}]}],
        "next":[{"name":"Down","steps":[{"kind":"call","definition":0,
            "arguments":[{"op":"literal","value":{"type":"int","value":depth.to_string()}}],"captures":[]}]}],
        "definitions":[{"id":0,"recursive":true,"parameterCount":1,"captureCount":0,
            "localScope":0,"bodyKind":"action","body":[
                {"name":"Base","steps":[
                    {"kind":"guard","expression":{"op":"eq","left":{"op":"parameter","slot":0},
                        "right":{"op":"literal","value":{"type":"int","value":"0"}}}},
                    {"kind":"assign","slot":0,"value":{"op":"mod","left":{"op":"add",
                        "left":{"op":"read","slot":0,"next":false},"right":{"op":"literal","value":{"type":"int","value":"1"}}},
                        "right":{"op":"literal","value":{"type":"int","value":"3"}}}}
                ]},
                {"name":"Recur","steps":[
                    {"kind":"guard","expression":{"op":"gt","left":{"op":"parameter","slot":0},
                        "right":{"op":"literal","value":{"type":"int","value":"0"}}}},
                    {"kind":"call","definition":0,"arguments":[{"op":"sub","left":{"op":"parameter","slot":0},
                        "right":{"op":"literal","value":{"type":"int","value":"1"}}}],"captures":[]}
                ]}
            ]}]
    })
}

fn bounds(work: u32) -> SafetyBounds {
    SafetyBounds {
        max_depth: 100,
        max_states: 1000,
        max_work: work,
        max_temporal_nodes: None,
    }
}

#[test]
fn marked_recursive_action_generates_successors_and_keeps_the_outer_action_name() {
    let program: SafetyProgram = serde_json::from_value(program(4)).unwrap();
    let result = program.check(bounds(1_000_000)).unwrap();
    assert_eq!(result.status, "complete");
    assert_eq!(result.states, 3);
    assert_eq!(result.generated, 4);
}

#[test]
fn action_recursion_and_work_exhaustion_are_inconclusive() {
    let mut wire = program(4);
    wire["definitions"][0]["body"] = json!([{"name":"Loop","steps":[
        {"kind":"call","definition":0,"arguments":[{"op":"parameter","slot":0}],"captures":[]}
    ]}]);
    let program: SafetyProgram = serde_json::from_value(wire).unwrap();
    let result = program.check(bounds(1_000_000)).unwrap();
    assert_eq!(result.status, "bounded");
    assert_eq!(result.reason.as_deref(), Some("evaluation"));
    assert!(result.violation.is_none());
    let result = program.check(bounds(7)).unwrap();
    assert_eq!(result.status, "bounded");
    assert_eq!(result.reason.as_deref(), Some("work"));
    assert_eq!(result.work, 7);
}

#[test]
fn recursive_action_does_not_accept_an_incomplete_successor() {
    let mut wire = program(2);
    wire["variables"] = json!(["x", "y"]);
    wire["initial"][0]["steps"]
        .as_array_mut()
        .unwrap()
        .push(json!({"kind":"assign","slot":1,
            "value":{"op":"literal","value":{"type":"int","value":"0"}}}));
    let program: SafetyProgram = serde_json::from_value(wire).unwrap();
    let error = program.check(bounds(1_000_000)).unwrap_err();
    assert_eq!(error.code, "protocol");
    assert!(
        error.message.contains("Incomplete safety state in action ")
            && error.message.ends_with("unassigned: y"),
        "{error}"
    );
}

#[test]
fn action_calls_reject_wrong_kind_arity_ids_and_unmarked_cycles_even_in_unreached_branches() {
    for fault in ["kind", "arity", "id", "cycle", "slot"] {
        let mut wire = program(1);
        match fault {
            "kind" => {
                wire["definitions"][0]["bodyKind"] = json!("expression");
                wire["definitions"][0]["body"] =
                    json!({"op":"literal","value":{"type":"bool","value":true}});
            }
            "arity" => wire["next"][0]["steps"][0]["arguments"] = json!([]),
            "id" => wire["next"][0]["steps"][0]["definition"] = json!(9),
            "cycle" => wire["definitions"][0]["recursive"] = json!(false),
            "slot" => wire["definitions"][0]["body"][0]["steps"][1]["slot"] = json!(1),
            _ => unreachable!(),
        }
        let program: SafetyProgram = serde_json::from_value(wire).unwrap();
        let error: CoreError = program.check(bounds(1_000_000)).unwrap_err();
        assert_eq!(error.code, "protocol", "{fault}");
    }
}
