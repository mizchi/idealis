#![cfg(not(target_arch = "wasm32"))]

use rtlc::{SafetyBounds, SafetyProgram};
use serde_json::{Value, json};

const ACCEPTED_SETS: usize = 242;
const ACCEPTED_FORWARDS: usize = 253;
const INITIAL_STATES: i32 = 32;
const PER_DEFINITION: usize = 30;
const REQUIRED_STACK: usize = if cfg!(debug_assertions) {
    8 * 1024 * 1024
} else {
    2 * 1024 * 1024
};
const NESTING_LIMIT: &str = "Safety evaluation nesting exceeds 256";

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

fn read(slot: usize) -> Value {
    json!({"op":"read","slot":slot,"next":false})
}

fn forwarding_definitions(size: usize, first: usize) -> Vec<Value> {
    (0..size)
        .map(|index| {
            let id = first + index;
            let body = if index + 1 == size {
                json!({"op":"parameter","slot":0})
            } else {
                json!({"op":"call","definition":id + 1,"arguments":[{"op":"parameter","slot":0}],"captures":[]})
            };
            json!({"id":id,"parameterCount":1,"captureCount":0,"bodyKind":"expression","localScope":0,"body":body})
        })
        .collect()
}

fn forwarded(first: usize, actual: Value) -> Value {
    json!({"op":"call","definition":first,"arguments":[actual],"captures":[]})
}

fn choose_range(upper: i32) -> Value {
    json!({"kind":"choose","slot":0,"domain":{"kind":"range","lower":integer(0),"upper":integer(upper)}})
}

fn forwarding_model(size: usize) -> SafetyProgram {
    serde_json::from_value(json!({
        "format":"modelt.safety-program","version":2,"name":"Forward","variables":["counter"],
        "initial":[{"name":"Init","steps":[choose_range(INITIAL_STATES - 1)]}],
        "next":[{"name":"Step","steps":[
            {"kind":"assign","slot":0,"value":forwarded(0, read(0))}
        ]}],
        "invariants":[{"name":"Safe","expression":{"op":"ge","left":read(0),"right":integer(0)}}],
        "assumptions":[],"checkDeadlock":false,
        "definitions":forwarding_definitions(size, 0)
    }))
    .unwrap()
}

fn on_required_stack<T: Send + 'static>(run: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(REQUIRED_STACK)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap()
}

#[test]
fn a_worker_pool_evaluates_forwarded_parameters_up_to_the_nesting_limit() {
    let parallel = forwarding_model(ACCEPTED_FORWARDS)
        .check_parallel(bounds(), 2)
        .unwrap();
    let sequential =
        on_required_stack(|| forwarding_model(ACCEPTED_FORWARDS).check(bounds())).unwrap();
    for result in [parallel, sequential] {
        assert_eq!(result.status, "complete");
        assert_eq!(result.states, 32);
        assert_eq!(result.generated, 64);
        assert_eq!(result.work, 16_289);
    }
}

#[test]
fn a_worker_pool_refuses_forwarded_parameters_past_the_nesting_limit() {
    let parallel = forwarding_model(ACCEPTED_FORWARDS + 1)
        .check_parallel(bounds(), 2)
        .unwrap_err();
    let sequential =
        on_required_stack(|| forwarding_model(ACCEPTED_FORWARDS + 1).check(bounds())).unwrap_err();
    for error in [parallel, sequential] {
        assert_eq!(error.code, "protocol");
        assert_eq!(error.message, NESTING_LIMIT);
    }
}

fn first_state_branches(
    definitions: Vec<Value>,
    upper: i32,
    consequent: Value,
    alternative: Value,
    invariant: Value,
) -> SafetyProgram {
    serde_json::from_value(json!({
        "format":"modelt.safety-program","version":2,"name":"Workers","variables":["x"],
        "initial":[{"name":"Init","steps":[choose_range(upper)]}],
        "next":[{"name":"Step","steps":[
            {"kind":"assign","slot":0,"value":{"op":"if",
                "condition":{"op":"eq","left":read(0),"right":integer(0)},
                "consequent":consequent,
                "alternative":alternative}}
        ]}],
        "invariants":[{"name":"Safe","expression":invariant}],
        "assumptions":[],"checkDeadlock":false,
        "definitions":definitions
    }))
    .unwrap()
}

fn nonterminating_first_state(forwards: usize) -> SafetyProgram {
    let mut definitions = forwarding_definitions(forwards, 0);
    definitions.push(json!({"id":forwards,"parameterCount":1,"captureCount":0,"bodyKind":"expression","localScope":0,"recursive":true,
        "body":{"op":"call","definition":forwards,"arguments":[{"op":"parameter","slot":0}],"captures":[]}}));
    first_state_branches(
        definitions,
        INITIAL_STATES - 1,
        json!({"op":"call","definition":forwards,"arguments":[read(0)],"captures":[]}),
        forwarded(0, read(0)),
        json!({"op":"ge","left":read(0),"right":integer(0)}),
    )
}

fn violating_first_state(first: usize, second: usize) -> SafetyProgram {
    let mut definitions = forwarding_definitions(first, 0);
    definitions.extend(forwarding_definitions(second, first));
    first_state_branches(
        definitions,
        15,
        json!({"op":"add","left":forwarded(0, read(0)),"right":integer(16)}),
        forwarded(first, read(0)),
        json!({"op":"lt","left":read(0),"right":integer(16)}),
    )
}

fn unbounded_work() -> SafetyBounds {
    SafetyBounds {
        max_work: 10_000_000,
        ..bounds()
    }
}

#[test]
fn a_worker_pool_counts_the_work_of_a_worker_that_no_longer_stops_at_forwarding() {
    let program = nonterminating_first_state(200);
    let result = program.check_parallel(unbounded_work(), 2).unwrap();
    assert_eq!(result.status, "bounded");
    assert_eq!(result.reason.as_deref(), Some("evaluation"));
    assert_eq!(result.states, 32);
    assert_eq!(result.work, 6_592);
}

#[test]
fn a_worker_pool_publishes_a_violation_found_before_a_later_forwarding_refusal() {
    let program = violating_first_state(200, 300);
    let result = program.check_parallel(unbounded_work(), 2).unwrap();
    assert_eq!(result.status, "invariant-violation");
    let violation = result.violation.unwrap();
    assert_eq!(violation.name, "Safe");
    let trace: Vec<_> = violation
        .trace
        .iter()
        .map(|step| step.values.clone())
        .collect();
    assert_eq!(
        trace,
        [
            vec![rtlc::Value::Int("0".into())],
            vec![rtlc::Value::Int("16".into())]
        ]
    );
}
