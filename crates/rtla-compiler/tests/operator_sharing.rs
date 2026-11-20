use rtla_compiler::{
    CompileError, CompileOptions, IrVersion, compile, compile_modules_with_options,
    compile_with_options,
};
use rtlc::SafetyBounds;

#[test]
fn explicit_v1_preserves_serialization_and_v2_is_accepted() {
    let module = rtla::parse_module(
        "---- MODULE Options ----\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\n====",
    )
    .unwrap();
    let config = rtla::parse_config("INIT Init\nNEXT Next").unwrap();
    let default = compile(&module, &config).unwrap();
    let explicit = compile_with_options(
        &module,
        &config,
        CompileOptions {
            irVersion: IrVersion::V1,
        },
    )
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&default).unwrap(),
        serde_json::to_vec(&explicit).unwrap()
    );
    let shared = compile_with_options(
        &module,
        &config,
        CompileOptions {
            irVersion: IrVersion::V2,
        },
    )
    .unwrap();
    let shared = serde_json::to_value(&shared).unwrap();
    assert_eq!(shared["version"], 2);
    assert!(shared["definitions"].is_array());
    assert_eq!(IrVersion::try_from(1).unwrap(), IrVersion::V1);
    assert_eq!(IrVersion::try_from(2).unwrap(), IrVersion::V2);
    assert!(IrVersion::try_from(0).is_err());
    assert!(IrVersion::try_from(3).is_err());
}

macro_rules! oracle {
    ($test:ident, $case:literal) => {
        #[test]
        fn $test() {
            let module = rtla::parse_module(include_str!(concat!(
                "../../../fixtures/tlc-operator-sharing/",
                $case,
                "/Differential.tla"
            )))
            .unwrap();
            let config = rtla::parse_config(include_str!(concat!(
                "../../../fixtures/tlc-operator-sharing/",
                $case,
                "/Differential.cfg"
            )))
            .unwrap();
            let compiled = compile_with_options(
                &module,
                &config,
                CompileOptions {
                    irVersion: IrVersion::V2,
                },
            );
            assert!(
                compiled.is_ok(),
                "{}: expected v2 compilation, got {:?}",
                $case,
                compiled.err()
            );
            let compiled = compiled.unwrap();
            assert_eq!(serde_json::to_value(&compiled).unwrap()["version"], 2);
            let result = compiled.check(SafetyBounds {
                max_depth: 64,
                max_states: 10000,
                max_work: if $case == "sharing-work-60" {
                    256
                } else {
                    1000000
                },
                max_temporal_nodes: None,
            });
            let golden: serde_json::Value = serde_json::from_str(include_str!(
                "../../../fixtures/tlc-operator-sharing/expected.json"
            ))
            .unwrap();
            let additional: serde_json::Value = serde_json::from_str(include_str!(
                "../../../fixtures/tlc-operator-sharing/expected-step3c.json"
            ))
            .unwrap();
            let generative: serde_json::Value = serde_json::from_str(include_str!(
                "../../../fixtures/tlc-operator-sharing/expected-step3d-a.json"
            ))
            .unwrap();
            let frames: serde_json::Value = serde_json::from_str(include_str!(
                "../../../fixtures/tlc-operator-sharing/expected-step3e-a.json"
            ))
            .unwrap();
            let fixture = golden["cases"]
                .as_array()
                .unwrap()
                .iter()
                .chain(additional["cases"].as_array().unwrap())
                .chain(generative["cases"].as_array().unwrap())
                .chain(frames["cases"].as_array().unwrap())
                .find(|case| case["name"] == $case)
                .unwrap();
            if $case == "sharing-used-argument" {
                let error = result.unwrap_err();
                assert_eq!(fixture["status"], "error");
                assert!(error.message.contains("Head") && error.message.contains("empty sequence"));
            } else {
                let result = serde_json::to_value(result.unwrap()).unwrap();
                if $case == "sharing-work-60" {
                    assert_eq!(result["status"], "bounded");
                    assert_eq!(result["reason"], "work");
                    assert!(result["work"].as_u64().unwrap() <= 256);
                } else if fixture["status"] == "invariant-violation" {
                    assert_eq!(result["status"], fixture["status"]);
                    assert_eq!(result["violation"]["name"], fixture["violation"]["name"]);
                    assert_eq!(result["violation"]["trace"], fixture["violation"]["trace"]);
                } else {
                    assert_eq!(fixture["status"], "success");
                    assert_eq!(result["status"], "complete");
                    assert_eq!(result["states"], fixture["states"]);
                    assert_eq!(result["generated"], fixture["generated"]);
                }
            }
        }
    };
}
oracle!(v2_chain_10, "sharing-chain-10");
oracle!(v2_chain_20, "sharing-chain-20");
oracle!(v2_chain_30, "sharing-chain-30");
oracle!(v2_chain_60, "sharing-chain-60");
oracle!(v2_work_60, "sharing-work-60");
oracle!(v2_lexical_capture, "sharing-lexical-capture");
oracle!(v2_binder_quantifier, "sharing-binder-quantifier");
oracle!(v2_binder_function, "sharing-binder-function");
oracle!(v2_binder_filter, "sharing-binder-filter");
oracle!(v2_binder_map_set, "sharing-binder-map-set");
oracle!(v2_binder_fold_sequence, "sharing-binder-fold-sequence");
oracle!(v2_binder_fold_set, "sharing-binder-fold-set");
oracle!(v2_binder_tuple_pattern, "sharing-binder-tuple-pattern");
oracle!(v2_unused_argument, "sharing-unused-argument");
oracle!(v2_used_argument, "sharing-used-argument");
oracle!(v2_prime, "sharing-prime");
oracle!(v2_enabled, "sharing-enabled");
oracle!(v2_generative_except, "sharing-generative-except");
oracle!(
    v2_generative_except_control,
    "sharing-generative-except-control"
);
oracle!(v2_generative_binder, "sharing-generative-binder");
oracle!(
    v2_generative_binder_control,
    "sharing-generative-binder-control"
);
oracle!(v2_generative_membership, "sharing-generative-membership");
oracle!(
    v2_generative_membership_alias,
    "sharing-generative-membership-alias"
);
oracle!(
    v2_generative_binder_exact,
    "sharing-generative-binder-exact"
);
oracle!(
    v2_generative_binder_exact_control,
    "sharing-generative-binder-exact-control"
);

