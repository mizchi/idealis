use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};

fn compile(definitions: &str) -> Result<rtlc::SafetyProgram, rtla_compiler::CompileError> {
    let module = rtla::parse_module(&format!(
        "---- MODULE Levels ----\nEXTENDS Naturals\nVARIABLE x\n{definitions}\nInit == x=0\nNext == x'=x\n===="
    )).unwrap();
    let config = rtla::parse_config("INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE").unwrap();
    compile_with_options(
        &module,
        &config,
        CompileOptions {
            irVersion: IrVersion::V2,
        },
    )
}

#[test]
fn unused_definitions_still_obey_source_level_constraints() {
    for definitions in [
        "Bad == (x')'",
        "Bad == UNCHANGED (x')",
        "Bad == ENABLED ([](x=0))",
        "Bad == <<[](x=0)>>",
        "Bad == {[](x=0)}",
        "Bad == ([](x=0)) = TRUE",
        "Bad == [x'=x]_(x')",
        "Bad == WF_(x')(x'=x)",
        "P(a) == a'\nQ(b) == P(b)\nBad == Q(x')",
        "Bad == LET P(a) == a' IN P(x')",
        "P(a) == LET Unused == a' IN TRUE\nBad == P(x')",
        "RECURSIVE P(_)\nP(a) == IF TRUE THEN a ELSE P(a)\nBad == P([]TRUE)",
    ] {
        let error = compile(definitions).unwrap_err();
        assert!(error.message.contains("level"), "{definitions}: {error}");
    }
}

#[test]
fn level_weights_do_not_treat_ignored_actuals_as_result_dependencies() {
    for definitions in [
        "P(a) == TRUE\nGood == P([]TRUE)'",
        "P(a) == a'\nGood == P(x)",
        "P(a) == ENABLED a\nGood == P(x'=x)'",
        "Good == LET P(a) == a' IN P(x)",
        "RECURSIVE P(_)\nP(a) == IF TRUE THEN a ELSE P(a)\nGood == P(x')",
    ] {
        compile(definitions).unwrap_or_else(|error| panic!("{definitions}: {error}"));
    }
}
