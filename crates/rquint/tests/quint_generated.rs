//! The six Quint models of evaluation 0044, read from Quint's JSON IR and
//! checked by rtlc, must answer like TLC did on Apalache's TLA+ (plan 0089).
use serde_json::Value;
use std::path::Path;

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

#[test]
fn quint_ir_models_agree_with_the_tlc_oracle() {
    let oracle: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("fixtures/quint-generated/tlc-oracle.json")).unwrap(),
    )
    .unwrap();
    let mut compared = 0;
    for entry in oracle.as_array().unwrap() {
        let name = entry["name"].as_str().unwrap();
        let property = entry["property"].as_str().unwrap();
        let ir =
            std::fs::read_to_string(root().join(format!("fixtures/quint-ir/{name}.json"))).unwrap();
        let translation = rquint::translate(
            &ir,
            &rquint::Options {
                invariants: vec![property.to_owned()],
                ..rquint::Options::default()
            },
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));
        let program = idealis_core::compile_tla_with_options(
            &translation.source,
            &translation.config,
            idealis_core::CompileOptions {
                irVersion: idealis_core::IrVersion::V2,
            },
        )
        .unwrap_or_else(|error| panic!("{name}: {error:?}\n{}", translation.source));
        let result = program
            .check(idealis_core::SafetyBounds {
                max_depth: 1_000,
                max_states: 100_000,
                max_work: 10_000_000,
                max_temporal_nodes: None,
            })
            .unwrap();
        let status = match result.status.as_str() {
            "complete" => "complete",
            _ if result.violation.is_some() => "invariant-violation",
            other => panic!("{name} {property}: {other}"),
        };
        assert_eq!(status, entry["status"], "{name} {property}");
        // TLC reports no state count when the initial state already
        // violates the invariant.
        if !entry["states"].is_null() {
            assert_eq!(result.states as u64, entry["states"], "{name} {property}");
        }
        compared += 1;
    }
    assert_eq!(compared, 12);
}

fn check(ir: &str, invariant: &str) -> Result<(String, usize), String> {
    let translation = rquint::translate(
        ir,
        &rquint::Options {
            invariants: vec![invariant.to_owned()],
            ..rquint::Options::default()
        },
    )
    .unwrap_or_else(|error| panic!("{invariant}: {error}"));
    let program = idealis_core::compile_tla_with_options(
        &translation.source,
        &translation.config,
        idealis_core::CompileOptions {
            irVersion: idealis_core::IrVersion::V2,
        },
    )
    .unwrap_or_else(|error| panic!("{invariant}: {error:?}\n{}", translation.source));
    let result = program
        .check(idealis_core::SafetyBounds {
            max_depth: 1_000,
            max_states: 100_000,
            max_work: 10_000_000,
            max_temporal_nodes: None,
        })
        .map_err(|error| error.message)?;
    Ok((result.status, result.states))
}

/// Builtins covered by Coverage.qnt; Quint 0.33.1's simulator found no
/// violation of all_facts and a violation of broken_index.
#[test]
fn coverage_facts_agree_with_the_quint_simulator() {
    let ir = std::fs::read_to_string(root().join("fixtures/quint-ir/Coverage.json")).unwrap();
    for fact in [
        "lists",
        "sets",
        "records",
        "maps",
        "arithmetic",
        "variants",
        "nondets",
        "all_facts",
    ] {
        assert_eq!(check(&ir, fact), Ok(("complete".to_owned(), 4)), "{fact}");
    }
    // A false constant invariant is rejected before the search.
    assert_eq!(
        check(&ir, "broken_index"),
        Err("Constant invariant or property is false".to_owned())
    );
}

/// The TLC verdicts in tlc-rquint.json were recorded for exactly the TLA+
/// rquint emits now; rerun evals/experiments/quint-ir/tlc.sh after changing
/// the translation.
#[test]
fn tlc_record_matches_the_current_translation() {
    let record: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("fixtures/quint-ir/tlc-rquint.json")).unwrap(),
    )
    .unwrap();
    let oracle: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("fixtures/quint-generated/tlc-oracle.json")).unwrap(),
    )
    .unwrap();
    let entries = record["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 12);
    for entry in entries {
        let name = entry["name"].as_str().unwrap();
        let property = entry["property"].as_str().unwrap();
        let ir =
            std::fs::read_to_string(root().join(format!("fixtures/quint-ir/{name}.json"))).unwrap();
        let translation = rquint::translate(
            &ir,
            &rquint::Options {
                invariants: vec![property.to_owned()],
                ..rquint::Options::default()
            },
        )
        .unwrap();
        assert_eq!(
            sha256_hex(translation.source.as_bytes()),
            entry["tlaSha256"],
            "{name} {property}"
        );
        let expected = oracle
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"] == name && row["property"] == property)
            .unwrap();
        assert_eq!(entry["status"], expected["status"], "{name} {property}");
        assert_eq!(entry["states"], expected["states"], "{name} {property}");
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
