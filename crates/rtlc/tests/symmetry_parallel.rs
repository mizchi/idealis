#![cfg(not(target_arch = "wasm32"))]

use rtlc::{SafetyBounds, SafetyOptions, SafetyProgram};

#[test]
fn parallel_record_orbits_preserve_counts_and_never_exceed_work_bounds() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("../src/reduction-fixtures.json")).unwrap();
    let fixture = fixtures
        .as_array()
        .unwrap()
        .iter()
        .find(|fixture| fixture["name"] == "record-12-safe/symmetry-1")
        .unwrap();
    let program: SafetyProgram = serde_json::from_value(fixture["program"].clone()).unwrap();
    let bounds = SafetyBounds {
        max_depth: 100,
        max_states: 10000,
        max_work: 100000000,
        max_temporal_nodes: None,
    };
    for symmetry_actions in [false, true] {
        let options = SafetyOptions {
            symmetry_actions,
            ..Default::default()
        };
        let serial = program.check_with_options(bounds, options).unwrap();
        let parallel = program
            .check_with_options(
                bounds,
                SafetyOptions {
                    workers: 8,
                    ..options
                },
            )
            .unwrap();
        assert_eq!(
            (
                parallel.status,
                parallel.states,
                parallel.generated,
                parallel.depth,
                parallel.violation
            ),
            (
                serial.status,
                serial.states,
                serial.generated,
                serial.depth,
                serial.violation
            )
        );
        for maximum in [1, 17, 5_000_000] {
            let result = program
                .check_with_options(
                    SafetyBounds {
                        max_work: maximum,
                        ..bounds
                    },
                    SafetyOptions {
                        workers: 8,
                        ..options
                    },
                )
                .unwrap();
            assert_eq!(result.status, "bounded");
            assert!(result.work <= maximum);
        }
    }
}
