#[test]
fn checked_model_can_be_loaded_without_a_checker() {
    let source = "model Library
entity Copy \"冊子\"
var held: lone Copy
init { no held }
behavior borrow(c: Copy) {
  guard { no held }
  update { held = c }
}
scope Copy = 1";
    let located = idealis_syntax::parse_located_checked(source).unwrap();
    let offset = located.declarations["borrow"];
    assert_eq!(
        source.chars().skip(offset).take(15).collect::<String>(),
        "behavior borrow"
    );
    let checked = idealis_syntax::validate_checked(located.model.into_model()).unwrap();
    assert_eq!(checked.actions[0].params[0].1, "Copy");
}

#[test]
fn source_descriptions_support_both_languages_and_block_strings() {
    let source = r#"model Notes
description = {
  ja: "モデルの目的",
  en: """First line
Second line with @text and "quotes".""",
}
"#;
    let located = idealis_syntax::parse_located_checked(source).unwrap();
    let (offset, descriptions) = located.descriptions.first_key_value().unwrap();
    assert_eq!(*offset, "model Notes\n".len());
    assert_eq!(descriptions["ja"], "モデルの目的");
    assert_eq!(
        descriptions["en"],
        "First line\nSecond line with @text and \"quotes\"."
    );
    let invalid = source.replace("ja:", "en:");
    assert!(idealis_syntax::parse_diagnostic(&invalid).is_err());
}
