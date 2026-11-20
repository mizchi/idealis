use rtlc::{SafetyBounds, SafetyProgram};
use serde_json::json;

#[path = "../examples/counter.rs"]
mod counter;

#[test]
fn quantified_obligations_cross_word_and_storage_boundaries() {
    for size in [16, 17, 64, 65] {
        for limit in [4, 2] {
            let mut input = serde_json::to_value(counter::ring()).unwrap();
            input["properties"] = json!([{"name":"Live","formula":{
                "kind":"forall","bounds":[{"slot":0,"domain":{"kind":"range","lower":{"op":"literal","value":{"type":"int","value":"1"}},"upper":{"op":"literal","value":{"type":"int","value":size.to_string()}}}}],
                "formula":{"kind":"always","formula":{"kind":"atom","expression":{"op":"lt","left":{"op":"read","slot":0,"next":false},"right":{"op":"literal","value":{"type":"int","value":limit.to_string()}}}}}
            }}]);
            let program: SafetyProgram = serde_json::from_value(input).unwrap();
            let bounds = SafetyBounds {
                max_depth: 100,
                max_states: 1000,
                max_work: 1_000_000,
                max_temporal_nodes: Some(10000),
            };
            let full = program.check(bounds).unwrap();
            assert_eq!(full.states, 4);
            assert_eq!(
                full.status,
                if limit == 4 {
                    "complete"
                } else {
                    "liveness-violation"
                }
            );
            if limit == 2 {
                let witness = full.violation.as_ref().unwrap();
                assert!(witness.loop_start.unwrap() < witness.trace.len());
            }
            for max_work in [1, 17, 100, full.work - 1] {
                let cut = program.check(SafetyBounds { max_work, ..bounds }).unwrap();
                assert_eq!(cut.status, "bounded");
                assert_eq!(cut.reason.as_deref(), Some("work"));
                assert!(cut.work <= max_work);
            }
        }
    }
}
