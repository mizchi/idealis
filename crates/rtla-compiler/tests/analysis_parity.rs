use rtla_compiler::{CompileOptions, IrVersion, analysis_roles, compile_with_options};
use rtlc::{SafetyBounds, SafetyProgram};
use serde_json::json;

fn source(declarations: &str, safe: &str) -> String {
    format!(
        "---- MODULE AnalysisParity ----\nEXTENDS Naturals, Integers, Sequences\nVARIABLE x\n{declarations}\nInit == x = 0\nNext == UNCHANGED x\nSafe == {safe}\n===="
    )
}

fn compile(source: &str) -> Result<SafetyProgram, rtla_compiler::CompileError> {
    let module = rtla::parse_module(source).unwrap();
    let config =
        rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE").unwrap();
    compile_with_options(
        &module,
        &config,
        CompileOptions {
            irVersion: IrVersion::V2,
        },
    )
}

fn bounds() -> SafetyBounds {
    SafetyBounds {
        max_depth: 8,
        max_states: 100,
        max_work: 10000,
        max_temporal_nodes: None,
    }
}

fn complete(source: &str) {
    let program = compile(source);
    assert!(program.is_ok(), "expected accepted v2 program: {program:?}");
    let result = serde_json::to_value(program.unwrap().check(bounds()).unwrap()).unwrap();
    assert_eq!(result["status"], "complete");
    assert_eq!(result["states"], 1);
    assert_eq!(result["generated"], 2);
    assert_eq!(result["depth"], 0);
    assert_eq!(result["reason"], json!(null));
    assert_eq!(result["violation"], json!(null));
}

#[test]
fn analysis_parity_role_table_matches_independent_expectation() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/tlc-operator-sharing/analysis-roles.json"
    ))
    .unwrap();
    let mut expected: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
    let additions: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../fixtures/tlc-operator-sharing/analysis-rust-roles.json"
    ))
    .unwrap();
    expected.extend(additions);
    expected.sort_by_key(|row| {
        ["space", "constructor", "operatorClass", "position"]
            .map(|key| row[key].as_str().unwrap().to_string())
    });
    assert_eq!(analysis_roles(), serde_json::json!(expected));
}

#[test]
fn analysis_parity_choose_domain_free_old_is_captured_when_unused() {
    let input = source(
        "",
        "[<<{9}>> EXCEPT ![1] = LET Pick == CHOOSE n \\in @: TRUE IN TRUE][1]",
    );
    complete(&input);
}

#[test]
fn analysis_parity_choose_domain_free_old_is_captured_when_used() {
    let input = source(
        "",
        "[<<{9}>> EXCEPT ![1] = LET Pick == CHOOSE n \\in @: TRUE IN Pick = 9][1]",
    );
    complete(&input);
}

#[test]
fn analysis_parity_choose_predicate_free_old_is_captured() {
    let input = source(
        "",
        "[<<9>> EXCEPT ![1] = LET Pick == CHOOSE n \\in {9}: @ = n IN TRUE][1]",
    );
    complete(&input);
}

#[test]
fn analysis_parity_notin_formal_membership_is_preserved() {
    complete(&source("F(p) == x \\notin p", "~F(Nat)"));
}

#[test]
fn analysis_parity_notin_capture_membership_is_preserved() {
    complete(&source("F(p) == LET G == x \\notin p IN G", "~F(Nat)"));
}

#[test]
fn analysis_parity_notin_function_head_membership_is_preserved() {
    complete(&source(
        "F(p) == LET f[i \\in {0}] == i \\notin p IN f[0]",
        "~F(Nat)",
    ));
}

#[test]
fn analysis_parity_notin_function_construction_membership_is_preserved() {
    complete(&source(
        "F(p) == LET f == [i \\in {0} |-> i \\notin p] IN f[0]",
        "~F(Nat)",
    ));
}

#[test]
fn analysis_parity_case_condition_is_value_position() {
    complete(&source(
        "F(p) == ENABLED (CASE UNCHANGED p -> UNCHANGED x [] OTHER -> UNCHANGED x)",
        "F(0)",
    ));
}

#[test]
fn analysis_parity_choose_caller_binder_controls() {
    for (declarations, safe) in [
        (
            "D(p) == CHOOSE n \\in p: TRUE",
            "\\A n \\in {9}: D({n}) = n",
        ),
        (
            "P(p) == CHOOSE n \\in {4}: p",
            "\\A n \\in {9}: P(n = 9) = 4",
        ),
    ] {
        complete(&source(declarations, safe));
    }
}

#[test]
fn analysis_parity_choose_caller_old_actual_controls() {
    for safe in [
        "[<<9>> EXCEPT ![1] = I(CHOOSE n \\in {@}: TRUE)][1] = 9",
        "[<<9>> EXCEPT ![1] = I(CHOOSE n \\in {9}: n = @)][1] = 9",
    ] {
        complete(&source("I(p) == [<<4>> EXCEPT ![1] = p][1]", safe));
    }
}

#[test]
fn analysis_parity_choose_empty_and_no_witness_are_evaluation_errors() {
    for choose in ["CHOOSE n \\in {}: TRUE", "CHOOSE n \\in {9}: FALSE"] {
        let program = compile(&source("", &format!("({choose}) = 9"))).unwrap();
        assert_eq!(
            program.check(bounds()).unwrap_err().message,
            "CHOOSE: no element of the domain satisfied the predicate"
        );
    }
}

#[test]
fn analysis_parity_choose_lazy_and_unused_actual_are_not_evaluated() {
    for (declarations, safe) in [
        ("", "IF TRUE THEN TRUE ELSE (CHOOSE n \\in {}: TRUE) = 9"),
        ("I(p) == TRUE", "I(CHOOSE n \\in {}: TRUE)"),
    ] {
        complete(&source(declarations, safe));
    }
}

