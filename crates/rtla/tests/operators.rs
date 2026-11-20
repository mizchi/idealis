use rtla::{parse_expression, parse_module};
use serde::Deserialize;

#[derive(Deserialize)]
struct Operator {
    canonical: String,
    fix: String,
}
#[derive(Deserialize)]
struct Alias {
    spelling: String,
    canonical: String,
}
#[derive(Deserialize)]
struct Catalog {
    operators: Vec<Operator>,
    aliases: Vec<Alias>,
}

#[test]
fn parses_the_complete_operator_vocabulary_and_aliases() {
    let catalog: Catalog = serde_json::from_str(include_str!("fixtures/operators.json")).unwrap();
    assert_eq!(catalog.operators.len(), 101);
    for alias in catalog.aliases {
        let Some(operator) = catalog
            .operators
            .iter()
            .find(|operator| operator.canonical == alias.canonical)
        else {
            continue;
        };
        if matches!(operator.canonical.as_str(), "." | "[") {
            continue;
        }
        let application = match operator.fix.as_str() {
            "prefix" if operator.canonical == "-." => "- value".into(),
            "prefix" => format!("{} value", alias.spelling),
            "postfix" => format!("value{}", alias.spelling),
            _ => format!("left {} right", alias.spelling),
        };
        assert!(parse_expression(&application).is_ok(), "{application}");
        let head = match operator.fix.as_str() {
            "prefix" => format!("{} value", alias.spelling),
            "postfix" => format!("value{}", alias.spelling),
            _ => format!("left {} right", alias.spelling),
        };
        let source = format!("---- MODULE Operators ----\n{head} == 0\n====");
        let module = parse_module(&source).unwrap_or_else(|error| panic!("{head}: {error:?}"));
        assert_eq!(module.definitions[0].name, alias.canonical, "{head}");
    }
}

#[test]
fn unicode_structural_aliases_parse_at_the_same_boundary() {
    for source in [
        "CASE TRUE → 1 □ OTHER → 2",
        "[value ∈ {1} ↦ value]",
        "[ {1} → {2} ]",
        "∀ value ∈ {1}: value = 1",
        "label ∷ TRUE",
        "ℕ",
        "ℤ",
        "ℝ",
        "⟨TRUE⟩_x",
        "/\\ TRUE\n/\\ FALSE",
    ] {
        assert!(parse_expression(source).is_ok(), "{source}");
    }
    assert!(parse_module("---- MODULE Unicode ----\nDef ≜ TRUE\n====").is_ok());
}
