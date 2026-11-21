#![cfg(not(target_arch = "wasm32"))]

use rtlc::{SafetyBounds, SafetyProgram};
use serde_json::{Value, json};

fn integer(value: i32) -> Value {
    json!({"op":"literal","value":{"type":"int","value":value.to_string()}})
}

fn grid() -> Value {
    let read = |slot| json!({"op":"read","slot":slot,"next":false});
    json!({
        "format":"modelt.safety-program","version":1,"name":"Grid","variables":["a","b"],
        "initial":[{"name":"Init","steps":[
            {"kind":"assign","slot":0,"value":integer(0)},
            {"kind":"assign","slot":1,"value":integer(0)}
        ]}],
        "next":(0..2).map(|slot| json!({"name":format!("Step{slot}"),"steps":[
            {"kind":"assign","slot":slot,"value":{"op":"if",
                "condition":{"op":"lt","left":read(slot),"right":integer(3)},
                "consequent":{"op":"add","left":read(slot),"right":integer(1)},
                "alternative":integer(0)}},
            {"kind":"assign","slot":1-slot,"value":read(1-slot)}
        ]})).collect::<Vec<_>>(),
        "invariants":[{"name":"Safe","expression":{"op":"le","left":read(0),"right":integer(3)}}],
        "assumptions":[],"checkDeadlock":true
    })
}

fn bounds() -> SafetyBounds {
    SafetyBounds {
        max_depth: 100,
        max_states: 100,
        max_work: 100_000,
        max_temporal_nodes: None,
    }
}

#[test]
fn parallel_diamond_deduplicates_and_keeps_bfs_counts() {
    let program: SafetyProgram = serde_json::from_value(grid()).unwrap();
    let expected = program.check(bounds()).unwrap();
    assert_eq!(expected.states, 16);
    assert_eq!(expected.generated, 33);
    for workers in [1, 2, 4, 8] {
        let actual = program.check_parallel(bounds(), workers).unwrap();
        assert_eq!(actual.status, expected.status);
        assert_eq!(actual.states, expected.states);
        assert_eq!(actual.generated, expected.generated);
        assert_eq!(actual.depth, expected.depth);
        assert_eq!(actual.violation, expected.violation);
        assert!(actual.work <= bounds().max_work);
    }
}

#[test]
fn parallel_constraints_keep_boundary_violations_and_counts() {
    let mut input = grid();
    for branch in input["next"].as_array_mut().unwrap() {
        branch["steps"][0]["value"]["condition"]["right"] = integer(31);
        branch["steps"].as_array_mut().unwrap().insert(
            0,
            json!({"kind":"bind","slot":0,"domain":{"kind":"set","values":[integer(0)]}}),
        );
    }
    input["invariants"][0]["expression"] = json!({"op":"le","left":{"op":"cardinality","argument":{"op":"set","values":[{"op":"read","slot":0,"next":false},{"op":"read","slot":1,"next":false}]}},"right":integer(2)});
    input["constraints"] = json!([{"name":"Limit","expression":{"op":"lt","left":{"op":"add","left":{"op":"read","slot":0,"next":false},"right":{"op":"read","slot":1,"next":false}},"right":integer(20)}}]);
    let limits = SafetyBounds {
        max_states: 2000,
        max_work: 10_000_000,
        ..bounds()
    };
    let program: SafetyProgram = serde_json::from_value(input.clone()).unwrap();
    let expected = program.check(limits).unwrap();
    assert_eq!(expected.status, "complete");
    assert_eq!(expected.states, 210);
    assert_eq!(expected.generated, 421);
    for workers in [2, 4, 8] {
        let actual = program.check_parallel(limits, workers).unwrap();
        assert_eq!(
            (actual.status, actual.states, actual.generated),
            (expected.status.clone(), 210, 421)
        );
    }
    input["invariants"][0]["expression"] = input["constraints"][0]["expression"].clone();
    let program: SafetyProgram = serde_json::from_value(input).unwrap();
    let expected = program.check(limits).unwrap();
    assert_eq!(expected.status, "invariant-violation");
    assert_eq!(expected.violation.as_ref().unwrap().trace.len(), 21);
    for workers in [2, 4, 8] {
        assert_eq!(
            program.check_parallel(limits, workers).unwrap().violation,
            expected.violation
        );
    }
}

