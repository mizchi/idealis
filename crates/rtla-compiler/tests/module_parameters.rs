use rtla_compiler::{CompileOptions, IrVersion, compile_modules_with_options};
use rtlc::SafetyBounds;

fn counter() -> rtla::ast::Module {
    rtla::parse_module("---- MODULE Counter ----\nEXTENDS Naturals\nCONSTANT Limit\nVARIABLE value\nInit == value=0\nNext == value'=IF value<Limit THEN value+1 ELSE 0\nSafe == value \\in 0..Limit\n====").unwrap()
}

#[test]
fn parameterized_instances_keep_qualifier_arguments_separate_from_operator_arguments() {
    let module = rtla::parse_module("---- MODULE Root ----\nEXTENDS Naturals\nVARIABLE x\nC(limit) == INSTANCE Counter WITH Limit <- limit, value <- x\nInit == C(2)!Init\nNext == C(2)!Next\nSafe == C(2)!Safe\n====").unwrap();
    let config =
        rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE").unwrap();
    for version in [IrVersion::V1, IrVersion::V2] {
        let program = compile_modules_with_options(
            &module,
            &config,
            &[counter()],
            CompileOptions { irVersion: version },
        )
        .unwrap();
        let result = program
            .check(SafetyBounds {
                max_depth: 100,
                max_states: 1000,
                max_work: 1000000,
                max_temporal_nodes: None,
            })
            .unwrap();
        assert_eq!(result.status, "complete");
        assert_eq!(result.states, 3);
        for next in ["C!Next(2)", "C(1,2)!Next"] {
            let mut invalid = module.clone();
            invalid
                .definitions
                .iter_mut()
                .find(|definition| definition.name == "Next")
                .unwrap()
                .body = rtla::parse_expression(next).unwrap();
            assert!(
                compile_modules_with_options(
                    &invalid,
                    &config,
                    &[counter()],
                    CompileOptions { irVersion: version }
                )
                .is_err(),
                "{next}"
            );
        }
    }
}

#[test]
fn let_instances_capture_the_lexical_bound_values() {
    let module = rtla::parse_module("---- MODULE Root ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == \\E delta \\in 1..2: LET C(limit) == INSTANCE Counter WITH Limit <- limit, value <- x IN C(delta)!Next\nSafe == x \\in 0..2\n====").unwrap();
    let config =
        rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE").unwrap();
    for version in [IrVersion::V1, IrVersion::V2] {
        let program = compile_modules_with_options(
            &module,
            &config,
            &[counter()],
            CompileOptions { irVersion: version },
        )
        .unwrap();
        let result = program
            .check(SafetyBounds {
                max_depth: 100,
                max_states: 1000,
                max_work: 1000000,
                max_temporal_nodes: None,
            })
            .unwrap();
        assert_eq!(result.status, "complete");
        assert_eq!(result.states, 3);
    }
}

#[test]
fn nested_modules_capture_only_the_surrounding_symbols_in_scope() {
    let module = rtla::parse_module("---- MODULE Root ----\nEXTENDS Naturals\nCONSTANT N\nVARIABLE x\n---- MODULE Inner ----\nInit == x=0\nNext == x'=IF x<N THEN x+1 ELSE 0\n====\nI == INSTANCE Inner\nInit == I!Init\nNext == I!Next\nSafe == x \\in 0..N\n====").unwrap();
    let config = rtla::parse_config(
        "CONSTANT N=2\nINIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    )
    .unwrap();
    for version in [IrVersion::V1, IrVersion::V2] {
        let program = compile_modules_with_options(
            &module,
            &config,
            &[],
            CompileOptions { irVersion: version },
        )
        .unwrap();
        let result = program
            .check(SafetyBounds {
                max_depth: 100,
                max_states: 1000,
                max_work: 1000000,
                max_temporal_nodes: None,
            })
            .unwrap();
        assert_eq!(result.status, "complete");
        assert_eq!(result.states, 3);
    }
}

#[test]
fn instance_operator_parameters_accept_named_symbolic_and_lambda_replacements() {
    let dependency = rtla::parse_module(
        "---- MODULE Transform ----\nCONSTANT F(_)\nVARIABLE value\nNext == value'=F(value)\n====",
    )
    .unwrap();
    for instance in [
        "I == INSTANCE Transform WITH F <- Inc, value <- x",
        "I == INSTANCE Transform WITH F <- LAMBDA value: (value+1)%3, value <- x",
    ] {
        let source = format!(
            "---- MODULE Root ----\nEXTENDS Naturals\nVARIABLE x\nInc(value) == (value+1)%3\n{instance}\nInit == x=0\nNext == I!Next\nSafe == x \\in 0..2\n===="
        );
        let module = rtla::parse_module(&source).unwrap();
        let config =
            rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE")
                .unwrap();
        for version in [IrVersion::V1, IrVersion::V2] {
            let program = compile_modules_with_options(
                &module,
                &config,
                std::slice::from_ref(&dependency),
                CompileOptions { irVersion: version },
            )
            .unwrap_or_else(|error| panic!("{instance}: {error}"));
            let result = program
                .check(SafetyBounds {
                    max_depth: 100,
                    max_states: 1000,
                    max_work: 1000000,
                    max_temporal_nodes: None,
                })
                .unwrap();
            assert_eq!(result.status, "complete");
            assert_eq!(result.states, 3);
        }
    }
}

#[test]
fn lexical_outer_symbols_are_not_instance_parameters() {
    let source = "---- MODULE Root ----\nCONSTANT N\nVARIABLE x\n---- MODULE Inner ----\nValue == N\n====\nI == INSTANCE Inner WITH N <- 1\nInit == x=I!Value\nNext == UNCHANGED x\n====";
    let module = rtla::parse_module(source).unwrap();
    let config =
        rtla::parse_config("CONSTANT N=2\nINIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE").unwrap();
    for version in [IrVersion::V1, IrVersion::V2] {
        assert!(
            compile_modules_with_options(
                &module,
                &config,
                &[],
                CompileOptions { irVersion: version }
            )
            .is_err()
        );
    }
}

#[test]
fn parameterized_instances_preserve_implicit_recursive_functions() {
    let module = rtla::parse_module("---- MODULE Test ----\nEXTENDS Naturals\nVARIABLE x\nI(limit) == INSTANCE Functions WITH Limit <- limit\nInit == x = I(5)!Factorial[5]\nNext == UNCHANGED x\nSafe == x=120\n====").unwrap();
    let dependency = rtla::parse_module("---- MODULE Functions ----\nEXTENDS Naturals\nCONSTANT Limit\nFactorial[value \\in 0..Limit] == IF value=0 THEN 1 ELSE value*Factorial[value-1]\n====").unwrap();
    let config =
        rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE").unwrap();
    let program = rtla_compiler::compile_modules_with_options(
        &module,
        &config,
        &[dependency],
        CompileOptions {
            irVersion: IrVersion::V2,
        },
    )
    .unwrap();
    let result = program
        .check(SafetyBounds {
            max_depth: 100,
            max_states: 1000,
            max_work: 1000000,
            max_temporal_nodes: None,
        })
        .unwrap();
    assert_eq!(result.status, "complete");
    assert_eq!(result.states, 1);
}
