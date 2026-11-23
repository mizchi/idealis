//! Replays Alloy 6.2.0's recorded answers against ralloy with roracle.
//! Regenerate a group with `just ralloy-oracle-record <group>`.
#[path = "alloy/oracle.rs"]
mod oracle;

use oracle::{AlloyModel, Outcomes};
use roracle::{CheckReport, OracleGroup};

/// Replays a group. `pending` lists (model, command) pairs recorded as known
/// gaps: such a model must stay `unknown` (budget exhausted), never a
/// mismatch or a silent pass, and its other commands must still agree with
/// Alloy one by one. Solving a gap forces this list to be updated.
fn replay(group: &str, pending: &[(&str, &str)]) {
    let saved: CheckReport<AlloyModel, Outcomes> = serde_json::from_str(
        &std::fs::read_to_string(oracle::root().join(format!("{group}.json")))
            .unwrap_or_else(|error| panic!("{group}: record it first: {error}")),
    )
    .unwrap();
    let mut oracle_group = OracleGroup::new(group);
    // Registered cases must equal the recorded inputs, so an edited model
    // without a new recording fails instead of replaying stale answers.
    for (id, model) in oracle::models(group) {
        oracle_group.case(id, model);
    }
    oracle_group
        .reference("alloy-6.2.0", |_| unreachable!("replay never runs Alloy"))
        .test_function("ralloy", oracle::ralloy);
    let report = oracle_group.replay(&saved).unwrap();
    for case in &report.cases {
        let skipped: Vec<&str> = pending
            .iter()
            .filter(|(model, _)| *model == case.case.id)
            .map(|(_, command)| *command)
            .collect();
        let expected = if skipped.is_empty() {
            roracle::Status::Matched
        } else {
            let roracle::Observation::Value { value: answers } = &case.reference else {
                panic!("{group}/{}: no recorded answers", case.case.id);
            };
            let mut others = answers.clone();
            others.retain(|label, _| !skipped.contains(&label.as_str()));
            assert_eq!(
                oracle::ralloy_except(&case.case.input, &skipped),
                roracle::Observation::value(others),
                "{group}/{}: commands besides the pending ones",
                case.case.id
            );
            roracle::Status::Unknown
        };
        for comparison in &case.comparisons {
            assert_eq!(
                comparison.status, expected,
                "{group}/{}: {:?}",
                case.case.id, comparison.actual
            );
        }
    }
    for (id, _) in pending {
        assert!(
            report.cases.iter().any(|case| case.case.id == *id),
            "{group}: unknown pending case {id}"
        );
    }
}

#[test]
fn core_agrees_with_alloy() {
    replay("core", &[]);
}

#[test]
fn nary_agrees_with_alloy() {
    // field-composition/CompositionHolds needs about 1.3e8 candidates at
    // scope 3 and exhausts the budget (plan 0079); plan 0083 targets it.
    replay("nary", &[("field-composition", "CompositionHolds")]);
}

#[test]
fn hierarchy_agrees_with_alloy() {
    replay("hierarchy", &[]);
}

#[test]
fn predicates_agree_with_alloy() {
    replay("predicates", &[]);
}