#[test]
fn parallel_trace_is_the_same_shortest_trace() {
    let mut input = grid();
    input["invariants"][0]["expression"]["right"] = integer(1);
    let program: SafetyProgram = serde_json::from_value(input).unwrap();
    let expected = program.check(bounds()).unwrap();
    assert_eq!(expected.violation.as_ref().unwrap().trace.len(), 3);
    for workers in [2, 4, 8] {
        for _ in 0..10 {
            let actual = program.check_parallel(bounds(), workers).unwrap();
            assert_eq!(actual.violation, expected.violation);
            assert_eq!(actual.states, expected.states);
            assert_eq!(actual.generated, expected.generated);
        }
    }
}

#[test]
fn global_cuts_remain_bounded_and_worker_zero_is_rejected() {
    let program: SafetyProgram = serde_json::from_value(grid()).unwrap();
    assert!(program.check_parallel(bounds(), 0).is_err());
    for workers in [2, 4, 8] {
        let exact = program
            .check_parallel(
                SafetyBounds {
                    max_work: program.check(bounds()).unwrap().work,
                    ..bounds()
                },
                workers,
            )
            .unwrap();
        assert_eq!(exact.status, "complete");
        assert_eq!(exact.states, 16);
        assert_eq!(exact.generated, 33);
        for max_work in 1..program.check(bounds()).unwrap().work {
            let actual = program
                .check_parallel(
                    SafetyBounds {
                        max_work,
                        ..bounds()
                    },
                    workers,
                )
                .unwrap();
            assert_eq!(
                actual.status, "bounded",
                "workers={workers}, maxWork={max_work}, result={actual:?}"
            );
            assert_eq!(actual.reason.as_deref(), Some("work"));
            assert!(actual.work <= max_work);
        }
        for limited in [
            SafetyBounds {
                max_states: 3,
                ..bounds()
            },
            SafetyBounds {
                max_depth: 1,
                ..bounds()
            },
        ] {
            let actual = program.check_parallel(limited, workers).unwrap();
            let expected = program.check(limited).unwrap();
            assert_eq!(actual.status, expected.status);
            assert_eq!(actual.reason, expected.reason);
            assert_eq!(actual.states, expected.states);
            assert_eq!(actual.generated, expected.generated);
        }
    }
}

#[test]
fn converging_workers_validate_each_canonical_state_once() {
    let mut input = grid();
    for action in input["next"].as_array_mut().unwrap() {
        action["steps"][0]["value"]["condition"]["right"] = integer(31);
    }
    input["invariants"][0]["expression"] = json!({"op":"and", "left":{"op":"le", "left":{"op":"read","slot":0,"next":false},"right":integer(31)}, "right":{"op":"forall","bounds":[{"slot":0,"domain":{"kind":"range","lower":integer(0),"upper":integer(9)}}],"body":{"op":"lt","left":{"op":"bound","slot":0},"right":integer(10)}}});
    let program: SafetyProgram = serde_json::from_value(input).unwrap();
    let limits = SafetyBounds {
        max_states: 2000,
        max_work: 1_000_000,
        ..bounds()
    };
    let expected = program.check(limits).unwrap();
    assert_eq!(expected.states, 1024);
    for workers in [2, 4, 8] {
        let actual = program.check_parallel(limits, workers).unwrap();
        assert_eq!(actual.status, "complete");
        assert_eq!(actual.states, expected.states);
        assert_eq!(actual.generated, expected.generated);
        assert_eq!(
            actual.work, expected.work,
            "invariants must be checked once per canonical state, workers={workers}"
        );
    }
}

