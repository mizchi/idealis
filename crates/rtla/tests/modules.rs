use rtla::parse_module;

#[test]
fn named_instances_keep_parameters_in_modules_and_let_scopes() {
    let module = serde_json::to_value(parse_module("---- MODULE Root ----\nA(n, F(_)) == INSTANCE Counter WITH Limit <- n, Transform <- F\nInit == LET B(n) == INSTANCE Counter WITH Limit <- n IN B(2)!Init\n====").unwrap()).unwrap();
    assert_eq!(module["instances"][0]["params"][0]["name"], "n");
    assert_eq!(module["instances"][0]["params"][1]["arity"], 1);
    let local = &module["definitions"][0]["body"];
    assert_eq!(local["kind"], "LetModule");
    assert_eq!(local["instances"][0]["name"], "B");
    assert_eq!(local["body"]["kind"], "Selection");
}

#[test]
fn recursive_declarations_keep_arity_and_mark_the_corresponding_definitions() {
    let module = serde_json::to_value(parse_module("---- MODULE Root ----\nRECURSIVE Fact(_), Even(_), Odd(_)\nFact(n) == IF n = 0 THEN 1 ELSE n * Fact(n-1)\nEven(n) == IF n = 0 THEN TRUE ELSE Odd(n-1)\nOdd(n) == IF n = 0 THEN FALSE ELSE Even(n-1)\nLocal == LET RECURSIVE F(_) F(n) == IF n=0 THEN 0 ELSE F(n-1) IN F(2)\n====").unwrap()).unwrap();
    assert_eq!(module["recursive"].as_array().unwrap().len(), 3);
    assert_eq!(module["recursive"][0]["arity"], 1);
    assert_eq!(module["definitions"][0]["isRecursive"], true);
    assert_eq!(
        module["definitions"][3]["body"]["definitions"][0]["isRecursive"],
        true
    );
}

#[test]
fn nested_modules_preserve_the_outer_module_after_the_inner_terminator() {
    let module = serde_json::to_value(parse_module("---- MODULE Root ----\nCONSTANT N\n---- MODULE Inner ----\nValue == N\n====\nI == INSTANCE Inner\nResult == I!Value\n====\nignored epilogue").unwrap()).unwrap();
    assert_eq!(module["modules"][0]["name"], "Inner");
    assert_eq!(module["modules"][0]["definitions"][0]["name"], "Value");
    assert_eq!(module["definitions"][0]["name"], "Result");
}
