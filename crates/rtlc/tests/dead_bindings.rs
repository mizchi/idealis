use rtlc::{SafetyBounds, SafetyProgram, SafetyResult};
use serde_json::Value;

const FIXTURES: &str = include_str!("dead-bindings-fixtures.json");
const QUINT_FIXTURES: &str = include_str!("dead-bindings-quint-fixtures.json");

fn bounds() -> SafetyBounds {
    SafetyBounds {
        max_depth: 100,
        max_states: 10_000,
        max_work: 100_000_000,
        max_temporal_nodes: None,
    }
}

fn program(spec: &str, variant: &str, version: u64) -> SafetyProgram {
    let fixtures: Value = serde_json::from_str(FIXTURES).unwrap();
    let quint: Value = serde_json::from_str(QUINT_FIXTURES).unwrap();
    let case = fixtures["cases"]
        .as_array()
        .unwrap()
        .iter()
        .chain(quint["cases"].as_array().unwrap())
        .find(|case| {
            case["spec"] == spec && case["variant"] == variant && case["irVersion"] == version
        })
        .unwrap_or_else(|| panic!("missing fixture {spec} {variant} v{version}"));
    serde_json::from_value(case["program"].clone()).unwrap()
}

fn observable(result: &SafetyResult) -> (String, Option<String>, usize, u32, u32, Value) {
    (
        result.status.clone(),
        result.reason.clone(),
        result.states,
        result.generated,
        result.depth,
        serde_json::to_value(&result.violation).unwrap(),
    )
}

fn counts(result: &SafetyResult) -> (&str, usize, u32, u32) {
    (
        result.status.as_str(),
        result.states,
        result.generated,
        result.depth,
    )
}

#[test]
fn unread_bindings_keep_the_result_of_the_read_twin() {
    for version in [1, 2] {
        for variant in ["complete", "deadlock", "violation"] {
            let dead = program("dead", variant, version).check(bounds()).unwrap();
            let live = program("live", variant, version).check(bounds()).unwrap();
            assert_eq!(observable(&dead), observable(&live), "{variant} v{version}");
        }
        let complete = program("dead", "complete", version)
            .check(bounds())
            .unwrap();
        assert_eq!(counts(&complete), ("complete", 10, 232, 3), "v{version}");
    }
}

#[test]
fn unread_bindings_evaluate_each_distinct_continuation_once() {
    for version in [1, 2] {
        for variant in ["complete", "deadlock", "violation"] {
            let dead = program("dead", variant, version).check(bounds()).unwrap();
            let live = program("live", variant, version).check(bounds()).unwrap();
            assert!(
                dead.work * 3 < live.work * 2,
                "{variant} v{version}: unread {} read {}",
                dead.work,
                live.work
            );
        }
    }
}

#[test]
fn wide_parallel_frontiers_keep_the_tlc_counts_with_less_work() {
    let bounds = SafetyBounds {
        max_states: 1_000_000,
        max_work: u32::MAX,
        ..bounds()
    };
    for workers in [1, 2, 4, 8] {
        let unread = program("nondet-tags", "complete", 2)
            .check_parallel(bounds, workers)
            .unwrap();
        let read = program("nondet-tags-read", "complete", 2)
            .check_parallel(bounds, workers)
            .unwrap();
        assert_eq!(
            (unread.status.as_str(), unread.states, unread.generated),
            ("complete", 4785, 896_821),
            "workers {workers}"
        );
        assert_eq!(
            (read.status.as_str(), read.states, read.generated),
            ("complete", 4785, 896_821),
            "workers {workers}"
        );
        assert!(
            unread.work * 3 < read.work * 2,
            "workers {workers}: unread {} read {}",
            unread.work,
            read.work
        );
    }
}

#[test]
fn bindings_read_only_inside_enabled_or_an_operator_argument_stay_read() {
    for spec in ["enabled", "call"] {
        for version in [1, 2] {
            let result = program(spec, "complete", version).check(bounds()).unwrap();
            assert_eq!(counts(&result), ("complete", 5, 7, 2), "{spec} v{version}");
        }
    }
}

#[test]
fn enabled_branches_copied_into_operator_frames_are_evaluated_for_every_binding() {
    let result = program("frame-enabled", "violation", 2)
        .check(bounds())
        .unwrap();
    assert_eq!(counts(&result), ("invariant-violation", 1, 1, 0));
    assert_eq!(result.violation.unwrap().name, "Safe");
}