fn wide_batch() -> SafetyProgram {
    serde_json::from_value(json!({
        "format":"modelt.safety-program","version":1,"name":"WideBatch","variables":["x"],
        "initial":[{"name":"Init","steps":[
            {"kind":"choose","slot":0,"domain":{"kind":"range","lower":integer(0),"upper":integer(127)}}
        ]}],
        "next":[{"name":"Step","steps":[
            {"kind":"bind","slot":0,"domain":{"kind":"range","lower":integer(0),"upper":integer(31)}},
            {"kind":"assign","slot":0,"value":{"op":"if","condition":{"op":"lt","left":{"op":"read","slot":0,"next":false},"right":integer(256)},"consequent":{"op":"add","left":{"op":"read","slot":0,"next":false},"right":integer(128)},"alternative":{"op":"read","slot":0,"next":false}}}
        ]}],
        "invariants":[],"assumptions":[],"checkDeadlock":true
    })).unwrap()
}

#[test]
fn wide_duplicate_batches_keep_results_constraints_and_shortest_traces() {
    let limits = SafetyBounds {
        max_states: 1000,
        max_work: 1_000_000,
        ..bounds()
    };
    let mut program = wide_batch();
    let expected = program.check(limits).unwrap();
    assert_eq!((expected.states, expected.generated), (384, 12416));
    for workers in [2, 4, 8] {
        assert_eq!(program.check_parallel(limits, workers).unwrap(), expected);
    }
    program.constraints = serde_json::from_value(json!([{"name":"Limit","expression":{"op":"le","left":{"op":"read","slot":0,"next":false},"right":integer(257)}}])).unwrap();
    let expected = program.check(limits).unwrap();
    assert_eq!(expected.states, 258);
    for workers in [2, 4, 8] {
        assert_eq!(program.check_parallel(limits, workers).unwrap(), expected);
    }
    program.invariants = serde_json::from_value(json!([{"name":"Safe","expression":{"op":"lt","left":{"op":"read","slot":0,"next":false},"right":integer(256)}}])).unwrap();
    let expected = program.check(limits).unwrap();
    assert_eq!(expected.violation.as_ref().unwrap().trace.len(), 3);
    for workers in [2, 4, 8] {
        let actual = program.check_parallel(limits, workers).unwrap();
        assert_eq!(
            (
                actual.status,
                actual.states,
                actual.generated,
                actual.violation
            ),
            (
                expected.status.clone(),
                expected.states,
                expected.generated,
                expected.violation.clone()
            )
        );
    }
}

#[test]
fn wide_batches_keep_budget_cuts_and_action_constraint_errors() {
    let limits = SafetyBounds {
        max_states: 1000,
        max_work: 1_000_000,
        ..bounds()
    };
    let mut program = wide_batch();
    for workers in [2, 4, 8] {
        let total = program.check_parallel(limits, workers).unwrap().work;
        for max_work in [1, 511, total / 2, total - 1] {
            let result = program
                .check_parallel(SafetyBounds { max_work, ..limits }, workers)
                .unwrap();
            assert_eq!(result.status, "bounded");
            assert_eq!(result.reason.as_deref(), Some("work"));
            assert!(result.work <= max_work);
        }
        let result = program
            .check_parallel(
                SafetyBounds {
                    max_states: 130,
                    ..limits
                },
                workers,
            )
            .unwrap();
        assert_eq!(result.status, "bounded");
        assert_eq!(result.reason.as_deref(), Some("states"));
        assert_eq!(result.states, 130);
    }
    program.action_constraints = serde_json::from_value(json!([{"name":"Invalid","expression":{"op":"lt","left":{"op":"literal","value":{"type":"bool","value":true}},"right":integer(1)}}])).unwrap();
    let expected = program.check(limits).unwrap_err();
    for workers in [2, 4, 8] {
        let actual = program.check_parallel(limits, workers).unwrap_err();
        assert_eq!(
            (actual.code, actual.message),
            (expected.code, expected.message.clone())
        );
    }
}
