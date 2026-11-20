use rtlc::{SafetyBounds, SafetyOptions, SafetyProgram};
use serde_json::json;

fn program(version: u8) -> SafetyProgram {
    let mut wire = json!({
        "format":"modelt.safety-program","version":version,"name":"PreparedCounter",
        "variables":["x"],"checkDeadlock":false,
        "assumptions":[],"initialProperties":[],"actionProperties":[],
        "properties":[],"fairness":[],"constraints":[],"actionConstraints":[],
        "initial":[{"name":"Init","steps":[{"kind":"assign","slot":0,"value":{"op":"literal","value":{"type":"int","value":"0"}}}]}],
        "next":[{"name":"Next","steps":[{"kind":"assign","slot":0,"value":{"op":"add","left":{"op":"read","slot":0,"next":false},"right":{"op":"literal","value":{"type":"int","value":"1"}}}}]}],
        "invariants":[{"name":"Safe","expression":{"op":"lt","left":{"op":"read","slot":0,"next":false},"right":{"op":"literal","value":{"type":"int","value":"3"}}}}]
    });
    if version == 2 {
        wire["definitions"] = json!([]);
    }
    serde_json::from_value(wire).unwrap()
}

#[test]
fn each_prepared_check_has_independent_state_budget_and_coverage() {
    for version in [1, 2] {
        let program = program(version);
        let prepared = program.clone().prepare().unwrap();
        for max_work in [1, 17, 10000, 2, 10000] {
            let bounds = SafetyBounds {
                max_depth: 10,
                max_states: 100,
                max_work,
                max_temporal_nodes: None,
            };
            assert_eq!(
                serde_json::to_value(prepared.check(bounds).unwrap()).unwrap(),
                serde_json::to_value(program.check(bounds).unwrap()).unwrap()
            );
            assert_eq!(
                serde_json::to_value(prepared.check_with_coverage(bounds).unwrap()).unwrap(),
                serde_json::to_value(program.check_with_coverage(bounds).unwrap()).unwrap()
            );
            for workers in [1, 8] {
                let options = SafetyOptions {
                    workers,
                    ..Default::default()
                };
                assert_eq!(
                    serde_json::to_value(prepared.check_with_options(bounds, options).unwrap())
                        .unwrap(),
                    serde_json::to_value(program.check_with_options(bounds, options).unwrap())
                        .unwrap()
                );
            }
        }
    }
}

#[test]
fn preparation_does_not_evaluate_constant_predicates_and_recovers_after_errors() {
    let mut program = program(1);
    program.invariants[0].expression =
        serde_json::from_value(json!({"op":"literal","value":{"type":"bool","value":false}}))
            .unwrap();
    let prepared = program.clone().prepare().unwrap();
    let bounds = SafetyBounds {
        max_depth: 10,
        max_states: 100,
        max_work: 10000,
        max_temporal_nodes: None,
    };
    for _ in 0..2 {
        let expected = program.check(bounds).unwrap_err();
        let actual = prepared.check(bounds).unwrap_err();
        assert_eq!(
            (actual.code, actual.message),
            (expected.code, expected.message)
        );
    }
}

#[test]
fn preparation_rejects_invalid_static_ir_without_running_a_search() {
    let mut program = program(2);
    program.definitions = None;
    let error = match program.prepare() {
        Ok(_) => panic!("invalid v2 was prepared"),
        Err(error) => error,
    };
    assert_eq!(error.code, "protocol");
    assert_eq!(error.message, "Missing safety IR v2 definitions");
}

#[test]
fn concurrent_callers_share_only_immutable_preparation() {
    let program = program(2);
    let prepared = program.clone().prepare().unwrap();
    std::thread::scope(|scope| {
        for max_work in [1, 2, 17, 10000] {
            let program = &program;
            let prepared = &prepared;
            scope.spawn(move || {
                let bounds = SafetyBounds {
                    max_depth: 10,
                    max_states: 100,
                    max_work,
                    max_temporal_nodes: None,
                };
                for _ in 0..8 {
                    assert_eq!(
                        serde_json::to_value(prepared.check(bounds).unwrap()).unwrap(),
                        serde_json::to_value(program.check(bounds).unwrap()).unwrap()
                    );
                }
            });
        }
    });
}

#[test]
fn untaken_invalid_data_is_not_evaluated_by_preparation_or_repeated_checks() {
    for version in [1, 2] {
        let mut program = program(version);
        program.invariants[0].expression = serde_json::from_value(json!({
            "op":"if","condition":{"op":"literal","value":{"type":"bool","value":false}},
            "consequent":{"op":"data","value":{"type":"int","value":"invalid"}},
            "alternative":{"op":"literal","value":{"type":"bool","value":true}}
        }))
        .unwrap();
        let prepared = program.clone().prepare().unwrap();
        let bounds = SafetyBounds {
            max_depth: 2,
            max_states: 100,
            max_work: 10000,
            max_temporal_nodes: None,
        };
        for _ in 0..3 {
            assert_eq!(
                serde_json::to_value(prepared.check(bounds).unwrap()).unwrap(),
                serde_json::to_value(program.check(bounds).unwrap()).unwrap()
            );
        }
    }
}
