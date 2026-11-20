use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;
use serde_json::Value;

#[test]
fn v2_lowers_standard_callbacks_and_assertions_like_v1() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("operators-support.json")).unwrap();
    let mut checked = 0;
    for case in cases.iter().filter(|case| {
        case["expected"] == "complete"
            && case["modules"]
                .as_object()
                .is_some_and(|modules| modules.is_empty())
    }) {
        let source = case["source"].as_str().unwrap();
        let module = rtla::parse_module(source).unwrap();
        let config = rtla::parse_config(case["config"].as_str().unwrap()).unwrap();
        let first = compile_with_options(
            &module,
            &config,
            CompileOptions {
                irVersion: IrVersion::V1,
            },
        )
        .unwrap();
        let second = compile_with_options(
            &module,
            &config,
            CompileOptions {
                irVersion: IrVersion::V2,
            },
        )
        .unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
        let bounds = SafetyBounds {
            max_depth: 100,
            max_states: 1000,
            max_work: 1000000,
            max_temporal_nodes: None,
        };
        let first = first.check(bounds).unwrap();
        let second = second.check(bounds).unwrap();
        assert_eq!(first.status, second.status, "{}", case["name"]);
        assert_eq!(first.states, second.states, "{}", case["name"]);
        assert_eq!(first.generated, second.generated, "{}", case["name"]);
        assert_eq!(
            serde_json::to_value(first.violation).unwrap(),
            serde_json::to_value(second.violation).unwrap(),
            "{}",
            case["name"]
        );
        checked += 1;
    }
    assert!(checked >= 20);
}

#[test]
fn v2_module_bundles_and_configuration_substitutions_match_v1() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("modules-support.json")).unwrap();
    let mut checked = 0;
    for case in cases {
        let module = rtla::parse_module(case["source"].as_str().unwrap()).unwrap();
        let config = rtla::parse_config(case["config"].as_str().unwrap()).unwrap();
        let modules = case["modules"]
            .as_object()
            .unwrap()
            .values()
            .map(|value| rtla::parse_module(value.as_str().unwrap()).unwrap())
            .collect::<Vec<_>>();
        let first = rtla_compiler::compile_modules_with_options(
            &module,
            &config,
            &modules,
            CompileOptions {
                irVersion: IrVersion::V1,
            },
        );
        let second = rtla_compiler::compile_modules_with_options(
            &module,
            &config,
            &modules,
            CompileOptions {
                irVersion: IrVersion::V2,
            },
        );
        let first = match first {
            Ok(first) => {
                assert_ne!(case["expected"], "unsupported", "{}", case["name"]);
                first
            }
            Err(error) => {
                assert_eq!(case["expected"], "unsupported", "{}: {error}", case["name"]);
                assert!(second.is_err(), "{}", case["name"]);
                continue;
            }
        };
        let second = second.unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
        let bounds = SafetyBounds {
            max_depth: 100,
            max_states: 1000,
            max_work: 1000000,
            max_temporal_nodes: None,
        };
        let first = first.check(bounds);
        let second = second.check(bounds);
        match (first, second) {
            (Ok(first), Ok(second)) => {
                assert_eq!(
                    first.status,
                    case["expected"].as_str().unwrap(),
                    "{}",
                    case["name"]
                );
                assert_eq!(first.status, second.status, "{}", case["name"]);
                assert_eq!(first.states, second.states, "{}", case["name"]);
                assert_eq!(first.generated, second.generated, "{}", case["name"]);
            }
            (Err(_), Err(_)) => {}
            (first, second) => panic!("{}: {first:?} != {second:?}", case["name"]),
        }
        checked += 1;
    }
    assert!(checked > 10);
}

#[test]
fn frontend_completion_models_match_the_independent_tlc_controls() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("frontend-completion.json")).unwrap();
    assert!(cases.len() >= 34);
    for case in cases {
        let module = rtla::parse_module(case["source"].as_str().unwrap()).unwrap();
        let config = rtla::parse_config(case["config"].as_str().unwrap()).unwrap();
        let modules: Vec<_> = case["modules"]
            .as_object()
            .into_iter()
            .flat_map(|modules| modules.values())
            .map(|source| rtla::parse_module(source.as_str().unwrap()).unwrap())
            .collect();
        for version in case["versions"].as_array().unwrap() {
            let version = IrVersion::try_from(version.as_u64().unwrap() as u8).unwrap();
            let program = rtla_compiler::compile_modules_with_options(
                &module,
                &config,
                &modules,
                CompileOptions { irVersion: version },
            );
            if case["status"] == "error" {
                assert!(program.is_err(), "{} {version:?}", case["name"]);
                continue;
            }
            let program =
                program.unwrap_or_else(|error| panic!("{} {version:?}: {error}", case["name"]));
            let result = program
                .check(SafetyBounds {
                    max_depth: 100,
                    max_states: 1000,
                    max_work: 1000000,
                    max_temporal_nodes: None,
                })
                .unwrap_or_else(|error| panic!("{} {version:?}: {error}", case["name"]));
            assert_eq!(
                result.status,
                case["status"].as_str().unwrap(),
                "{} {version:?}",
                case["name"]
            );
            assert_eq!(
                result.states as u64,
                case["states"].as_u64().unwrap(),
                "{} {version:?}",
                case["name"]
            );
        }
    }
}

#[test]
fn specification_wrappers_keep_the_constant_argument_boundary_in_v2() {
    let module = rtla::parse_module("---- MODULE Test ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=(x+1)%2\nWrapped(unused) == Init /\\ [][Next]_x\nSpec == Wrapped(x)\n====").unwrap();
    let config = rtla::parse_config("SPECIFICATION Spec\nCHECK_DEADLOCK FALSE").unwrap();
    for version in [IrVersion::V1, IrVersion::V2] {
        let error = compile_with_options(&module, &config, CompileOptions { irVersion: version })
            .unwrap_err();
        assert_eq!(
            error.message,
            "SPECIFICATION operator arguments must be constant"
        );
    }
}