#[test]
fn analysis_parity_function_domain_free_old_is_captured() {
    for definition in ["f[i \\in @] == i", "f == [i \\in @ |-> i]"] {
        let input = source(
            "",
            &format!("[<<{{9}}>> EXCEPT ![1] = LET {definition} IN TRUE][1]"),
        );
        complete(&input);
    }
}

fn normalize(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => values.iter_mut().for_each(normalize),
        serde_json::Value::Object(fields) => {
            fields.retain(|key, value| {
                !(key == "subscript" && value.is_null()
                    || matches!(
                        key.as_str(),
                        "fairness" | "initialProperties" | "actionProperties" | "properties"
                    ) && value.as_array().is_some_and(Vec::is_empty))
            });
            fields.values_mut().for_each(normalize);
        }
        _ => {}
    }
}

fn matches_node(value: &serde_json::Value, expected: &serde_json::Value) -> bool {
    match expected {
        serde_json::Value::Object(fields) => fields.iter().all(|(key, entry)| {
            value
                .get(key)
                .is_some_and(|value| matches_node(value, entry))
        }),
        serde_json::Value::Array(entries) => value.as_array().is_some_and(|values| {
            values.len() == entries.len()
                && values
                    .iter()
                    .zip(entries)
                    .all(|(value, entry)| matches_node(value, entry))
        }),
        _ => value == expected,
    }
}

fn contains_node(value: &serde_json::Value, expected: &serde_json::Value) -> bool {
    matches_node(value, expected)
        || match value {
            serde_json::Value::Object(fields) => {
                fields.values().any(|value| contains_node(value, expected))
            }
            serde_json::Value::Array(values) => {
                values.iter().any(|value| contains_node(value, expected))
            }
            _ => false,
        }
}

fn call_actual_case(id: &str) {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/tlc-operator-sharing/analysis-call-actual-cases.json"
    ))
    .unwrap();
    let entry = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == id)
        .unwrap();
    let result = compile(entry["input"]["source"].as_str().unwrap());
    if entry["expected"]["kind"] == "error" {
        let message = entry["expected"]["message"]
            .as_str()
            .unwrap()
            .strip_prefix("Unsupported TLA input: ")
            .unwrap();
        assert_eq!(
            result.map(|_| ()),
            Err(rtla_compiler::CompileError {
                message: message.into()
            }),
            "{id}: native refusal"
        );
        return;
    }
    let program = result.unwrap_or_else(|error| panic!("{id}: expected accepted v2: {error:?}"));
    let mut ir = serde_json::to_value(&program).unwrap();
    normalize(&mut ir);
    assert_eq!(ir["version"], 2, "{id}");
    for assertion in entry["assertions"].as_array().unwrap() {
        let pointer = assertion["pointer"].as_str().unwrap();
        assert_eq!(
            ir.pointer(pointer).unwrap(),
            &assertion["value"],
            "{id}: native {pointer}"
        );
    }
    for node in entry["contains"].as_array().unwrap() {
        assert!(contains_node(&ir, node), "{id}: expected IR node {node}");
    }
    for assertion in entry["absent"].as_array().unwrap() {
        let pointer = assertion["pointer"].as_str().unwrap();
        assert!(
            !contains_node(ir.pointer(pointer).unwrap(), &assertion["node"]),
            "{id}: unexpected IR node {assertion}"
        );
    }
    let checked = serde_json::to_value(program.check(bounds()).unwrap()).unwrap();
    assert_eq!(checked["status"], "complete", "{id}");
    assert_eq!(checked["reason"], json!(null), "{id}");
    assert_eq!(checked["states"], 1, "{id}");
    assert_eq!(checked["generated"], 2, "{id}");
    assert_eq!(checked["depth"], 0, "{id}");
    assert_eq!(checked["violation"], json!(null), "{id}");
}

macro_rules! call_actual_test {
    ($name:ident, $id:literal) => {
        #[test]
        fn $name() {
            call_actual_case($id);
        }
    };
}

call_actual_test!(
    analysis_parity_call_actual_membership_unused_nat,
    "membership-unused-nat"
);
call_actual_test!(
    analysis_parity_call_actual_membership_unused_finite,
    "membership-unused-finite"
);
call_actual_test!(
    analysis_parity_call_actual_membership_demanded_nat,
    "membership-demanded-nat"
);
call_actual_test!(
    analysis_parity_call_actual_membership_literal_control,
    "membership-literal-control"
);
call_actual_test!(
    analysis_parity_call_actual_target_unused_literal,
    "target-unused-literal"
);
call_actual_test!(
    analysis_parity_call_actual_target_demanded_state,
    "target-demanded-state"
);
call_actual_test!(
    analysis_parity_call_actual_target_demanded_literal_refusal,
    "target-demanded-literal-refusal"
);
call_actual_test!(
    analysis_parity_call_actual_generative_unused_actual,
    "generative-unused-actual"
);
call_actual_test!(
    analysis_parity_call_actual_generative_demanded_state,
    "generative-demanded-state"
);
call_actual_test!(
    analysis_parity_call_actual_generative_demanded_literal_refusal,
    "generative-demanded-literal-refusal"
);
call_actual_test!(
    analysis_parity_call_actual_free_old_unused_free,
    "free-old-unused-free"
);
call_actual_test!(
    analysis_parity_call_actual_free_old_unused_bound,
    "free-old-unused-bound"
);
call_actual_test!(
    analysis_parity_call_actual_free_old_demanded_free,
    "free-old-demanded-free"
);
call_actual_test!(
    analysis_parity_call_actual_free_old_demanded_bound,
    "free-old-demanded-bound"
);
