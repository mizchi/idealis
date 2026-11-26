#![cfg(not(target_arch = "wasm32"))]

use rtlc::{SafetyBounds, SafetyProgram};
use serde_json::{Value, json};

const ACCEPTED_SETS: usize = 242;
const INITIAL_STATES: i32 = 32;
const PER_DEFINITION: usize = 30;

fn integer(value: i32) -> Value {
    json!({"op":"literal","value":{"type":"int","value":value.to_string()}})
}

fn nested_sets(size: usize) -> Vec<Value> {
    let count = size.div_ceil(PER_DEFINITION);
    (0..count)
        .map(|id| {
            let levels = if id + 1 == count {
                size - PER_DEFINITION * (count - 1)
            } else {
                PER_DEFINITION
            };
            let inner = if id + 1 == count {
                json!({"op":"set","values":[{"op":"literal","value":{"type":"bool","value":true}}]})
            } else {
                json!({"op":"call","definition":id + 1,"arguments":[],"captures":[]})
            };
            let body = (0..levels).fold(inner, |inner, _| {
                json!({"op":"sequence-set","argument":inner})
            });
            json!({"id":id,"parameterCount":0,"captureCount":0,"bodyKind":"expression","localScope":0,"body":body})
        })
        .collect()
}

fn model(size: usize) -> SafetyProgram {
    let counter = json!({"op":"read","slot":0,"next":false});
    serde_json::from_value(json!({
        "format":"modelt.safety-program","version":2,"name":"Sets","variables":["counter"],
        "initial":[{"name":"Init","steps":[
            {"kind":"choose","slot":0,"domain":{"kind":"range","lower":integer(0),"upper":integer(INITIAL_STATES - 1)}}
        ]}],
        "next":[{"name":"Step","steps":[
            {"kind":"guard","expression":{"op":"sequence-in",
                "left":{"op":"tuple","values":[]},
                "right":{"op":"call","definition":0,"arguments":[],"captures":[]}}},
            {"kind":"assign","slot":0,"value":counter.clone()}
        ]}],
        "invariants":[{"name":"Safe","expression":{"op":"ge","left":counter,"right":integer(0)}}],
        "assumptions":[],"checkDeadlock":false,
        "definitions":nested_sets(size)
    }))
    .unwrap()
}

fn bounds() -> SafetyBounds {
    SafetyBounds {
        max_depth: 8,
        max_states: 1000,
        max_work: 10_000_000,
        max_temporal_nodes: None,
    }
}

#[test]
fn a_worker_pool_evaluates_nested_sets_up_to_the_nesting_limit() {
    let result = model(ACCEPTED_SETS).check_parallel(bounds(), 2).unwrap();
    assert_eq!(result.status, "complete");
    assert_eq!(result.states, 32);
    assert_eq!(result.generated, 64);
    assert_eq!(result.work, 423);
}

#[test]
fn a_worker_pool_refuses_nested_sets_past_the_nesting_limit() {
    let error = model(ACCEPTED_SETS + 1)
        .check_parallel(bounds(), 2)
        .unwrap_err();
    assert_eq!(error.code, "protocol");
    assert_eq!(error.message, "Safety evaluation nesting exceeds 256");
}
