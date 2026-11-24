use rtlc::{SafetyBounds, SafetyProgram};
use serde_json::{Value, json};
use std::time::Instant;

fn literal(value: i64) -> Value {
    json!({"op":"literal","value":{"type":"int","value":value.to_string()}})
}

fn program(states: i64, records: bool, version: u8) -> SafetyProgram {
    let elements: Vec<Value> = (0..64)
        .map(|n| {
            if records {
                json!({"op":"record","fields":[["a", literal(n)]]})
            } else {
                literal(n)
            }
        })
        .collect();
    let (callback, definitions) = if version == 1 {
        (
            json!({"op":"cup","left":{"op":"bound","slot":0},"right":{"op":"set","values":[{"op":"bound","slot":1}]}}),
            json!(null),
        )
    } else {
        (
            json!({"op":"call","definition":0,"arguments":[{"op":"bound","slot":0},{"op":"bound","slot":1}],"captures":[]}),
            json!([{"id":0,"parameterCount":2,"captureCount":0,"bodyKind":"expression","localScope":2,"body":{"op":"cup","left":{"op":"parameter","slot":0},"right":{"op":"set","values":[{"op":"parameter","slot":1}]}}}]),
        )
    };
    let fold = json!({"op":"fold","accumulator":0,"initial":{"op":"set","values":[]},"collection":{"op":"set","values":elements},"order":"set-members","body":callback});
    let read = json!({"op":"read","slot":0,"next":false});
    let next_value = json!({"op":"mod","left":{"op":"add","left":read,"right":{"op":"sub","left":{"op":"cardinality","argument":fold},"right":literal(63)}},"right":literal(states)});
    let mut wire = json!({
        "format":"modelt.safety-program","version":version,"name":"Cost","variables":["x"],
        "initial":[{"name":"Init","steps":[{"kind":"assign","slot":0,"value":literal(0)}]}],
        "next":[{"name":"Next","steps":[{"kind":"assign","slot":0,"value":next_value}]}],
        "invariants":[{"name":"Safe","expression":{"op":"le","left":{"op":"read","slot":0,"next":false},"right":literal(states)}}],
        "assumptions":[],"checkDeadlock":false
    });
    if version == 2 {
        wire["definitions"] = definitions;
    }
    serde_json::from_value(wire).unwrap()
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    values[values.len() / 2]
}

fn main() {
    let bounds = SafetyBounds {
        max_depth: 1000,
        max_states: 10_000,
        max_work: 100_000_000,
        max_temporal_nodes: None,
    };
    println!("version states kind work median_us");
    for version in [1, 2] {
        for states in [1, 64] {
            for records in [false, true] {
                let program = program(states, records, version);
                let result = program.check(bounds).unwrap();
                assert_eq!(result.status, "complete");
                assert_eq!(result.states as i64, states);
                let times: Vec<f64> = (0..400)
                    .map(|_| {
                        let start = Instant::now();
                        let _ = program.check(bounds).unwrap();
                        start.elapsed().as_secs_f64() * 1e6
                    })
                    .collect();
                println!(
                    "v{version} {states} {} {} {:.1}",
                    if records { "records" } else { "integers" },
                    result.work,
                    median(times)
                );
            }
        }
    }
}