fn source_control(declarations: &str, safe: &str, next: &str) -> String {
    format!(
        "---- MODULE SharingControl ----\nVARIABLE x\n{declarations}\nInit == x = 0\nNext == {next}\nSafe == {safe}\n===="
    )
}

fn assert_source_control(source: &str) {
    let module = rtla::parse_module(source).unwrap();
    let config =
        rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE").unwrap();
    let compiled = compile_with_options(
        &module,
        &config,
        CompileOptions {
            irVersion: IrVersion::V2,
        },
    )
    .unwrap();
    assert_eq!(serde_json::to_value(&compiled).unwrap()["version"], 2);
    let result = compiled
        .check(SafetyBounds {
            max_depth: 64,
            max_states: 10000,
            max_work: 1000000,
            max_temporal_nodes: None,
        })
        .unwrap();
    assert_eq!(result.status, "complete");
    assert_eq!(result.states, 1);
    assert_eq!(result.generated, 2);
    assert!(result.violation.is_none());
}

#[test]
fn v2_handwritten_capture_survives_binder_shadowing() {
    assert_source_control(&source_control(
        "",
        "\\A c \\in {0}: LET G == c = 0 IN \\A c \\in {1}: G",
        "UNCHANGED x",
    ));
}

#[test]
fn v2_handwritten_sibling_captures_outer_binder() {
    assert_source_control(&source_control(
        "",
        "\\A c \\in {0}: LET H == c = 0 G == H IN G",
        "UNCHANGED x",
    ));
}

#[test]
fn v2_handwritten_transitive_siblings_retain_declaring_frame() {
    assert_source_control(&source_control(
        "",
        "\\A c \\in {0}: LET H == c = 0 G == H F == G IN \\A c \\in {1}: F",
        "UNCHANGED x",
    ));
}

#[test]
fn v2_handwritten_sibling_parameters_retain_declaring_frame() {
    assert_source_control(&source_control(
        "",
        "\\A c \\in {0}: LET H(p) == p /\\ (c = 0) G(q) == H(q) IN \\A c \\in {1}: G(TRUE)",
        "UNCHANGED x",
    ));
}

fn guard_chain() -> String {
    let mut declarations = String::from("D0 == TRUE");
    for index in 0..60 {
        declarations.push_str(&format!(
            "\nD{} == IF TRUE THEN TRUE ELSE (D{index} /\\ D{index})",
            index + 1
        ));
    }
    declarations
}

#[test]
fn v2_handwritten_deep_expression_guard_in_next() {
    assert_source_control(&source_control(
        &guard_chain(),
        "TRUE",
        "D60 /\\ UNCHANGED x",
    ));
}

#[test]
fn v2_handwritten_deep_expression_guard_in_inlined_action() {
    assert_source_control(&source_control(
        &format!("{}\nGuarded == D60 /\\ UNCHANGED x", guard_chain()),
        "TRUE",
        "Guarded",
    ));
}

