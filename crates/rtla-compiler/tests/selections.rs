use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;

#[test]
fn subexpression_values_and_operator_arguments_select_the_tlc_operand() {
    for (definition, expression, expected) in [
        ("Op == 1+2", "Op!2", 2),
        ("Op(a) == IF a=0 THEN 10 ELSE 20", "Op(0)!2", 10),
        (
            "Op(a) == \\A bound \\in 0..2: label(bound) :: a+bound",
            "Op(1)!label(2)",
            3,
        ),
        ("Op == \\A bound \\in 0..2: bound+1", "Op!(2)", 3),
        (
            "Op == \\A bound \\in 0..2: bound+1\nApply(F(_), value) == F(value)",
            "Apply(Op!@, 2)",
            3,
        ),
        ("Op == CASE TRUE -> 3 [] OTHER -> 4", "Op!1!2", 3),
        (
            "Op == LET Inner(value) == value+1 IN TRUE",
            "Op!:!Inner(2)",
            3,
        ),
        ("Op == [first |-> 1, second |-> 3]", "Op!2", 3),
    ] {
        let source = format!(
            "---- MODULE Test ----\nEXTENDS Naturals\nVARIABLE x\n{definition}\nInit == x = {expression}\nNext == UNCHANGED x\nSafe == x = {expected}\n===="
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
            assert_eq!(result.states, 1, "{source}");
        }
    }
}

#[test]
fn selected_actions_keep_the_source_operator_name() {
    let module=rtla::parse_module("---- MODULE Test ----\nEXTENDS Naturals\nVARIABLE x\nOp(value) == label:: x'=value\nInit == x=0\nNext == Op((x+1)%3)!label\nSafe == x=0\n====").unwrap();
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
        assert_eq!(result["violation"]["trace"][1]["action"], "Op");
    }
}
