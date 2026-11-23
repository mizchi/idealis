//! Replays Alloy 6.2.0's recorded answers against ralloy with roracle.
//! Regenerate a group with `just ralloy-oracle-record <group>`.
#[path = "alloy/oracle.rs"]
mod oracle;

use oracle::{AlloyModel, Outcomes};
use roracle::{CheckReport, OracleGroup};

fn replay(group: &str) {
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
    oracle_group.replay(&saved).unwrap().assert_matched();
}

#[test]
fn core_agrees_with_alloy() {
    replay("core");
}
