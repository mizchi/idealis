use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;

#[test]
fn declared_recursion_and_mutual_recursion_execute_as_v2_calls() {
    for (definitions, expression, expected) in [
        (
            "RECURSIVE Fact(_)\nFact(value) == IF value=0 THEN 1 ELSE value*Fact(value-1)",
            "Fact(5)",
            "120",
        ),
        (
            "RECURSIVE Even(_), Odd(_)\nEven(value) == IF value=0 THEN TRUE ELSE Odd(value-1)\nOdd(value) == IF value=0 THEN FALSE ELSE Even(value-1)",
            "IF Even(6) THEN 6 ELSE 7",
            "6",
        ),
        (
            "RECURSIVE Sum(_)\nSum(value) == IF value=0 THEN 0 ELSE value+Sum(value-1)",
            "IF x=3 THEN Sum(x) ELSE x",
            "6",
        ),
    ] {
        let source = format!(
            "---- MODULE Test ----\nEXTENDS Naturals\nVARIABLE x\n{definitions}\nInit == x=3\nNext == x' = {expression}\nSafe == x=3 \\/ x={expected}\n===="
        );
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
        .unwrap_or_else(|error| panic!("{source}: {error}"));
        let result = program
            .check(SafetyBounds {
                max_depth: 100,
                max_states: 1000,
                max_work: 1000000,
                max_temporal_nodes: None,
            })
            .unwrap();
        assert_eq!(result.status, "complete", "{source}");
        assert_eq!(result.states, 2, "{source}");
    }
}

#[test]
fn nonterminating_source_recursion_is_inconclusive() {
    let module = rtla::parse_module("---- MODULE Test ----\nVARIABLE x\nRECURSIVE Loop(_)\nLoop(value) == Loop(value)\nInit == x=0\nNext == x'=Loop(x)\n====").unwrap();
    let config = rtla::parse_config("INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE").unwrap();
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
            max_depth: 10,
            max_states: 1000,
            max_work: 1000000,
            max_temporal_nodes: None,
        })
        .unwrap();
    assert_eq!(result.status, "bounded");
    assert_eq!(result.reason.as_deref(), Some("evaluation"));
}

#[test]
fn recursive_functions_evaluate_requested_keys_and_domain_without_expanding_their_body() {
    for (definition, initial, invariant) in [
        (
            "Factorial[value \\in 0..5] == IF value=0 THEN 1 ELSE value*Factorial[value-1]",
            "Factorial[5]",
            "x=120",
        ),
        (
            "Fn[left \\in 0..2, right \\in 0..2] == IF left=0 THEN right ELSE Fn[left-1,right]+1",
            "Fn[2,1]",
            "x=3",
        ),
        (
            "Fn[<<left,right>> \\in {<<0,1>>, <<1,1>>, <<2,1>>}] == IF left=0 THEN right ELSE Fn[<<left-1,right>>]+1",
            "Fn[<<2,1>>]",
            "x=3",
        ),
        (
            "Fn[value \\in 0..1] == IF value=0 THEN 0 ELSE Fn[1]",
            "DOMAIN Fn",
            "x=0..1",
        ),
        (
            "Fn[value \\in 0..1] == IF value=0 THEN 0 ELSE Fn[1]",
            "Fn[0]",
            "x=0",
        ),
    ] {
        let source = format!(
            "---- MODULE Test ----\nEXTENDS Naturals\nVARIABLE x\n{definition}\nInit == x={initial}\nNext == UNCHANGED x\nSafe == {invariant}\n===="
        );
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
        .unwrap_or_else(|error| panic!("{source}: {error}"));
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

#[test]
fn recursive_boolean_operators_work_in_action_guards_and_temporal_atoms() {
    let module = rtla::parse_module("---- MODULE Test ----\nEXTENDS Naturals\nVARIABLE x\nRECURSIVE Even(_)\nEven(value) == IF value=0 THEN TRUE ELSE IF value=1 THEN FALSE ELSE Even(value-2)\nInit == x=0\nNext == Even(2*x) /\\ x'=(x+1)%3\nSafe == []Even(2*x)\n====").unwrap();
    let config =
        rtla::parse_config("INIT Init\nNEXT Next\nPROPERTY Safe\nCHECK_DEADLOCK FALSE").unwrap();
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
            max_depth: 100,
            max_states: 1000,
            max_work: 1000000,
            max_temporal_nodes: None,
        })
        .unwrap();
    assert_eq!(result.status, "complete");
    assert_eq!(result.states, 3);
}

#[test]
fn recursive_callbacks_can_be_used_by_folds_in_v2() {
    let module=rtla::parse_module("---- MODULE Test ----\nEXTENDS Naturals, Apalache\nVARIABLE x\nRECURSIVE Add(_,_)\nAdd(left,right) == IF right=0 THEN left ELSE Add(left+1,right-1)\nInit == x=ApaFoldSeqLeft(Add,0,<<1,2,3>>)\nNext == UNCHANGED x\nSafe == x=6\n====").unwrap();
    let config =
        rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE").unwrap();
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
            max_depth: 100,
            max_states: 1000,
            max_work: 1000000,
            max_temporal_nodes: None,
        })
        .unwrap();
    assert_eq!(result.status, "complete");
    assert_eq!(result.states, 1);
}
