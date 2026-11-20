use rtlc::{SafetyBounds, SafetyProgram};
use serde_json::{Value, json};

#[path = "../examples/counter.rs"]
mod counter;

fn bounds(max_work: u32) -> SafetyBounds {
    SafetyBounds {
        max_depth: 10,
        max_states: 100,
        max_work,
        max_temporal_nodes: None,
    }
}

fn program(expression: Value) -> SafetyProgram {
    let mut input = serde_json::to_value(counter::ring()).unwrap();
    input["invariants"][0]["expression"] = expression;
    serde_json::from_value(input).unwrap()
}

fn integer(value: i32) -> Value {
    json!({"op":"literal","value":{"type":"int","value":value.to_string()}})
}

#[test]
fn callbacks_reject_invalid_slots_even_in_unexecuted_branches() {
    let sequence = json!({"op":"tuple","values":[]});
    for expression in [
        json!({"op":"select-seq","slot":1,"sequence":sequence,"body":{"op":"literal","value":{"type":"bool","value":true}}}),
        json!({"op":"select-seq","slot":0,"sequence":{"op":"bound","slot":0},"body":{"op":"bound","slot":0}}),
        json!({"op":"select-seq","slot":0,"sequence":sequence,"body":{"op":"bound","slot":1}}),
        json!({"op":"sort-seq","slot":0,"sequence":sequence,"body":{"op":"bound","slot":2}}),
    ] {
        let input = program(
            json!({"op":"if","condition":{"op":"literal","value":{"type":"bool","value":true}},"consequent":{"op":"literal","value":{"type":"bool","value":true}},"alternative":expression}),
        );
        assert_eq!(input.check(bounds(1000)).unwrap_err().code, "protocol");
    }
}

#[test]
fn selection_and_every_insertion_comparison_are_bounded() {
    for operation in ["select-seq", "sort-seq"] {
        let values: Vec<_> = (0..128).map(integer).collect();
        let mut expected = values.clone();
        if operation == "sort-seq" {
            expected.reverse();
        }
        let input = program(
            json!({"op":"eq","left":{"op":operation,"slot":0,"sequence":{"op":"tuple","values":values},"body":{"op":"literal","value":{"type":"bool","value":true}}},"right":{"op":"tuple","values":expected}}),
        );
        let cut = input.check(bounds(100)).unwrap();
        assert_eq!(cut.status, "bounded");
        assert!(cut.work <= 100);
        assert_eq!(input.check(bounds(500_000)).unwrap().status, "complete");
    }
}

#[test]
fn callback_arity_counts_towards_the_bound_variable_limit() {
    for (operation, count) in [("select-seq", 40), ("sort-seq", 39)] {
        let bindings: Vec<_> = (0..count)
            .map(|slot| json!({"slot":slot,"domain":{"kind":"set","values":[]}}))
            .collect();
        let input = program(
            json!({"op":"forall","bounds":bindings,"body":{"op":operation,"slot":count,"sequence":{"op":"tuple","values":[]},"body":{"op":"literal","value":{"type":"bool","value":true}}}}),
        );
        assert_eq!(input.check(bounds(1000)).unwrap_err().code, "protocol");
    }
}

#[test]
fn assert_evaluates_its_message_before_testing_the_condition() {
    let input = program(
        json!({"op":"assert","condition":{"op":"literal","value":{"type":"bool","value":false}},"message":{"op":"data","value":{"type":"string","value":"diagnostic marker"}}}),
    );
    let error = input.check(bounds(1000)).unwrap_err();
    assert!(error.message.contains("diagnostic marker"), "{error}");
    for condition in [true, false] {
        let input = program(
            json!({"op":"assert","condition":{"op":"literal","value":{"type":"bool","value":condition}},"message":{"op":"div","left":integer(1),"right":integer(0)}}),
        );
        let error = input.check(bounds(1000)).unwrap_err();
        assert!(error.message.contains("zero"), "{error}");
    }
}

#[test]
fn v2_callbacks_validate_binder_slots_and_reach_shared_definitions() {
    for sort in [false, true] {
        let sequence = json!({"op":"tuple","values":[integer(2),integer(1)]});
        let predicate = if sort {
            json!({"op":"lt","left":{"op":"bound","slot":0},"right":{"op":"bound","slot":1}})
        } else {
            json!({"op":"gt","left":{"op":"bound","slot":0},"right":integer(1)})
        };
        let selected = json!({"op":if sort {"sort-seq"} else {"select-seq"},"slot":0,"sequence":sequence,"body":{"op":"call","definition":0,"arguments":[predicate],"captures":[]}});
        let expected = if sort {
            vec![integer(1), integer(2)]
        } else {
            vec![integer(2)]
        };
        let mut wire = serde_json::to_value(counter::ring()).unwrap();
        wire["invariants"][0]["expression"] =
            json!({"op":"eq","left":selected,"right":{"op":"tuple","values":expected}});
        wire["version"] = json!(2);
        wire["definitions"] = json!([{"id":0,"parameterCount":1,"captureCount":0,"bodyKind":"expression","localScope":0,"body":{"op":"parameter","slot":0}}]);
        let input: SafetyProgram = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(input.check(bounds(10000)).unwrap().status, "complete");
        wire["invariants"][0]["expression"]["left"]["slot"] = json!(1);
        let invalid: SafetyProgram = serde_json::from_value(wire).unwrap();
        assert_eq!(invalid.check(bounds(10000)).unwrap_err().code, "protocol");
    }
}

#[test]
fn deeply_nested_action_frames_are_rejected_and_dropped_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let mut input = counter::ring();
            input.version = 2;
            input.definitions = Some(vec![]);
            for _ in 0..10000 {
                input.next = vec![rtlc::ir::Branch {
                    name: "Nested".into(),
                    steps: vec![rtlc::ir::Step::Frame {
                        arguments: vec![],
                        captures: vec![],
                        parameter_count: 0,
                        capture_count: 0,
                        local_scope: 0,
                        branches: std::mem::take(&mut input.next),
                    }],
                }];
            }
            assert_eq!(input.check(bounds(1000)).unwrap_err().code, "protocol");
        })
        .unwrap()
        .join()
        .unwrap();
}
