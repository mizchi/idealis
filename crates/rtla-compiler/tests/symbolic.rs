use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;

#[test]
fn custom_symbolic_calls_work_in_both_ir_versions() {
    for (definition, next, extends) in [
        ("left %% right == ~left", "x %% TRUE", ""),
        ("-. value == ~value", "-x", ""),
        ("value ^+ == ~value", "x^+", ""),
        ("left %% right == ~left", "%%(x, TRUE)", ""),
        ("", "LET left %% right == ~left IN x %% TRUE", ""),
        ("left + right == ~left", "x + TRUE", "EXTENDS Sequences"),
    ] {
        let source = format!(
            "---- MODULE Symbolic ----\n{extends}\nVARIABLE x\n{definition}\nInit == x = FALSE\nNext == x' = ({next})\nSafe == x \\in BOOLEAN\n===="
        );
        let module = rtla::parse_module(&source).unwrap();
        let config =
            rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE")
                .unwrap();
        for version in [IrVersion::V1, IrVersion::V2] {
            let program =
                compile_with_options(&module, &config, CompileOptions { irVersion: version })
                    .unwrap();
            let result = program
                .check(SafetyBounds {
                    max_depth: 10,
                    max_states: 100,
                    max_work: 100000,
                    max_temporal_nodes: None,
                })
                .unwrap();
            assert_eq!(result.states, 2, "{source}");
            assert_eq!(serde_json::to_value(result).unwrap()["status"], "complete");
        }
    }
}

#[test]
fn symbolic_builtin_and_import_conflicts_are_rejected_even_when_unused() {
    for (extends, definition) in [
        ("", "left /\\ right == FALSE"),
        ("", "DOMAIN value == value"),
        ("", "value' == value"),
        ("", "left \\X right == left"),
        ("EXTENDS Naturals", "left + right == left"),
        ("EXTENDS Integers", "-. value == value"),
        ("EXTENDS Sequences", "left \\o right == left"),
        ("EXTENDS TLC", "left @@ right == left"),
        ("", "Unused == LET left /\\ right == FALSE IN TRUE"),
        ("", "Unused(_ /\\ _) == TRUE"),
    ] {
        let source = format!(
            "---- MODULE Symbolic ----\n{extends}\nVARIABLE x\n{definition}\nInit == x = 0\nNext == UNCHANGED x\n===="
        );
        let module = rtla::parse_module(&source).unwrap();
        let config = rtla::parse_config("INIT Init\nNEXT Next").unwrap();
        for version in [IrVersion::V1, IrVersion::V2] {
            assert!(
                compile_with_options(&module, &config, CompileOptions { irVersion: version })
                    .is_err(),
                "{source}"
            );
        }
    }
}
