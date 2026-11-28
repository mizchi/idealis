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
