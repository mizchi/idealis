use rtla::{parse_expression, parse_module};

#[test]
fn remaining_official_corpus_controls_have_structured_asts() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/corpus-completion.json")).unwrap();
    for case in cases {
        parse_module(case["source"].as_str().unwrap())
            .unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
    }
}

#[test]
fn restricted_subscripts_keep_instance_arguments_and_separate_fairness_actions() {
    let expression = serde_json::to_value(parse_expression("[A]_M(S)!vars").unwrap()).unwrap();
    assert_eq!(expression["subscript"]["kind"], "Selection");
    let expression = serde_json::to_value(parse_expression("WF_M(S)!vars(A)").unwrap()).unwrap();
    assert_eq!(expression["action"]["name"], "A");
    assert_eq!(expression["subscript"]["selectors"][0]["name"], "vars");
    assert_eq!(expression["subscript"]["expression"]["operator"], "M");
    let expression = serde_json::to_value(parse_expression("WF_M(S)!vars(T)(A)").unwrap()).unwrap();
    assert_eq!(expression["action"]["name"], "A");
    assert_eq!(
        expression["subscript"]["selectors"][0]["args"][0]["name"],
        "T"
    );
}

#[test]
fn large_numerals_remain_exact_text_instead_of_rounding() {
    for source in [
        "9007199254740993",
        "184467440737095516160",
        "\\H9876543210FEDCBA",
    ] {
        let expression = serde_json::to_value(parse_expression(source).unwrap()).unwrap();
        assert_eq!(expression["kind"], "Integer");
        assert_eq!(expression["text"], source);
    }
}