#[test]
fn replayed_continuations_never_need_more_work_than_the_first_evaluation() {
    for version in [1, 2] {
        let result = program("type-cache", "complete", version)
            .check(SafetyBounds {
                max_work: 17,
                ..bounds()
            })
            .unwrap();
        assert_eq!(counts(&result), ("complete", 1, 3, 0), "v{version}");
        let error = program("division", "error", version)
            .check(bounds())
            .unwrap_err();
        assert_eq!(
            (error.code, error.message.as_str()),
            ("protocol", "Integer division by zero"),
            "v{version}"
        );
    }
}

#[test]
fn continuations_with_many_successors_are_explored_without_replay() {
    let result = program("wide", "complete", 1)
        .check(SafetyBounds {
            max_work: 1_000_000,
            ..bounds()
        })
        .unwrap();
    assert_eq!(counts(&result), ("complete", 1, 100_001, 0));
}

#[test]
fn bindings_read_by_later_steps_stay_read() {
    let cases = [
        ("step-call", 1, ("complete", 12, 433, 1)),
        ("step-call", 2, ("complete", 12, 433, 1)),
        ("step-frame", 2, ("complete", 12, 433, 1)),
        ("choose-read", 1, ("complete", 5, 61, 1)),
        ("choose-read", 2, ("complete", 5, 61, 1)),
        ("bind-read", 1, ("complete", 4, 25, 1)),
        ("bind-read", 2, ("complete", 4, 25, 1)),
        ("step-call-args", 1, ("complete", 7, 43, 1)),
        ("step-call-args", 2, ("complete", 7, 43, 1)),
        ("step-frame-args", 2, ("complete", 7, 43, 1)),
    ];
    for (spec, version, expected) in cases {
        let result = program(spec, "complete", version).check(bounds()).unwrap();
        assert_eq!(counts(&result), expected, "{spec} v{version}");
    }
}

#[test]
fn continuations_beyond_the_recording_limit_are_evaluated_again() {
    for version in [1, 2] {
        let result = program("many", "complete", version)
            .check(bounds())
            .unwrap();
        assert_eq!(counts(&result), ("complete", 100, 20_001, 1), "v{version}");
    }
}

#[test]
fn the_work_budget_still_bounds_replayed_continuations() {
    for version in [1, 2] {
        let result = program("huge", "complete", version)
            .check(SafetyBounds {
                max_work: 100_000,
                ..bounds()
            })
            .unwrap();
        assert_eq!(
            (
                result.status.as_str(),
                result.reason.as_deref(),
                result.generated,
                result.work
            ),
            ("bounded", Some("work"), 49_999, 100_000),
            "v{version}"
        );
    }
}

#[test]
fn bind_runs_keep_the_evaluation_depth_check() {
    let deepest = program("deep-action-20", "complete", 2)
        .check(bounds())
        .unwrap();
    assert_eq!(counts(&deepest), ("complete", 2, 3, 1));
    let beyond = program("deep-action-21", "bounded", 2)
        .check(bounds())
        .unwrap();
    assert_eq!(
        (beyond.status.as_str(), beyond.reason.as_deref()),
        ("bounded", Some("evaluation"))
    );
}

#[test]
fn runs_that_end_the_branch_are_evaluated_as_before() {
    let fixtures: Value = serde_json::from_str(FIXTURES).unwrap();
    for case in fixtures["cases"].as_array().unwrap() {
        if case["spec"] != "end-run" {
            continue;
        }
        let program: SafetyProgram = serde_json::from_value(case["program"].clone()).unwrap();
        for expected in case["baseline"].as_array().unwrap() {
            let max_work = u32::try_from(expected["maxWork"].as_u64().unwrap()).unwrap();
            let actual = program
                .check(SafetyBounds {
                    max_work,
                    ..bounds()
                })
                .unwrap();
            assert_eq!(
                serde_json::json!({
                    "maxWork": max_work,
                    "status": actual.status,
                    "reason": actual.reason,
                    "states": actual.states,
                    "generated": actual.generated,
                    "work": actual.work,
                }),
                *expected,
                "v{}",
                case["irVersion"]
            );
        }
    }
}
