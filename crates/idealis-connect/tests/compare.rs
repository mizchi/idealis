use idealis_connect::{ComparisonStatus, Observation, Target, compare};
use serde_json::json;

fn value(id: &str, value: serde_json::Value) -> Target {
    Target {
        id: id.into(),
        outcome: Observation::Value { value },
    }
}

#[test]
fn compares_each_implementation_with_an_independent_allowed_set() {
    let oracle = Observation::Allowed {
        values: vec![
            json!({"running":true,"amount":2}),
            json!({"amount":3,"running":true}),
        ],
    };
    let checked = compare(
        oracle,
        vec![
            value("rust", json!({"amount":2,"running":true})),
            value("go", json!({"amount":3,"running":true})),
            value("broken", json!({"amount":0,"running":true})),
        ],
    )
    .unwrap();
    assert_eq!(checked.status, ComparisonStatus::Mismatch);
    assert_eq!(
        checked
            .comparisons
            .iter()
            .map(|entry| entry.status)
            .collect::<Vec<_>>(),
        vec![
            ComparisonStatus::Matched,
            ComparisonStatus::Matched,
            ComparisonStatus::Mismatch
        ]
    );
}

#[test]
fn preserves_inconclusive_and_infrastructure_outcomes() {
    let checked = compare(
        Observation::Unknown {
            reason: "oracle-work".into(),
        },
        vec![Target {
            id: "driver".into(),
            outcome: Observation::Error {
                message: "setup".into(),
            },
        }],
    )
    .unwrap();
    assert_eq!(checked.status, ComparisonStatus::Error);
    assert!(
        matches!(&checked.comparisons[0].outcome, Observation::Error { message } if message == "setup")
    );
    let checked = compare(
        Observation::Allowed { values: vec![] },
        vec![value("driver", json!(null))],
    )
    .unwrap();
    assert_eq!(checked.status, ComparisonStatus::Invalid);
    for outcome in [
        Observation::Unknown {
            reason: "work".into(),
        },
        Observation::Invalid {
            reason: "precondition".into(),
        },
    ] {
        assert_ne!(
            compare(
                Observation::Allowed {
                    values: vec![json!(1)]
                },
                vec![Target {
                    id: "driver".into(),
                    outcome
                }]
            )
            .unwrap()
            .status,
            ComparisonStatus::Matched
        );
    }
}

#[test]
fn malformed_or_empty_comparisons_are_errors() {
    assert!(
        compare(
            Observation::Allowed {
                values: vec![json!(1)]
            },
            vec![]
        )
        .is_err()
    );
    assert!(
        compare(
            Observation::Allowed {
                values: vec![json!(1)]
            },
            vec![value("same", json!(1)), value("same", json!(2))]
        )
        .is_err()
    );
    assert!(
        compare(
            Observation::Allowed {
                values: vec![json!(1.5)]
            },
            vec![value("driver", json!(1))]
        )
        .is_err()
    );
    assert!(
        compare(
            Observation::Allowed {
                values: vec![json!(9007199254740992u64)]
            },
            vec![value("driver", json!(1))]
        )
        .is_err()
    );
}