#[test]
fn v2_handwritten_nested_actuals_keep_nesting_limit_40() {
    let safe = format!("{}TRUE{}", "Id(".repeat(41), ")".repeat(41));
    let source = source_control("Id(p) == p", &safe, "UNCHANGED x");
    let module = rtla::parse_module(&source).unwrap();
    let config = rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe").unwrap();
    let error = compile_with_options(
        &module,
        &config,
        CompileOptions {
            irVersion: IrVersion::V2,
        },
    )
    .unwrap_err();
    assert_eq!(error.message, "expression or operator nesting exceeds 40");
}

oracle!(v2_unchanged_alias, "sharing-unchanged-alias");
oracle!(v2_unchanged_parameter, "sharing-unchanged-parameter");
oracle!(v2_unchanged_nested, "sharing-unchanged-nested");

#[test]
fn v2_forward_chain_5000_compiles_without_evaluation() {
    let mut declarations = String::new();
    for id in 0..5000 {
        declarations.push_str(&format!(
            "O{id} == {}\n",
            if id == 4999 {
                "TRUE".into()
            } else {
                format!("O{}", id + 1)
            }
        ));
    }
    let source = source_control(&declarations, "O0", "UNCHANGED x");
    let module = rtla::parse_module(&source).unwrap();
    let config = rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe").unwrap();
    let program = compile_with_options(
        &module,
        &config,
        CompileOptions {
            irVersion: IrVersion::V2,
        },
    )
    .unwrap();
    assert_eq!(program.definitions.as_ref().unwrap().len(), 5001);
}

#[test]
fn v2_enabled_variable_target_keeps_actual_and_captured_parameters() {
    for declaration in [
        "Can(p) == ENABLED (UNCHANGED p)",
        "Can(p) == LET Keep == ENABLED (UNCHANGED p) IN Keep",
        "Keep(p) == UNCHANGED p\nInner(p) == ENABLED Keep(p)\nCan(q) == Inner(q)",
    ] {
        assert_source_control(&source_control(
            &format!("Vars == <<x>>\n{declaration}"),
            "Can(Vars)",
            "UNCHANGED x",
        ));
    }
}

#[test]
fn v2_target_alias_actual_restores_caller_ancestry() {
    assert_source_control(&source_control(
        "Id(p) == p\nCan(v) == ENABLED (UNCHANGED v)",
        "Can(Id(Id(x)))",
        "UNCHANGED x",
    ));
}

#[test]
fn v2_membership_purpose_reaches_formals_aliases_and_captures() {
    for declaration in [
        "Has(p) == x \\in p",
        "Has(p) == LET Local == x \\in p IN Local",
        "Inner(p) == x \\in p\nHas(q) == Inner(q)",
        "Types(p) == IF TRUE THEN p ELSE {1}\nHas(q) == x \\in Types(q)",
        "Types(p) == CASE TRUE -> p [] OTHER -> {1}\nHas(q) == x \\in Types(q)",
    ] {
        assert_source_control(
            &source_control(declaration, "Has(Nat)", "UNCHANGED x")
                .replace("VARIABLE x", "EXTENDS Naturals\nVARIABLE x"),
        );
    }
}

#[test]
fn v2_enabled_subscript_target_specialization() {
    for (declaration, safe) in [
        ("Can(v) == ENABLED [x' = x]_v", "Can(x)"),
        ("Can(v) == ENABLED <<x' = x>>_v", "~Can(x)"),
    ] {
        assert_source_control(&source_control(declaration, safe, "UNCHANGED x"));
    }
}

#[test]
fn v2_frame_dependent_action_actual_keeps_caller_binder() {
    assert_source_control(&source_control(
        "Take(p) == x' = p",
        "x = 0",
        "\\E a \\in {0}: Take(a)",
    ));
}

#[test]
fn v2_action_capture_target_resolves_original_source_actual() {
    assert_source_control(&source_control(
        "Vars == <<x>>\nKeep(p) == LET Can == ENABLED (UNCHANGED p) IN Can /\\ UNCHANGED p",
        "x = 0",
        "Keep(Vars)",
    ));
}

oracle!(v2_action_frame_exists, "sharing-action-frame-exists");
oracle!(
    v2_action_frame_two_variables,
    "sharing-action-frame-two-variables"
);
oracle!(
    v2_action_frame_binder_actual,
    "sharing-action-frame-binder-actual"
);
oracle!(v2_action_frame_except, "sharing-action-frame-except");
oracle!(
    v2_action_frame_except_control,
    "sharing-action-frame-except-control"
);
oracle!(v2_action_frame_shadow, "sharing-action-frame-shadow");
oracle!(v2_action_frame_enabled, "sharing-action-frame-enabled");

