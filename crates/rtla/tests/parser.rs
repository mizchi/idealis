use rtla::ast::ExpressionKind;
use rtla::{ErrorKind, parse_config, parse_expression, parse_module};

#[test]
fn parses_apalache_assignment_at_tlc_comparison_precedence() {
    let expression = parse_expression("state' := state + 1").unwrap();
    let ExpressionKind::Apply { operator, args } = expression.node else {
        panic!("expected infix assignment")
    };
    assert_eq!(operator, ":=");
    assert!(matches!(&args[1].node, ExpressionKind::Apply { operator, .. } if operator == "+"));
    assert!(parse_expression("a := b = c").is_err());
}

#[test]
fn parses_upstream_channel_without_a_checker() {
    let module = parse_module(include_str!("fixtures/Channel.tla")).unwrap();
    assert_eq!(module.name, "Channel");
    assert_eq!(module.variables[0].name, "chan");
    assert_eq!(module.constants[0].name, "Data");
    assert_eq!(module.definitions.len(), 6);
    assert_eq!(module.theorems.len(), 1);
    assert_eq!(module.definitions[0].name, "TypeInvariant");
}

#[test]
fn preserves_nested_comments_strings_and_locations() {
    let expression =
        parse_expression("(* outer (* nested *) *)\nIF TRUE THEN \"a\\\"b\" ELSE \"c\"").unwrap();
    assert_eq!(expression.location.line, 2);
    assert_eq!(expression.location.column, 1);
    let ExpressionKind::If { consequent, .. } = expression.node else {
        panic!("expected IF")
    };
    assert!(matches!(consequent.node, ExpressionKind::String { ref value } if value == "a\"b"));
    for source in [
        "(* missing",
        "\"missing",
        "[a |-> 1",
        "1 = 2 = 3",
        "1 / 2 * 3",
        "TRUE /\\ FALSE \\/ TRUE",
    ] {
        assert_eq!(
            parse_expression(source).unwrap_err().kind,
            ErrorKind::Syntax,
            "{source}"
        );
    }
}

#[test]
fn strings_are_never_operators_and_concatenation_is_not_an_octal_numeral() {
    for source in ["\"[]\"", "\"SUBSET\"", "\"/\\\\\""] {
        assert!(matches!(
            parse_expression(source).unwrap().node,
            ExpressionKind::String { .. }
        ));
    }
    assert!(
        matches!(parse_expression(r"<<1>> \o <<2>>").unwrap().node, ExpressionKind::Apply { ref operator, .. } if operator == "\\o")
    );
    assert!(matches!(
        parse_expression(r"\o17").unwrap().node,
        ExpressionKind::Number { value: 15 }
    ));
}

#[test]
fn parses_config_and_diagnoses_incomplete_input() {
    let config = parse_config(
        "CONSTANT N = 3 S = {a, b}\nINIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    )
    .unwrap();
    assert_eq!(config.init.as_deref(), Some("Init"));
    assert_eq!(config.constants.len(), 2);
    assert!(!config.check_deadlock);
    for source in [
        "INIT",
        "INIT Init\nINIT Other",
        "CONSTANT N =",
        "CHECK_DEADLOCK maybe",
    ] {
        assert_eq!(parse_config(source).unwrap_err().kind, ErrorKind::Syntax);
    }
    let error = parse_module("---- MODULE M ----\nRECURSIVE Op(\n====").unwrap_err();
    assert_eq!(error.kind, ErrorKind::Syntax);
    assert_eq!((error.location.line, error.location.column), (3, 1));
}

#[test]
fn parses_die_hard_junctions_and_keeps_the_bullets() {
    let module = parse_module(include_str!("fixtures/DieHard.tla")).unwrap();
    assert_eq!(module.name, "DieHard");
    let initial = module
        .definitions
        .iter()
        .find(|definition| definition.name == "Init")
        .unwrap();
    assert!(
        matches!(initial.body.node, ExpressionKind::And { ref operands, .. } if operands.len() == 2)
    );
}

#[test]
fn rejects_empty_applications_and_misindented_junctions() {
    for source in ["F()", "f[]", "[f EXCEPT ![] = 0]", "/\\ TRUE\nFALSE"] {
        assert!(parse_expression(source).is_err(), "{source}");
    }
}

#[test]
fn roundtrips_the_typed_ast_and_returns_protocol_errors_as_data() {
    let module = parse_module(include_str!("fixtures/Channel.tla")).unwrap();
    let decoded: rtla::ast::Module =
        serde_json::from_slice(&serde_json::to_vec(&module).unwrap()).unwrap();
    assert_eq!(module, decoded);
    for bytes in [
        b"invalid".as_slice(),
        br#"{"kind":"module","source":"bad","extra":1}"#,
    ] {
        assert!(matches!(
            serde_json::from_slice::<rtla::Response>(&rtla::dispatch(bytes)).unwrap(),
            rtla::Response::Error { .. }
        ));
    }
}

#[test]
fn reports_resource_limits_without_claiming_the_tla_syntax_is_invalid() {
    let source = format!("{}TRUE{}", "(".repeat(130), ")".repeat(130));
    assert_eq!(
        parse_expression(&source).unwrap_err().kind,
        ErrorKind::Unsupported
    );
    let config = format!("CONSTANT N={}0{}", "{".repeat(130), "}".repeat(130));
    assert_eq!(
        parse_config(&config).unwrap_err().kind,
        ErrorKind::Unsupported
    );
    assert!(matches!(
        parse_expression("unresolved").unwrap().node,
        ExpressionKind::Identifier { .. }
    ));
}

#[test]
fn deeply_nested_calls_and_containers_return_resource_errors() {
    for (opening, closing) in [
        ("F(", ")"),
        ("<<", ">>"),
        ("{", "}"),
        ("F(LAMBDA value:", ")"),
    ] {
        let source = format!("{}TRUE{}", opening.repeat(130), closing.repeat(130));
        assert_eq!(
            parse_expression(&source).unwrap_err().kind,
            ErrorKind::Unsupported
        );
    }
}

#[test]
fn bounds_ast_depth_for_flat_operator_and_postfix_chains() {
    for source in [
        std::iter::repeat_n("1", 130).collect::<Vec<_>>().join("+"),
        format!("x{}", "'".repeat(130)),
        format!("x{}", ".a".repeat(130)),
    ] {
        assert!(
            matches!(parse_expression(&source), Err(error) if error.kind == ErrorKind::Unsupported)
        );
    }
}
