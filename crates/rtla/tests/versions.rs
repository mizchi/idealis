use rtla::{dispatch, parse_expression};
use serde_json::{Value, json};

fn version_expression(source: &str, version: &str) -> Value {
    serde_json::from_slice(&dispatch(
        &serde_json::to_vec(&json!({"kind":"expression","source":source,"syntaxVersion":version}))
            .unwrap(),
    ))
    .unwrap()
}

#[test]
fn explicit_syntax_versions_keep_different_prefix_binding_without_changing_default() {
    let stable = version_expression("SUBSET 1..2", "1.7.4");
    let modern = version_expression("SUBSET 1..2", "1.8.0");
    assert_eq!(stable["kind"], "expression");
    assert_eq!(modern["kind"], "expression");
    assert_eq!(stable["expression"]["operator"], "..");
    assert_eq!(stable["expression"]["args"][0]["operator"], "SUBSET");
    assert_eq!(modern["expression"]["operator"], "SUBSET");
    assert_eq!(modern["expression"]["args"][0]["operator"], "..");
    assert_eq!(
        serde_json::to_value(parse_expression("SUBSET 1..2").unwrap()).unwrap(),
        stable["expression"]
    );
    assert_eq!(version_expression("DOMAIN f + n", "1.7.4")["kind"], "error");
    assert_eq!(
        version_expression("DOMAIN f + n", "1.8.0")["kind"],
        "expression"
    );
    assert_eq!(version_expression("TRUE", "2.0")["kind"], "error");
}
