const SOURCE: &str = r#"
model Ledger
entity Account "残高を持つ口座。引用: \"口座\"。"
var open: set Account
init { no open }
action add(a: Account) { open' = open + a }
reachable Opened { some open }
scope Account = 2, steps = 3
"#;

#[test]
fn entity_definition_generates_both_types_and_has_signature_semantics() {
    let model = rmodel::parse(SOURCE).unwrap();
    let old = rmodel::parse(&SOURCE.replace(
        r#"entity Account "残高を持つ口座。引用: \"口座\"。""#,
        "sig Account",
    ))
    .unwrap();
    let definition = "残高を持つ口座。引用: \"口座\"。";
    assert!(rmodel::bindings::rust(&model, SOURCE).contains(&format!("/// {definition}\n")));
    assert!(rmodel::bindings::typescript(&model).contains(&format!("/** {definition} */\n")));
    assert_eq!(
        rmodel::reference::evaluate(&model, 100),
        rmodel::reference::evaluate(&old, 100)
    );
    assert_eq!(rmodel::to_alloy(&model), rmodel::to_alloy(&old));
    assert_eq!(
        rmodel::to_tla(&model, "Opened").unwrap(),
        rmodel::to_tla(&old, "Opened").unwrap()
    );
}

#[test]
fn entity_names_are_checked_at_declarations_and_references() {
    for change in [
        SOURCE.replace("var open:", "sig Account\nvar open:"),
        SOURCE.replace("set Account", "set Acccount"),
        SOURCE.replace(
            r#"entity Account "残高を持つ口座。引用: \"口座\"。""#,
            "entity Account \"\"",
        ),
    ] {
        assert!(rmodel::parse(&change).is_err(), "{change}");
    }
}

#[test]
fn definition_text_remains_text_in_generated_sources() {
    let source = SOURCE.replace("残高を持つ口座", "説明 */ 終わり") + "\n// \"####\n";
    let model = rmodel::parse(&source).unwrap();
    let rust = rmodel::bindings::rust(&model, &source);
    assert!(rust.contains(&format!("pub const MODEL: &str = {:?};", source.trim_end())));
    let typescript = rmodel::bindings::typescript(&model);
    assert!(typescript.contains("/** 説明 *&#47; 終わり"));
}
