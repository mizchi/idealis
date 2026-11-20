use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;

fn check(expression: &str, declarations: &str) -> Result<rtlc::SafetyResult, rtlc::CoreError> {
    let source = format!(
        "---- MODULE Lazy ----\nEXTENDS Naturals\nVARIABLE x\nF[i \\in 0..1023] == IF i=0 THEN 7 ELSE IF i=1 THEN 1 \\div 0 ELSE F[0]\nAlias == F\nId(f) == f\n{declarations}\nInit == x = ({expression})\nNext == UNCHANGED x\n===="
    );
    let module = rtla::parse_module(&source).unwrap();
    let config = rtla::parse_config("INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE").unwrap();
    compile_with_options(
        &module,
        &config,
        CompileOptions {
            irVersion: IrVersion::V2,
        },
    )
    .unwrap()
    .check(SafetyBounds {
        max_depth: 10,
        max_states: 100,
        max_work: 100_000,
        max_temporal_nodes: None,
    })
}

#[test]
fn aliases_actuals_and_selected_branches_do_not_force_unused_function_elements() {
    for expression in [
        "Alias[0]",
        "Id(F)[0]",
        "Id(Alias)[0]",
        "(IF TRUE THEN Alias ELSE [i \\in {} |-> 0])[0]",
        "(LET Captured == Alias IN Id(Captured))[0]",
        "DOMAIN Id(Alias)",
        "Id([fn |-> Alias]).fn[0]",
        "Id(<<Alias, TRUE>>)[1][0]",
    ] {
        let result = check(expression, "").unwrap_or_else(|error| panic!("{expression}: {error}"));
        assert_eq!(result.status, "complete", "{expression}");
        assert_eq!(result.states, 1);
    }
}

#[test]
fn demanded_elements_and_full_function_observation_still_report_errors() {
    for expression in ["Id(Alias)[1]", "Alias = Alias"] {
        assert!(check(expression, "").is_err(), "{expression}");
    }
    let error = check("Id(Alias)[1024]", "").unwrap_err();
    assert!(error.message.contains("outside its domain"), "{error}");
}
