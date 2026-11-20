use rtla::{parse_expression, parse_module};
use serde_json::json;

#[test]
fn misplaced_proof_and_module_syntax_is_a_syntax_error_not_an_unimplemented_feature() {
    for source in [
        "ASSUME TRUE PROVE TRUE",
        "INSTANCE Other",
        "PROOF OBVIOUS",
        "LAMBDA value: value",
    ] {
        assert_eq!(
            parse_expression(source).unwrap_err().kind,
            rtla::ErrorKind::Syntax,
            "{source}"
        );
    }
    assert_eq!(
        parse_module("---- MODULE Test ----\nPROOF OBVIOUS\n====")
            .unwrap_err()
            .kind,
        rtla::ErrorKind::Syntax
    );
}

#[test]
fn parameterized_labels_keep_bound_names_and_reject_expression_parameters() {
    let expression =
        serde_json::to_value(parse_expression("label(a, b) :: P(a, b)").unwrap()).unwrap();
    assert_eq!(expression["kind"], "Label");
    assert_eq!(expression["params"], json!(["a", "b"]));
    assert_eq!(expression["body"]["operator"], "P");
    for source in [
        "label((a)) :: TRUE",
        "label(a + b) :: TRUE",
        "label(A!b) :: TRUE",
    ] {
        assert!(parse_expression(source).is_err(), "{source}");
    }
}

#[test]
fn lambda_operator_arguments_keep_parameters_and_body() {
    let expression = serde_json::to_value(
        parse_expression("Map(LAMBDA value, index: value[index], seq)").unwrap(),
    )
    .unwrap();
    assert_eq!(expression["args"][0]["kind"], "Lambda");
    assert_eq!(expression["args"][0]["params"][0]["name"], "value");
    assert_eq!(expression["args"][0]["params"][1]["name"], "index");
    assert_eq!(expression["args"][0]["body"]["kind"], "FunctionApplication");
    assert!(parse_expression("LAMBDA F(_): F(1)").is_err());
    for source in [
        "LAMBDA value: value",
        "<<LAMBDA value: value>>",
        "F((LAMBDA value: value))",
    ] {
        assert!(parse_expression(source).is_err(), "{source}");
    }
}

#[test]
fn selectors_keep_navigation_after_operator_arguments() {
    let expression =
        serde_json::to_value(parse_expression("op(a, b)!<<!>>!3!(x, y)!:!@").unwrap()).unwrap();
    assert_eq!(expression["kind"], "Selection");
    assert_eq!(expression["expression"]["operator"], "op");
    let selectors = &expression["selectors"];
    assert_eq!(selectors[0]["kind"], "Left");
    assert_eq!(selectors[1]["kind"], "Right");
    assert_eq!(selectors[2], json!({"kind":"Index", "index":3}));
    assert_eq!(selectors[3]["kind"], "Arguments");
    assert_eq!(selectors[3]["args"].as_array().unwrap().len(), 2);
    assert_eq!(selectors[4]["kind"], "Body");
    assert_eq!(selectors[5]["kind"], "Bound");
    let qualified = serde_json::to_value(parse_expression("M!Op(1)!label(2)!1").unwrap()).unwrap();
    assert_eq!(qualified["expression"]["operator"], "M!Op");
    assert_eq!(qualified["selectors"][0]["name"], "label");
    assert_eq!(qualified["selectors"][0]["args"][0]["value"], 2);
}

#[test]
fn instance_substitutions_accept_symbolic_targets_and_operator_values() {
    let module =
        parse_module("---- MODULE Test ----\nI == INSTANCE Ops WITH + <- *, ~ <- ~\n====").unwrap();
    assert_eq!(module.instances[0].substitutions[0].name, "+");
    assert_eq!(module.instances[0].substitutions[1].name, "~");
}
