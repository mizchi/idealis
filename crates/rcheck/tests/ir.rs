//! Plan 0108: the kernel IR as JSON round-trips through rmodel's checker
//! without changing the model or its verdicts, and malformed IR is
//! rejected before type checking.
#[path = "../../rmodel/examples/support/generator.rs"]
#[allow(dead_code)]
mod generator;

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn verdicts(model: &rmodel::Model) -> String {
    let report = rmodel::reference::analyze(model, 100_000);
    format!(
        "{:?} {:?} {:?}",
        report.verdicts, report.declarations, report.scope
    )
}

fn round_trip(label: &str, model: &rmodel::Model) {
    let ir = rcheck::ir::to_ir(model).unwrap_or_else(|e| panic!("{label}: {e}"));
    let json = serde_json::to_string(&ir).unwrap();
    let (back, omitted) =
        rcheck::ir::parse(&json).unwrap_or_else(|e| panic!("{label}: {e}\n{json}"));
    assert!(omitted.is_empty());
    assert_eq!(
        rcheck::ir::to_ir(&back).unwrap(),
        ir,
        "{label}: the IR changed"
    );
    assert_eq!(
        verdicts(&back),
        verdicts(model),
        "{label}: the verdicts changed"
    );
}

#[test]
fn entity_definitions_survive_ir_and_cannot_name_an_unknown_sort() {
    let model = rmodel::parse(include_str!("../../../fixtures/rcheck/bank.model")).unwrap();
    let mut ir = rcheck::ir::to_ir(&model).unwrap();
    let (round_tripped, _) = rcheck::ir::parse(&serde_json::to_string(&ir).unwrap()).unwrap();
    assert_eq!(
        round_tripped
            .entity_definitions
            .get("Account")
            .map(String::as_str),
        Some("入出金によって残高が変わる口座。")
    );
    ir.entity_definitions
        .insert("Missing".into(), "unknown entity".into());
    assert!(
        rcheck::ir::from_ir(&ir)
            .unwrap_err()
            .contains("unknown entity Missing")
    );
}

#[test]
fn the_corpus_round_trips() {
    let mut count = 0;
    for dir in ["fixtures/rmodel", "spec/models", "fixtures/rcheck"] {
        for entry in std::fs::read_dir(root().join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "model") {
                let model = rmodel::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
                round_trip(&path.display().to_string(), &model);
                count += 1;
            }
        }
    }
    assert!(count >= 8, "{count} models");
}

#[test]
fn generated_models_round_trip() {
    let mut count = 0;
    for seed in 9800..=9899 {
        if let Ok(model) = rmodel::parse(&generator::generate(seed)) {
            round_trip(&format!("seed {seed}"), &model);
            count += 1;
        }
    }
    assert!(count > 90, "{count} models");
}

#[test]
fn malformed_ir_is_rejected() {
    let model = rmodel::parse(
        &std::fs::read_to_string(root().join("fixtures/rmodel/permissions.model")).unwrap(),
    )
    .unwrap();
    let good = serde_json::to_value(rcheck::ir::to_ir(&model).unwrap()).unwrap();
    let mut cases = Vec::new();
    let mut unknown_op = good.clone();
    unknown_op["facts"][0]["op"] = "xor".into();
    cases.push(("unknown op", unknown_op, "unknown variant"));
    let mut missing = good.clone();
    missing["relations"][0]
        .as_object_mut()
        .unwrap()
        .remove("shape");
    cases.push(("missing field", missing, "missing field"));
    let mut extra = good.clone();
    extra["actions"][0]["note"] = "x".into();
    cases.push(("unknown field", extra, "unknown field"));
    let mut version = good.clone();
    version["version"] = 2.into();
    cases.push(("version", version, "version"));
    let mut ill_typed = good.clone();
    ill_typed["relations"][0]["shape"]["to"] = "Nowhere".into();
    cases.push(("unknown sort", ill_typed, "unknown sig"));
    let mut steps = good.clone();
    steps["scope"]["steps"] = 33.into();
    cases.push(("trace scope", steps, "steps"));
    let mut sort_size = good.clone();
    sort_size["scope"]["sizes"]["User"] = 8.into();
    cases.push(("sort scope", sort_size, "outside"));
    let mut ints = good;
    ints["scope"]["ints"] = serde_json::json!([i64::MIN, i64::MAX]);
    cases.push(("integer scope", ints, "at most 20"));
    for (label, json, needle) in cases {
        let error = rcheck::ir::parse(&json.to_string()).unwrap_err();
        assert!(error.contains(needle), "{label}: {error}");
    }
}

#[test]
fn linked_models_round_trip() {
    for name in ["withdrawal", "raft"] {
        let path = root().join(format!("fixtures/rcheck/linked/{name}/project.rmodel.json"));
        let source = rcheck::project::load_source(path.to_str().unwrap()).unwrap();
        round_trip(name, &rmodel::parse(&source).unwrap());
    }
}

#[test]
fn cli_withholds_verdicts_when_an_adapter_omits_a_transition() {
    let source = "model Gate
        enum Flag = Off | On
        var x: one Flag
        init { x = Off }
        behavior open() { update { x = On } }
        invariant Closed { x = Off }
        scope steps = 3";
    let mut ir = rcheck::ir::to_ir(&rmodel::parse(source).unwrap()).unwrap();
    ir.actions.clear();
    ir.omitted.push(rcheck::ir::Omitted {
        name: "open".into(),
        reason: "unsupported transition".into(),
    });
    let path = std::env::temp_dir().join(format!("rcheck-partial-{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_vec(&ir).unwrap()).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rcheck"))
        .arg(&path)
        .args(["--question", "Closed"])
        .output();
    std::fs::remove_file(path).unwrap();

    let output = output.unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("open (unsupported transition)"));
}