#[test]
fn v2_let_with_free_old_matches_tlc_in_nested_enabled() {
    for (definition, application) in [
        ("Step(a) == @ = 9 /\\ x' = a", "Step(x)"),
        ("Step == @ = 9 /\\ x' = x", "Step"),
    ] {
        let safe = format!(
            "x = 0 /\\ [<<9>> EXCEPT ![1] = LET {definition} IN [<<4>> EXCEPT ![1] = ENABLED {application}][1]][1]"
        );
        let source = source_control("", &safe, "UNCHANGED x")
            .replace("VARIABLE x", "EXTENDS Naturals, Sequences\nVARIABLE x");
        let module = rtla::parse_module(&source).unwrap();
        let config =
            rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE")
                .unwrap();
        let program = compile_with_options(
            &module,
            &config,
            CompileOptions {
                irVersion: IrVersion::V2,
            },
        )
        .unwrap();
        let result = program
            .check(SafetyBounds {
                max_depth: 64,
                max_states: 10000,
                max_work: 1000000,
                max_temporal_nodes: None,
            })
            .unwrap();
        if application == "Step" {
            assert!(
                compile_with_options(
                    &module,
                    &config,
                    CompileOptions {
                        irVersion: IrVersion::V1
                    }
                )
                .unwrap_err()
                .message
                .contains("require safety IR v2")
            );
            assert_eq!(result.status, "complete");
            assert!(result.violation.is_none());
            let wrong = rtla::parse_module(&source.replace("@ = 9", "@ = 4")).unwrap();
            let wrong = compile_with_options(
                &wrong,
                &config,
                CompileOptions {
                    irVersion: IrVersion::V2,
                },
            )
            .unwrap()
            .check(SafetyBounds {
                max_depth: 64,
                max_states: 10000,
                max_work: 1000000,
                max_temporal_nodes: None,
            })
            .unwrap();
            assert_eq!(wrong.status, "invariant-violation");
        } else {
            assert_eq!(result.status, "invariant-violation");
            assert!(result.violation.is_some());
            assert_source_control(&source.replace("@ = 9", "@ = 4"));
        }
    }
}

#[test]
fn v2_function_definition_domains_capture_free_except_old() {
    for (domain, body, application) in [
        ("@", "i", "f[9] = 9"),
        ("@", "i", "TRUE"),
        ("@", "i", "ENABLED (x' = f[9])"),
        ("{9}", "@", "f[9] = {9}"),
    ] {
        for definition in [
            format!("f[i \\in {domain}] == {body}"),
            format!("f == [i \\in {domain} |-> {body}]"),
        ] {
            let source = source_control(
                "",
                &format!(
                    "x = 0 /\\ [<<{{9}}>> EXCEPT ![1] = LET {definition} IN {application}][1]"
                ),
                "UNCHANGED x",
            );
            assert_source_control(&source);
        }
    }
    for definition in ["f[i \\in @] == i", "f == [i \\in @ |-> i]"] {
        let source = source_control(
            "",
            &format!(
                "x = 0 /\\ [<<{{9}}>> EXCEPT ![1] = LET Id(a) == LET {definition} IN f[9] IN Id(0) = 9][1]"
            ),
            "UNCHANGED x",
        );
        assert_source_control(&source);
    }
}

#[test]
fn v2_function_definition_domains_propagate_target_and_membership_requirements() {
    let golden: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/tlc-operator-sharing/expected-step3c.json"
    ))
    .unwrap();
    let stuttering_control = golden["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "sharing-unchanged-parameter")
        .unwrap();
    assert_eq!(stuttering_control["status"], "success");
    for (parameter, domain, application, safe, work) in [
        (
            "v",
            "(IF ENABLED (UNCHANGED v) THEN {0} ELSE {})",
            "f[0] = 0",
            "x = 0 /\\ F(x)",
            12,
        ),
        ("p", "{IF x \\in p THEN 1 ELSE 2}", "f[1] = 1", "F(Nat)", 10),
    ] {
        let mut previous = None;
        for definition in [
            format!("f[i \\in {domain}] == i"),
            format!("f == [i \\in {domain} |-> i]"),
        ] {
            let source = format!(
                "---- MODULE FunctionDomain ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\nF({parameter}) == LET {definition} IN {application}\nSafe == {safe}\n===="
            );
            let module = rtla::parse_module(&source).unwrap();
            let config =
                rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE")
                    .unwrap();
            let compiled = compile_with_options(
                &module,
                &config,
                CompileOptions {
                    irVersion: IrVersion::V2,
                },
            )
            .unwrap();
            let structure = serde_json::to_value(&compiled).unwrap();
            if let Some(previous) = previous {
                assert_eq!(structure, previous);
            }
            previous = Some(structure);
            let result = serde_json::to_value(
                compiled
                    .check(SafetyBounds {
                        max_depth: 8,
                        max_states: 100,
                        max_work: 10000,
                        max_temporal_nodes: None,
                    })
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(
                result,
                serde_json::json!({
                    "status": "complete",
                    "reason": null,
                    "states": 1,
                    "generated": 2,
                    "depth": 0,
                    "work": work,
                    "violation": null
                })
            );
            assert_eq!(result["states"], stuttering_control["states"]);
            assert_eq!(result["generated"], stuttering_control["generated"]);
        }
    }
}

fn module_corpus_case(name: &str) -> serde_json::Value {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("modules-support.json")).unwrap();
    cases
        .into_iter()
        .find(|case| case["name"] == name)
        .unwrap_or_else(|| panic!("{name} is in the module corpus"))
}

