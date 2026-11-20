use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;

#[test]
fn user_operator_arguments_preserve_lexical_captures_in_both_ir_versions() {
    for (definitions, next) in [
        (
            "Apply(F(_), value) == F(value)\nInc(value) == (value+1)%3",
            "x' = Apply(Inc, x)",
        ),
        (
            "Apply(F(_), value) == F(value)",
            "\\E delta \\in 1..2: x' = Apply(LAMBDA value: (value+delta)%3, x)",
        ),
        (
            "Apply(F(_), value) == F(value)\nForward(G(_), value) == Apply(G, value)",
            "x' = Forward(LAMBDA value: (value+1)%3, x)",
        ),
        (
            "Combine(F(_,_), left, right) == F(left, right)",
            "x' = Combine(+, x, 1)%3",
        ),
        (
            "Run(A(_), value) == A(value)\nAssign(value) == x' = value",
            "Run(Assign, (x+1)%3)",
        ),
    ] {
        let source = format!(
            "---- MODULE Test ----\nEXTENDS Naturals\nVARIABLE x\n{definitions}\nInit == x=0\nNext == {next}\nSafe == x \\in 0..2\n===="
        );
        let module = rtla::parse_module(&source).unwrap();
        let config =
            rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE")
                .unwrap();
        for version in [IrVersion::V1, IrVersion::V2] {
            let program =
                compile_with_options(&module, &config, CompileOptions { irVersion: version })
                    .unwrap_or_else(|error| panic!("{version:?}: {source}: {error}"));
            let result = program
                .check(SafetyBounds {
                    max_depth: 100,
                    max_states: 1000,
                    max_work: 1000000,
                    max_temporal_nodes: None,
                })
                .unwrap();
            assert_eq!(result.status, "complete", "{source}");
            assert_eq!(result.states, 3, "{source}");
        }
    }
}

#[test]
fn lambdas_and_forwarded_operators_work_in_standard_callbacks_and_except_arguments() {
    for (definitions, initial) in [
        (
            "Apply(F(_), value) == F(value)\nUpdate(value) == [<<1>> EXCEPT ![1] = @ + value]",
            "[<<2>> EXCEPT ![1] = Apply(Update, @)[1]]",
        ),
        (
            "Filter(P(_), seq) == SelectSeq(seq, P)",
            "Filter(LAMBDA value: value > 1, <<1,2,3>> )",
        ),
        ("", "SelectSeq(<<1,2,3>>, LAMBDA value: value > 1)"),
        ("", "SortSeq(<<3,1,2>>, LAMBDA left, right: left <= right)"),
    ] {
        let module = rtla::parse_module(&format!("---- MODULE Test ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLE x\n{definitions}\nInit == x = {initial}\nNext == UNCHANGED x\nSafe == x = <<2,3>> \\/ x = <<1,2,3>> \\/ x = <<3>>\n====")).unwrap();
        let config =
            rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE")
                .unwrap();
        for version in [IrVersion::V1, IrVersion::V2] {
            if version == IrVersion::V1 && initial.contains("Apply(Update, @)") {
                assert!(
                    compile_with_options(&module, &config, CompileOptions { irVersion: version })
                        .unwrap_err()
                        .message
                        .contains("EXCEPT @ arguments across a nested EXCEPT require safety IR v2")
                );
                continue;
            }
            let program =
                compile_with_options(&module, &config, CompileOptions { irVersion: version })
                    .unwrap_or_else(|error| panic!("{version:?}: {initial}: {error}"));
            let result = program
                .check(SafetyBounds {
                    max_depth: 100,
                    max_states: 1000,
                    max_work: 1000000,
                    max_temporal_nodes: None,
                })
                .unwrap();
            assert_eq!(result.status, "complete", "{initial}");
        }
    }
}

#[test]
fn grouped_bound_names_keep_distinct_lexical_capture_identities() {
    for safe in [
        "\\A left,right \\in {1,2}: LET Add(value) == left+right+value IN Add(0)=left+right",
        "\\A left,right \\in {1,2}: Apply(LAMBDA value: left+right+value,0)=left+right",
    ] {
        let module = rtla::parse_module(&format!("---- MODULE Test ----\nEXTENDS Naturals\nVARIABLE x\nApply(F(_),value) == F(value)\nInit == x=0\nNext == UNCHANGED x\nSafe == {safe}\n====")).unwrap();
        let config =
            rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE")
                .unwrap();
        for version in [IrVersion::V1, IrVersion::V2] {
            let program =
                compile_with_options(&module, &config, CompileOptions { irVersion: version })
                    .unwrap();
            let result = program
                .check(SafetyBounds {
                    max_depth: 100,
                    max_states: 1000,
                    max_work: 1000000,
                    max_temporal_nodes: None,
                })
                .unwrap();
            assert_eq!(result.status, "complete", "{safe} {version:?}");
        }
    }
}

#[test]
fn specialized_actions_keep_public_names_in_counterexamples() {
    let module=rtla::parse_module("---- MODULE Test ----\nEXTENDS Naturals\nVARIABLE x\nRun(A(_),value) == A(value)\nAssign(value) == x'=value\nInit == x=0\nNext == Run(Assign,(x+1)%3)\nSafe == x=0\n====").unwrap();
    let config =
        rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE").unwrap();
    for version in [IrVersion::V1, IrVersion::V2] {
        let program =
            compile_with_options(&module, &config, CompileOptions { irVersion: version }).unwrap();
        let result = program
            .check(SafetyBounds {
                max_depth: 100,
                max_states: 1000,
                max_work: 1000000,
                max_temporal_nodes: None,
            })
            .unwrap();
        assert_eq!(result.status, "invariant-violation");
        let result = serde_json::to_value(result).unwrap();
        assert_eq!(
            result["violation"]["trace"][1]["action"],
            if version == IrVersion::V2 {
                "Run"
            } else {
                "Assign"
            }
        );
    }
}
