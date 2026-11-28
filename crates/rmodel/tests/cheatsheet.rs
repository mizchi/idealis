use std::collections::BTreeMap;

use rmodel::reference::Outcome::{self, Holds, Reachable, Violated};

const GUIDE: &str = include_str!("../CHEATSHEET.md");
const EXPECTED: &[(&str, &[(&str, Outcome)])] = &[
    (
        "Locking",
        &[("OnlyAuthorized", Holds), ("Held", Reachable { depth: 1 })],
    ),
    (
        "Shipment",
        &[
            ("HasCarrier", Holds),
            ("CanShip", Reachable { depth: 1 }),
            ("KeepsTracking", Violated { depth: 2 }),
        ],
    ),
    (
        "SmallJournal",
        &[("Bounded", Holds), ("FirstEntry", Reachable { depth: 1 })],
    ),
    ("Service", &[]),
];

#[test]
fn cheatsheet_models_keep_their_documented_verdicts() {
    let mut expected_models: BTreeMap<_, _> = EXPECTED.iter().copied().collect();
    let blocks = GUIDE
        .split("\n```rmodel\n")
        .skip(1)
        .map(|block| block.split_once("\n```").expect("closing rmodel fence").0);

    for source in blocks {
        let model = rmodel::parse(source).unwrap_or_else(|error| panic!("{error}\n{source}"));
        let name = model.name.as_str();
        let expected = expected_models
            .remove(name)
            .unwrap_or_else(|| panic!("unexpected or duplicate cheatsheet model {name}"));
        let report = rmodel::reference::analyze(&model, 10_000);
        assert_eq!(report.scope, Holds, "{name}: exploration");
        assert_eq!(report.declarations, Holds, "{name}: declarations");
        let verdicts: Vec<_> = report
            .verdicts
            .iter()
            .map(|verdict| (verdict.check.as_str(), verdict.outcome))
            .collect();
        assert_eq!(verdicts, expected, "{name}");
        if name == "Service" {
            check_fairness(&model);
        }
    }
    assert!(
        expected_models.is_empty(),
        "missing cheatsheet models: {expected_models:?}"
    );
}

fn check_fairness(model: &rmodel::Model) {
    for (name, violates) in [
        ("FirstService", false),
        ("ServesWaiting", false),
        ("MayStarve", true),
    ] {
        let (source, config) = rmodel::to_tla(model, name).unwrap();
        let result = idealis_core::compile_tla_with_options(
            &source,
            &config,
            idealis_core::CompileOptions {
                irVersion: idealis_core::IrVersion::V2,
            },
        )
        .unwrap()
        .check(idealis_core::SafetyBounds {
            max_depth: 100,
            max_states: 1_000,
            max_work: 100_000,
            max_temporal_nodes: Some(1_000),
        })
        .unwrap();
        if violates {
            assert!(result.violation.is_some(), "{name}: {:?}", result.reason);
        } else {
            assert_eq!(result.status, "complete", "{name}: {:?}", result.reason);
            assert!(result.violation.is_none(), "{name}");
        }
    }
}
