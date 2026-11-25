//! The reference evaluator against every claim of the specifications
//! (plan 0097): it is the semantics, so it must give the verdicts the
//! claims expect.
use serde_json::Value;
use std::path::Path;

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn word(outcome: rmodel::reference::Outcome) -> &'static str {
    use rmodel::reference::Outcome;
    match outcome {
        Outcome::Empty => "empty",
        Outcome::Holds => "holds",
        Outcome::Violated { .. } => "violated",
        Outcome::Reachable { .. } => "reachable",
        Outcome::Unreachable => "unreachable",
        Outcome::Unknown => "unknown",
    }
}

#[test]
fn reference_verdicts_match_every_claim() {
    let mut compared = 0;
    for spec in ["fixtures/rmodel/spec.json", "spec/idealis.json"] {
        let spec: Value =
            serde_json::from_str(&std::fs::read_to_string(root().join(spec)).unwrap()).unwrap();
        for rule in spec["spec"]["rules"].as_array().unwrap() {
            for claim in rule["claims"].as_array().unwrap() {
                let path = claim["model"].as_str().unwrap();
                let model =
                    rmodel::parse(&std::fs::read_to_string(root().join(path)).unwrap()).unwrap();
                let verdicts = rmodel::reference::evaluate(&model, 2_000_000);
                let check = claim["check"].as_str().unwrap();
                let verdict = verdicts.iter().find(|v| v.check == check).unwrap();
                assert_eq!(word(verdict.outcome), claim["expect"], "{path} {check}");
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 32);
}
