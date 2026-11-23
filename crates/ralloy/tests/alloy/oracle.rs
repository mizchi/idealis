//! Shared by the Alloy recorder example and the replay test: the corpus, the
//! observation compared against Alloy 6.2.0 and how ralloy produces it.
#![allow(dead_code)]
use ralloy::{Model, Status};
use roracle::Observation;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Work budget per command; reaching it is `unknown`, never agreement.
pub const BUDGET: u64 = 50_000_000;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AlloyModel {
    pub source: String,
}

/// Whether each labeled command has a solution: an instance for `run`, a
/// counterexample for `check`.
pub type Outcomes = BTreeMap<String, bool>;

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/alloy")
}

/// The `.als` models of one corpus group, sorted by file name.
pub fn models(group: &str) -> Vec<(String, AlloyModel)> {
    let mut paths: Vec<_> = std::fs::read_dir(root().join(group))
        .unwrap_or_else(|error| panic!("{group}: {error}"))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "als"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let id = path.file_stem().unwrap().to_string_lossy().into_owned();
            let source = std::fs::read_to_string(&path).unwrap();
            (id, AlloyModel { source })
        })
        .collect()
}

/// Analyzes every command. A found instance must also pass ralloy's own
/// instance check, so an answer the evaluator cannot reproduce is invalid.
pub fn ralloy(model: &AlloyModel) -> Observation<Outcomes> {
    let parsed = match Model::parse(&model.source) {
        Ok(parsed) => parsed,
        Err(error) => {
            return Observation::Error {
                message: error.to_string(),
            };
        }
    };
    let mut outcomes = Outcomes::new();
    for (index, command) in parsed.commands().iter().enumerate() {
        let analysis = match parsed.analyze(index, BUDGET) {
            Ok(analysis) => analysis,
            Err(error) => {
                return Observation::Error {
                    message: format!("{}: {error}", command.label),
                };
            }
        };
        let solved = match analysis.status {
            Status::Instance | Status::Counterexample => true,
            Status::NoInstance | Status::NoCounterexample => false,
            Status::Unknown => {
                return Observation::Unknown {
                    reason: format!("{}: work budget", command.label),
                };
            }
        };
        if let Some(instance) = &analysis.instance {
            // A run instance satisfies its body; a check counterexample
            // satisfies the facts and violates the assertion.
            let expected = match command.kind {
                ralloy::Kind::Run => (ralloy::CheckStatus::Matched, None),
                ralloy::Kind::Check => (
                    ralloy::CheckStatus::Mismatch,
                    Some(ralloy::CheckReason::Property),
                ),
            };
            let checked = parsed.check_instance(index, instance, BUDGET);
            if !matches!(checked, Ok(ref check) if (check.status, check.reason) == expected) {
                return Observation::Invalid {
                    reason: format!("{}: instance fails its own check", command.label),
                };
            }
        }
        outcomes.insert(command.label.clone(), solved);
    }
    Observation::value(outcomes)
}
