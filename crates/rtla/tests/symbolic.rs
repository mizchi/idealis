use rtla::ast::{DefinitionKind, ExpressionKind};
use rtla::{ErrorKind, parse_expression, parse_module};
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    id: String,
    source: String,
    accepted: bool,
}

#[test]
fn symbolic_syntax_matches_the_fixed_sany_controls() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("fixtures/symbolic.json")).unwrap();
    assert_eq!(cases.len(), 33);
    for case in cases {
        let result = parse_module(&case.source);
        if case.accepted {
            assert!(result.is_ok(), "{}: {result:?}", case.id);
        } else {
            assert_eq!(result.unwrap_err().kind, ErrorKind::Syntax, "{}", case.id);
        }
    }
}

#[test]
fn symbolic_heads_keep_parameter_order_arity_and_locations_without_new_ast_kinds() {
    let module = parse_module(
        "---- MODULE Symbolic ----\nleft %% right == left\n-. value == value\nvalue ^+ == value\nCONSTANT _ + _, ~ _, _ ^*\nF(_ %% _, -. _, _ ^+) == TRUE\n====",
    ).unwrap();
    let heads = module
        .definitions
        .iter()
        .map(|definition| {
            let DefinitionKind::Operator { params } = &definition.head else {
                panic!("operator head")
            };
            (
                definition.name.as_str(),
                params
                    .iter()
                    .map(|param| (param.name.as_str(), param.arity))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        heads,
        vec![
            ("%%", vec![("left", 0), ("right", 0)]),
            ("-.", vec![("value", 0)]),
            ("^+", vec![("value", 0)]),
            ("F", vec![("%%", 2), ("-.", 1), ("^+", 1)]),
        ]
    );
    assert_eq!(module.definitions[0].location.column, 6);
    assert_eq!(module.definitions[1].location.column, 1);
    assert_eq!(module.definitions[2].location.column, 7);
    assert_eq!(
        module
            .constants
            .iter()
            .map(|constant| (constant.name.as_str(), constant.arity))
            .collect::<Vec<_>>(),
        vec![("+", 2), ("~", 1), ("^*", 1)]
    );
    assert!(matches!(parse_expression("M!N!%%(1, 2)").unwrap().node,
        ExpressionKind::Apply { operator, args } if operator == "M!N!%%" && args.len() == 2));
    assert!(matches!(parse_expression("-(1)").unwrap().node,
        ExpressionKind::Apply { operator, args } if operator == "-." && args.len() == 1));
}

#[test]
fn symbolic_references_do_not_confuse_strings_and_prefix_operands() {
    let expression = parse_expression("F(+, -, -., ~, ^+, M!%%)").unwrap();
    let ExpressionKind::Apply { args, .. } = expression.node else {
        panic!("call")
    };
    assert_eq!(
        args.iter()
            .map(|argument| match &argument.node {
                ExpressionKind::Identifier { name } => name.as_str(),
                _ => panic!("operator reference"),
            })
            .collect::<Vec<_>>(),
        vec!["+", "-", "-.", "~", "^+", "M!%%"]
    );
    assert!(
        matches!(parse_expression("\"%%\"").unwrap().node, ExpressionKind::String { value } if value == "%%")
    );
    assert!(parse_expression("~").is_err());
    assert!(parse_expression("- value == value").is_err());
    assert!(parse_expression("F(\\X)").is_err());
    assert!(parse_expression("F( \\X )").is_err());
    assert!(parse_expression("F( \\times )").is_err());
    assert!(
        matches!(parse_expression("F(×)").unwrap().node, ExpressionKind::Apply { args, .. } if matches!(&args[0].node, ExpressionKind::Identifier { name } if name == "\\X"))
    );
}
