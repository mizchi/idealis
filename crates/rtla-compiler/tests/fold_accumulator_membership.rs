use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;

const BOUNDS: SafetyBounds = SafetyBounds {
    max_depth: 10,
    max_states: 100,
    max_work: 1000000,
    max_temporal_nodes: None,
};

const VERSIONS: [IrVersion; 2] = [IrVersion::V1, IrVersion::V2];

fn check(source: &str, version: IrVersion) -> Result<String, String> {
    let module = rtla::parse_module(source).unwrap();
    let config =
        rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE").unwrap();
    let program = compile_with_options(&module, &config, CompileOptions { irVersion: version })
        .map_err(|error| error.to_string())?;
    program
        .check(BOUNDS)
        .map(|result| result.status.to_string())
        .map_err(|error| error.message)
}

fn model(definitions: &str, next: &str, safe: &str) -> String {
    format!(
        "---- MODULE M ----\nEXTENDS Integers, Sequences, FiniteSets, Apalache\nVARIABLE x\n{definitions}\nInit == x = {{}}\nNext == x' = {next}\nSafe == {safe}\n===="
    )
}

fn assert_complete(source: &str) {
    for version in VERSIONS {
        assert_eq!(
            check(source, version),
            Ok("complete".into()),
            "{version:?}\n{source}"
        );
    }
}

#[test]
fn sequence_fold_callback_tests_membership_in_the_accumulator() {
    assert_complete(&model(
        "Lam(acc, u) == IF 1 \\in acc THEN acc ELSE acc \\union {u}",
        "ApaFoldSeqLeft(Lam, {}, <<1, 2>>)",
        "x \\in {{}, {1}} /\\ Cardinality(x) <= 1",
    ));
}

#[test]
fn set_fold_callback_tests_membership_in_the_accumulator() {
    assert_complete(&model(
        "Lam(acc, u) == IF 1 \\in acc THEN acc ELSE acc \\union {u}",
        "ApaFoldSet(Lam, {1}, {2, 3})",
        "x \\in {{}, {1}}",
    ));
}

#[test]
fn membership_in_the_accumulator_decides_the_fold_result() {
    assert_complete(&model(
        "Lam(acc, u) == IF 1 \\in acc THEN acc ELSE acc \\union {u}",
        "ApaFoldSeqLeft(Lam, {}, <<2, 1, 3>>)",
        "x \\in {{}, {2, 1}}",
    ));
}

#[test]
fn filter_over_a_constant_domain_tests_membership_in_the_accumulator() {
    assert_complete(&model(
        "Lam(acc, u) == acc \\union {e \\in {1, 2, 3} : e - 1 \\in acc}",
        "ApaFoldSeqLeft(Lam, {1}, <<0, 0>>)",
        "x \\in {{}, {1, 2, 3}}",
    ));
}

#[test]
fn callback_keeps_the_arguments_of_its_enclosing_operator() {
    assert_complete(&model(
        "Outer(n, s) == LET Lam(acc, u) == acc \\union (IF n \\in acc THEN s ELSE {u}) IN ApaFoldSeqLeft(Lam, {1}, <<7>>)",
        "Outer(1, {5})",
        "x \\in {{}, {1, 5}}",
    ));
}

#[test]
fn callback_without_membership_is_unchanged() {
    assert_complete(&model(
        "Lam(acc, u) == IF Cardinality(acc) > 0 THEN acc ELSE acc \\union {u}",
        "ApaFoldSeqLeft(Lam, {}, <<1, 2>>)",
        "x \\in {{}, {1}}",
    ));
}

#[test]
fn lambda_callback_tests_membership_in_the_accumulator() {
    assert_complete(&model(
        "",
        "ApaFoldSeqLeft(LAMBDA acc, u: IF 1 \\in acc THEN acc ELSE acc \\union {u}, {}, <<1, 2>>)",
        "x \\in {{}, {1}}",
    ));
}

#[test]
fn callback_passes_the_accumulator_to_a_membership_operator() {
    assert_complete(&model(
        "Has(s, e) == e \\in s\nLam(acc, u) == IF Has(acc, 1) THEN acc ELSE acc \\union {u}",
        "ApaFoldSeqLeft(Lam, {}, <<1, 2>>)",
        "x \\in {{}, {1}}",
    ));
}

#[test]
fn wrong_invariant_over_a_membership_fold_is_a_violation() {
    for version in VERSIONS {
        let source = model(
            "Lam(acc, u) == IF 1 \\in acc THEN acc ELSE acc \\union {u}",
            "ApaFoldSeqLeft(Lam, {}, <<1, 2>>)",
            "x = {}",
        );
        assert_eq!(
            check(&source, version),
            Ok("invariant-violation".into()),
            "{version:?}"
        );
    }
}
