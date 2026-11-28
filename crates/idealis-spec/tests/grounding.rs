//! roracle's comparison rules grounded against their models (plans 0093,
//! 0094): every input in scope goes through the implementation, and each
//! result is checked as a model instance with ralloy's check_instance.
use roracle::Status;
use roracle::grounding::{aggregate, compare};
use std::collections::BTreeMap;

/// Every combination of zero to three comparison statuses, folded by
/// `Status::aggregate`, is an instance of spec/models/roracle-report.model
/// that satisfies its facts (plan 0093).
#[test]
fn aggregate_conforms_to_the_report_model() {
    let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let model = idealis_model::parse(
        &std::fs::read_to_string(root.join("spec/models/roracle-report.model")).unwrap(),
    )
    .unwrap();
    let alloy = ralloy::Model::parse(&idealis_model::to_alloy(&model)).unwrap();
    let all = [
        Status::Matched,
        Status::Mismatch,
        Status::Invalid,
        Status::Unknown,
        Status::Error,
    ];
    let name = |status: Status| format!("{status:?}");
    let slots = ["Slot0", "Slot1", "Slot2"];
    let mut checked = 0;
    for count in 0..=slots.len() {
        for code in 0..all.len().pow(count as u32) {
            let statuses: Vec<Status> = (0..count)
                .map(|position| all[code / all.len().pow(position as u32) % all.len()])
                .collect();
            let report = aggregate(statuses.iter().copied());
            let mut signatures = BTreeMap::from([
                ("World".to_owned(), vec!["World".to_owned()]),
                ("s_Slot".to_owned(), slots.map(String::from).to_vec()),
                ("s_Status".to_owned(), all.map(name).to_vec()),
            ]);
            for status in all {
                signatures.insert(format!("e_{}", name(status)), vec![name(status)]);
            }
            let instance = ralloy::Instance {
                signatures,
                fields: BTreeMap::from([
                    (
                        "r_status".to_owned(),
                        statuses
                            .iter()
                            .zip(slots)
                            .map(|(status, slot)| vec!["World".into(), slot.into(), name(*status)])
                            .collect(),
                    ),
                    (
                        "r_report".to_owned(),
                        vec![vec!["World".into(), name(report)]],
                    ),
                ]),
                trace: vec![BTreeMap::new()],
                back: Some(0),
            };
            let checked_instance = alloy.check_instance(0, &instance, 10_000_000).unwrap();
            assert!(
                !matches!(
                    checked_instance.status,
                    ralloy::CheckStatus::Invalid | ralloy::CheckStatus::Unknown
                ),
                "{statuses:?} -> {report:?}: {:?} {:?}",
                checked_instance.status,
                checked_instance.reason
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 156);
}

/// Every pair of nine observations over the values {0, 1}, compared by
/// `compare`, is an instance of spec/models/roracle-compare.model that
/// satisfies its facts (plan 0094).
#[test]
fn compare_conforms_to_the_compare_model() {
    let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let model = idealis_model::parse(
        &std::fs::read_to_string(root.join("spec/models/roracle-compare.model")).unwrap(),
    )
    .unwrap();
    let alloy = ralloy::Model::parse(&idealis_model::to_alloy(&model)).unwrap();
    let observations: Vec<roracle::Observation<u8>> = vec![
        roracle::Observation::value(0),
        roracle::Observation::value(1),
        roracle::Observation::Allowed { values: vec![] },
        roracle::Observation::Allowed { values: vec![0] },
        roracle::Observation::Allowed { values: vec![1] },
        roracle::Observation::Allowed { values: vec![0, 1] },
        roracle::Observation::Unknown {
            reason: "budget".into(),
        },
        roracle::Observation::Invalid {
            reason: "contract".into(),
        },
        roracle::Observation::Error {
            message: "crash".into(),
        },
    ];
    let value = |value: &u8| format!("V{value}");
    // (kind, value, allowed) of an observation as model atoms.
    let parts =
        |observation: &roracle::Observation<u8>| -> (&'static str, Vec<String>, Vec<String>) {
            match observation {
                roracle::Observation::Value { value: v } => ("ValueOf", vec![value(v)], vec![]),
                roracle::Observation::Allowed { values } => {
                    ("AllowedSet", vec![], values.iter().map(value).collect())
                }
                roracle::Observation::Unknown { .. } => ("UnknownObservation", vec![], vec![]),
                roracle::Observation::Invalid { .. } => ("InvalidObservation", vec![], vec![]),
                roracle::Observation::Error { .. } => ("ErrorObservation", vec![], vec![]),
            }
        };
    let outcomes = ["Matched", "Mismatch", "Invalid", "Unknown", "Error"];
    let kinds = [
        "ValueOf",
        "AllowedSet",
        "UnknownObservation",
        "InvalidObservation",
        "ErrorObservation",
    ];
    let mut checked = 0;
    for reference in &observations {
        for actual in &observations {
            let outcome = format!("{:?}", compare(reference, actual));
            let mut signatures = BTreeMap::from([
                ("World".to_owned(), vec!["World".to_owned()]),
                ("s_Value".to_owned(), vec!["V0".to_owned(), "V1".to_owned()]),
                ("s_Outcome".to_owned(), outcomes.map(String::from).to_vec()),
                ("s_Observed".to_owned(), kinds.map(String::from).to_vec()),
            ]);
            for member in outcomes.iter().chain(&kinds) {
                signatures.insert(format!("e_{member}"), vec![member.to_string()]);
            }
            let world = |atoms: Vec<String>| -> Vec<Vec<String>> {
                atoms
                    .into_iter()
                    .map(|atom| vec!["World".to_owned(), atom])
                    .collect()
            };
            let (reference_kind, reference_value, reference_allowed) = parts(reference);
            let (actual_kind, actual_value, actual_allowed) = parts(actual);
            let instance = ralloy::Instance {
                signatures,
                fields: BTreeMap::from([
                    (
                        "r_referenceKind".to_owned(),
                        world(vec![reference_kind.to_owned()]),
                    ),
                    ("r_referenceValue".to_owned(), world(reference_value)),
                    ("r_referenceAllowed".to_owned(), world(reference_allowed)),
                    (
                        "r_actualKind".to_owned(),
                        world(vec![actual_kind.to_owned()]),
                    ),
                    ("r_actualValue".to_owned(), world(actual_value)),
                    ("r_actualAllowed".to_owned(), world(actual_allowed)),
                    ("r_outcome".to_owned(), world(vec![outcome.clone()])),
                ]),
                trace: vec![BTreeMap::new()],
                back: Some(0),
            };
            let result = alloy.check_instance(0, &instance, 10_000_000).unwrap();
            assert!(
                !matches!(
                    result.status,
                    ralloy::CheckStatus::Invalid | ralloy::CheckStatus::Unknown
                ),
                "{reference:?} vs {actual:?} -> {outcome}: {:?} {:?}",
                result.status,
                result.reason
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 81);
}