fn compile_corpus_case(
    case: &serde_json::Value,
    version: IrVersion,
) -> Result<rtlc::SafetyProgram, CompileError> {
    let module = rtla::parse_module(case["source"].as_str().unwrap()).unwrap();
    let config = rtla::parse_config(case["config"].as_str().unwrap()).unwrap();
    let modules = case["modules"]
        .as_object()
        .unwrap()
        .values()
        .map(|source| rtla::parse_module(source.as_str().unwrap()).unwrap())
        .collect::<Vec<_>>();
    let options = CompileOptions { irVersion: version };
    if modules.is_empty() {
        compile_with_options(&module, &config, options)
    } else {
        compile_modules_with_options(&module, &config, &modules, options)
    }
}

macro_rules! linking_case {
    ($v1:ident, $v2:ident, $case:literal) => {
        #[test]
        fn $v1() {
            let program = compile_corpus_case(&module_corpus_case($case), IrVersion::V1)
                .unwrap_or_else(|error| panic!("{}: {error}", $case));
            assert_eq!(serde_json::to_value(&program).unwrap()["version"], 1);
        }

        #[test]
        fn $v2() {
            let program = compile_corpus_case(&module_corpus_case($case), IrVersion::V2)
                .unwrap_or_else(|error| panic!("{}: {error}", $case));
            assert_eq!(serde_json::to_value(&program).unwrap()["version"], 2);
        }
    };
}
linking_case!(
    v1_compiles_instances_with_substitutions,
    v2_compiles_instances_with_substitutions,
    "instance-twice"
);
linking_case!(
    v1_compiles_configuration_operator_substitution,
    v2_compiles_configuration_operator_substitution,
    "constant-operator-override"
);

#[test]
fn v2_module_corpus_keeps_valid_models_and_invalid_input_diagnostics() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("modules-support.json")).unwrap();
    let mut accepted = 0;
    let mut refused = 0;
    for case in &cases {
        let name = case["name"].as_str().unwrap();
        let first = compile_corpus_case(case, IrVersion::V1);
        let second = compile_corpus_case(case, IrVersion::V2);
        match (first, second) {
            (Ok(first), Ok(second)) => {
                assert_ne!(case["expected"], "unsupported", "{name}");
                let bounds = SafetyBounds {
                    max_depth: 100,
                    max_states: 1000,
                    max_work: 1000000,
                    max_temporal_nodes: None,
                };
                let first = first.check(bounds);
                let second = second.check(bounds);
                if case["expected"] == "model-error" {
                    let first = first.unwrap_err();
                    let second = second.unwrap_err();
                    assert_eq!(first.code, second.code, "{name}");
                    assert!(first.message.contains("CASE with no conditions true"));
                    assert!(second.message.contains("CASE with no conditions true"));
                    accepted += 1;
                    continue;
                }
                let first = first.unwrap();
                let second = second.unwrap();
                assert_eq!(first.status, case["expected"].as_str().unwrap(), "{name}");
                assert_eq!(first.status, second.status, "{name}");
                assert_eq!(first.states, second.states, "{name}");
                assert_eq!(first.generated, second.generated, "{name}");
                accepted += 1;
            }
            (Err(first), Err(second)) => {
                assert_eq!(case["expected"], "unsupported", "{name}");
                assert_eq!(first, second, "{name}");
                refused += 1;
            }
            (first, second) => panic!("{name}: {first:?} != {second:?}"),
        }
    }
    assert!(accepted > 0 && refused > 0);
}
