use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;

#[test]
fn recursive_assignment_actions_compile_as_shared_v2_definitions() {
    for definitions in [
        "RECURSIVE Down(_)\nDown(n) == IF n=0 THEN x'=(x+1)%3 ELSE Down(n-1)",
        "RECURSIVE Down(_), Other(_)\nDown(n) == IF n=0 THEN x'=(x+1)%3 ELSE Other(n-1)\nOther(n) == Down(n)",
    ] {
        let module = rtla::parse_module(&format!("---- MODULE RecursiveActions ----\nEXTENDS Naturals\nVARIABLE x\n{definitions}\nInit == x=0\nNext == Down(4)\n====")).unwrap();
        let config = rtla::parse_config("INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE").unwrap();
        assert!(
            compile_with_options(
                &module,
                &config,
                CompileOptions {
                    irVersion: IrVersion::V1
                }
            )
            .is_err()
        );
        let program = compile_with_options(
            &module,
            &config,
            CompileOptions {
                irVersion: IrVersion::V2,
            },
        )
        .unwrap();
        let wire = serde_json::to_value(&program).unwrap();
        assert!(
            wire["definitions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|definition| definition["bodyKind"] == "action")
        );
        assert!(serde_json::to_vec(&program).unwrap().len() < 10_000);
        let result = program
            .check(SafetyBounds {
                max_depth: 1000,
                max_states: 2000,
                max_work: 1_000_000,
                max_temporal_nodes: None,
            })
            .unwrap();
        assert_eq!(result.status, "complete");
        assert_eq!(result.states, 3);
        assert_eq!(result.generated, 4);
    }
}

#[test]
fn recursive_action_cases_preserve_caller_frames_continuations_and_assignments() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("recursive-actions.json")).unwrap();
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let module = rtla::parse_module(case["source"].as_str().unwrap()).unwrap();
        let config = rtla::parse_config(case["config"].as_str().unwrap()).unwrap();
        let modules = case["modules"]
            .as_object()
            .map(|modules| {
                modules
                    .values()
                    .map(|value| rtla::parse_module(value.as_str().unwrap()).unwrap())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let compiled = rtla_compiler::compile_modules_with_options(
            &module,
            &config,
            &modules,
            CompileOptions {
                irVersion: IrVersion::V2,
            },
        );
        if case["status"] == "error" {
            assert!(compiled.is_err(), "{name}");
            continue;
        }
        let program = compiled.unwrap_or_else(|error| panic!("{name}: {error}"));
        let result = program
            .check(SafetyBounds {
                max_depth: 1000,
                max_states: 2000,
                max_work: 1_000_000,
                max_temporal_nodes: Some(10000),
            })
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            result.status,
            case["status"].as_str().unwrap(),
            "{name}: {result:?}"
        );
        if result.status == "bounded" {
            assert_eq!(result.reason.as_deref(), Some("evaluation"), "{name}");
            assert!(result.violation.is_none());
        } else {
            assert_eq!(
                result.states as u64,
                case["states"].as_u64().unwrap(),
                "{name}"
            );
            assert_eq!(
                result.generated as u64,
                case["generated"].as_u64().unwrap(),
                "{name}"
            );
        }
    }
}

#[test]
fn recursive_higher_order_actions_specialize_operator_arguments_once() {
    let module=rtla::parse_module("---- MODULE Higher ----\nEXTENDS Naturals\nVARIABLE x\nRECURSIVE Apply(_,_)\nApply(F(_),n) == IF n=0 THEN F((x+1)%3) ELSE Apply(F,n-1)\nAssign(v) == x'=v\nInit == x=0\nNext == Apply(Assign,3)\n====").unwrap();
    let config = rtla::parse_config("INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE").unwrap();
    let program = compile_with_options(
        &module,
        &config,
        CompileOptions {
            irVersion: IrVersion::V2,
        },
    )
    .unwrap();
    assert!(program.definitions.as_ref().unwrap().len() < 8);
    let result = program
        .check(SafetyBounds {
            max_depth: 100,
            max_states: 1000,
            max_work: 1_000_000,
            max_temporal_nodes: None,
        })
        .unwrap();
    assert_eq!(result.status, "complete");
    assert_eq!(result.states, 3);
    assert_eq!(result.generated, 4);
}

#[test]
fn recursive_generation_reuses_prepared_searches_and_parallel_workers() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("recursive-actions.json")).unwrap();
    let case = cases.iter().find(|case| case["name"] == "wide").unwrap();
    let module = rtla::parse_module(case["source"].as_str().unwrap()).unwrap();
    let config = rtla::parse_config(case["config"].as_str().unwrap()).unwrap();
    let program = compile_with_options(
        &module,
        &config,
        CompileOptions {
            irVersion: IrVersion::V2,
        },
    )
    .unwrap()
    .prepare()
    .unwrap();
    let bounds = SafetyBounds {
        max_depth: 1000,
        max_states: 2000,
        max_work: 1_000_000,
        max_temporal_nodes: None,
    };
    let expected = program.check(bounds).unwrap();
    let result = program
        .check(SafetyBounds {
            max_work: 5,
            ..bounds
        })
        .unwrap();
    assert_eq!(result.status, "bounded");
    assert_eq!(result.reason.as_deref(), Some("work"));
    assert_eq!(program.check(bounds).unwrap(), expected);
    let parallel = program
        .check_with_options(
            bounds,
            rtlc::SafetyOptions {
                workers: 4,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(parallel.status, expected.status);
    assert_eq!(parallel.states, expected.states);
    assert_eq!(parallel.generated, expected.generated);
}
