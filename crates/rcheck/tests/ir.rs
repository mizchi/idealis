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
    for (label, json, needle) in cases {
        let error = rcheck::ir::parse(&json.to_string()).unwrap_err();
        assert!(error.contains(needle), "{label}: {error}");
    }
}
