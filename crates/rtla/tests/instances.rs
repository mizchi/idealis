use rtla::{parse_expression, parse_module};

#[test]
fn qualified_operators_and_instance_substitutions_are_preserved() {
    let source = "---- MODULE Root ----\nVARIABLE x\nA == INSTANCE Counter WITH value <- x, Limit <- 2\nLOCAL INSTANCE Helpers\nInit == A!Init\nNext == A!Next(1)\n====";
    let module = serde_json::to_value(parse_module(source).unwrap()).unwrap();
    assert_eq!(module["instances"][0]["name"], "A");
    assert_eq!(module["instances"][0]["module"], "Counter");
    assert_eq!(module["instances"][0]["substitutions"][0]["name"], "value");
    assert_eq!(module["instances"][1]["isLocal"], true);
    assert_eq!(module["definitions"][0]["body"]["name"], "A!Init");
    assert_eq!(module["definitions"][1]["body"]["operator"], "A!Next");
    let expression = serde_json::to_value(parse_expression("A!B!Apply(1)").unwrap()).unwrap();
    assert_eq!(expression["operator"], "A!B!Apply");
}

#[test]
fn duplicate_instance_substitutions_are_rejected() {
    assert!(
        parse_module(
            "---- MODULE Root ----\nA == INSTANCE Counter WITH Limit <- 1, Limit <- 2\n===="
        )
        .is_err()
    );
}

#[test]
fn symbolic_with_values_stop_at_the_next_module_unit() {
    for following in [
        "Next == TRUE",
        "Next(value) == value",
        "left %% right == left",
        "ASSUME TRUE",
        "THEOREM TRUE",
        "CONSTANT value",
        "RECURSIVE F(_)\nF(value) == value",
        "LOCAL Next == TRUE",
        "INSTANCE Other",
    ] {
        let source =
            format!("---- MODULE Test ----\nI == INSTANCE Symbols WITH ~ <- ~\n{following}\n====");
        let module =
            rtla::parse_module(&source).unwrap_or_else(|error| panic!("{source}: {error}"));
        assert!(
            matches!(&module.instances[0].substitutions[0].expression.node, rtla::ast::ExpressionKind::Identifier { name } if name == "~")
        );
    }
    for spelling in ["\\X", "\\times"] {
        assert!(
            rtla::parse_module(&format!(
                "---- MODULE Test ----\nI == INSTANCE Symbols WITH F <- {spelling}\n===="
            ))
            .is_err()
        );
    }
}
