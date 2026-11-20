use rtla::parse_module;

#[test]
fn terminal_proofs_and_assume_prove_preserve_structure() {
    let module = serde_json::to_value(parse_module("---- MODULE Test ----\nLEMMA Identity == ASSUME NEW CONSTANT x, NEW F(_), x = x PROVE x = x\nPROOF BY ONLY x = x DEF F\nTHEOREM TRUE OBVIOUS\nTHEOREM FALSE PROOF OMITTED\n====").unwrap()).unwrap();
    let theorem = &module["theorems"][0];
    assert_eq!(theorem["statement"]["kind"], "AssumeProve");
    assert_eq!(
        theorem["statement"]["assumptions"][0]["declaration"]["name"],
        "x"
    );
    assert_eq!(
        theorem["statement"]["assumptions"][1]["declaration"]["arity"],
        1
    );
    assert_eq!(theorem["proof"]["kind"], "By");
    assert_eq!(theorem["proof"]["only"], true);
    assert_eq!(module["theorems"][1]["proof"]["kind"], "Obvious");
    assert_eq!(module["theorems"][2]["proof"]["kind"], "Omitted");
}

#[test]
fn hierarchical_proofs_keep_step_kinds_numbers_and_nested_proofs() {
    let module = serde_json::to_value(parse_module("---- MODULE Test ----\nTHEOREM TRUE\nPROOF\n<1>a DEFINE D == TRUE\n<1>b HAVE TRUE\n<1>c TAKE x, y\n<1>d WITNESS 1, 2\n<1>e PICK z : TRUE\n  <2>a TRUE OBVIOUS\n  <2>b QED BY <2>a\n<1>f USE ONLY D DEF D\n<1>g HIDE D\n<1>h CASE TRUE\n  PROOF OBVIOUS\n<1>i SUFFICES ASSUME TRUE PROVE TRUE\n  OBVIOUS\n<1>q QED BY <1>b, <1>e!1\n====").unwrap()).unwrap();
    let steps = &module["theorems"][0]["proof"]["steps"];
    for (index, kind) in [
        "Define", "Have", "Take", "Witness", "Pick", "Use", "Hide", "Case", "Assert", "Qed",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(steps[index]["body"]["kind"], *kind);
    }
    assert_eq!(steps[4]["proof"]["steps"][0]["number"], "<2>a");
    assert_eq!(steps[4]["proof"]["steps"][1]["body"]["kind"], "Qed");
    assert_eq!(steps[8]["body"]["suffices"], true);
}

#[test]
fn proof_levels_and_statements_cannot_be_silently_skipped() {
    for body in [
        "THEOREM TRUE PROOF <1>a TRUE",
        "THEOREM TRUE PROOF <1>a HAVE TRUE OBVIOUS <1>q QED",
        "THEOREM TRUE PROOF <2>a TRUE <1>q QED",
        "THEOREM ASSUME NEW F(_) \\in {1} PROVE TRUE",
        "THEOREM label::ASSUME TRUE PROVE TRUE",
    ] {
        assert!(
            parse_module(&format!("---- MODULE Test ----\n{body}\n====")).is_err(),
            "{body}"
        );
    }
}
