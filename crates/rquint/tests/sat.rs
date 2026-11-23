//! The 0044 Quint models lowered to Alloy and solved by ralloy's SAT, bounded
//! by 8 steps and 5-bit integers, answer like TLC (plan 0095). Models whose
//! values do not fit the bit width are refused, never wrapped.
use serde_json::Value;
use std::path::Path;

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn options(property: &str) -> rquint::Options {
    rquint::Options {
        invariants: vec![property.to_owned()],
        ..rquint::Options::default()
    }
}

#[test]
fn sat_agrees_with_the_tlc_oracle() {
    let oracle: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("fixtures/quint-generated/tlc-oracle.json")).unwrap(),
    )
    .unwrap();
    let (mut compared, mut rejected) = (0, 0);
    for entry in oracle.as_array().unwrap() {
        let name = entry["name"].as_str().unwrap();
        let property = entry["property"].as_str().unwrap();
        let ir =
            std::fs::read_to_string(root().join(format!("fixtures/quint-ir/{name}.json"))).unwrap();
        let lowered = rquint::to_alloy(&ir, &options(property), rquint::Bounds::default());
        // SequenceFold's total, 312, needs 10 bits; ralloy has at most 5.
        // The lowering refuses it instead of letting the integer wrap.
        if name == "SequenceFold" {
            let error = lowered.unwrap_err();
            assert!(error.0.contains("outside the 5-bit range"), "{error}");
            rejected += 1;
            continue;
        }
        let alloy_source = lowered.unwrap_or_else(|error| panic!("{name}: {error}"));
        let model = ralloy::Model::parse(&alloy_source)
            .unwrap_or_else(|error| panic!("{name}: {error}\n{alloy_source}"));
        let index = model
            .commands()
            .iter()
            .position(|command| command.label == format!("c_{property}"))
            .unwrap();
        let analysis = model.analyze(index, 50_000_000).unwrap();
        if let Some(instance) = &analysis.instance {
            let check = model.check_instance(index, instance, 50_000_000).unwrap();
            assert_eq!(
                check.status,
                ralloy::CheckStatus::Mismatch,
                "{name} {property}"
            );
        }
        let status = match analysis.status {
            ralloy::Status::NoCounterexample => "complete",
            ralloy::Status::Counterexample => "invariant-violation",
            other => panic!("{name} {property}: {other:?}\n{alloy_source}"),
        };
        assert_eq!(status, entry["status"], "{name} {property}\n{alloy_source}");
        compared += 1;
    }
    assert_eq!((compared, rejected), (10, 2));
}

/// Records, maps, lists and booleans as state are outside the SAT subset.
#[test]
fn coverage_is_rejected() {
    let ir = std::fs::read_to_string(root().join("fixtures/quint-ir/Coverage.json")).unwrap();
    let error =
        rquint::to_alloy(&ir, &options("all_facts"), rquint::Bounds::default()).unwrap_err();
    assert!(error.0.contains("unsupported"), "{error}");
}

/// Alloy 6.2.0's verdicts in alloy-sat.json were recorded for exactly the
/// text rquint lowers to now; rerun evals/experiments/quint-ir/alloy.sh
/// after changing the lowering.
#[test]
fn alloy_record_matches_the_current_lowering() {
    use sha2::{Digest, Sha256};
    let record: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("fixtures/quint-ir/alloy-sat.json")).unwrap(),
    )
    .unwrap();
    let oracle: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("fixtures/quint-generated/tlc-oracle.json")).unwrap(),
    )
    .unwrap();
    let rows = record["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 10);
    for row in rows {
        let name = row["name"].as_str().unwrap();
        let property = row["property"].as_str().unwrap();
        let ir =
            std::fs::read_to_string(root().join(format!("fixtures/quint-ir/{name}.json"))).unwrap();
        let source = rquint::to_alloy(&ir, &options(property), rquint::Bounds::default()).unwrap();
        let digest: String = Sha256::digest(source.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(digest, row["alsSha256"], "{name} {property}");
        let tlc = oracle
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"] == name && entry["property"] == property)
            .unwrap();
        let want = if tlc["status"] == "complete" {
            "no-counterexample"
        } else {
            "counterexample"
        };
        assert_eq!(row["alloy"], want, "{name} {property}");
    }
}
