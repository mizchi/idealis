use roracle::{BenchConfig, Observation, OracleGroup, Status};
use std::cell::Cell;

fn config() -> BenchConfig {
    BenchConfig {
        baseline: "a".into(),
        scope: "function".into(),
        samples: 4,
        warmup: 1,
        iterations: 2,
        max_evaluations: 100,
    }
}

#[test]
fn checks_and_replays_recorded_inputs_without_calling_reference() {
    let calls = Cell::new(0);
    let mut group = OracleGroup::new("arithmetic");
    group
        .case("four", 4)
        .reference("oracle", |input| {
            calls.set(calls.get() + 1);
            Observation::value(input + 1)
        })
        .test_function("a", |input| Observation::value(1 + input));
    let report = group.check().unwrap();
    report.assert_matched();
    let saved = serde_json::to_string(&report).unwrap();
    let decoded = serde_json::from_str(&saved).unwrap();
    group.replay(&decoded).unwrap().assert_matched();
    assert_eq!(calls.get(), 1);
    let mut tampered = report;
    tampered.cases[0].comparisons[0].status = Status::Mismatch;
    assert!(group.replay(&tampered).is_err());
}

#[test]
fn compares_values_and_allowed_sets_without_vacuous_success() {
    for (oracle, actual, expected) in [
        (
            Observation::value(1),
            Observation::value(2),
            Status::Mismatch,
        ),
        (
            Observation::Allowed { values: vec![1, 2] },
            Observation::value(2),
            Status::Matched,
        ),
        (
            Observation::Allowed { values: vec![] },
            Observation::value(2),
            Status::Invalid,
        ),
        (
            Observation::Unknown {
                reason: "budget".into(),
            },
            Observation::value(2),
            Status::Unknown,
        ),
        (
            Observation::value(1),
            Observation::Error {
                message: "crash".into(),
            },
            Status::Error,
        ),
        (
            Observation::value(1),
            Observation::Allowed { values: vec![1] },
            Status::Invalid,
        ),
        (
            Observation::Invalid {
                reason: "input".into(),
            },
            Observation::value(1),
            Status::Invalid,
        ),
    ] {
        let mut group = OracleGroup::new("values");
        group
            .case("input", ())
            .reference("oracle", |_| oracle.clone())
            .test_function("a", |_| actual.clone());
        assert_eq!(group.check().unwrap().status, expected);
    }
}

#[test]
fn rejects_empty_duplicate_and_incomplete_suites() {
    let mut group = OracleGroup::<(), i32>::new("empty");
    assert!(group.check().is_err());
    group
        .case("input", ())
        .reference("oracle", |_| Observation::value(1));
    assert!(group.check().is_err());
    group.test_function("a", |_| Observation::value(1));
    group.check().unwrap().assert_matched();
    group.case("input", ());
    assert!(group.check().is_err());
    let mut duplicate = OracleGroup::new("duplicate");
    duplicate
        .case("input", ())
        .reference("oracle", |_| Observation::value(1))
        .reference("other", |_| Observation::value(1))
        .test_function("a", |_| Observation::value(1));
    assert!(duplicate.check().is_err());
}

#[test]
fn measures_targets_only_with_balanced_order_and_explicit_baseline() {
    let reference_calls = Cell::new(0);
    let target_calls = Cell::new(0);
    let mut group = OracleGroup::new("bench");
    group
        .case("input", 7)
        .reference("oracle", |input| {
            reference_calls.set(reference_calls.get() + 1);
            Observation::value(*input)
        })
        .test_function("a", |input| {
            target_calls.set(target_calls.get() + 1);
            Observation::value(*input)
        })
        .test_function("b", |input| Observation::value(*input));
    let mut ticks = 0;
    let report = group
        .bench_with_clock(config(), || {
            ticks += 10;
            ticks
        })
        .unwrap();
    report.assert_matched();
    assert_eq!(reference_calls.get(), 1);
    assert_eq!(target_calls.get(), 10);
    assert_eq!(report.evaluations, 21);
    assert_eq!(
        report.cases[0].order,
        vec![
            vec!["a", "b"],
            vec!["b", "a"],
            vec!["b", "a"],
            vec!["a", "b"]
        ]
    );
    let first = &report.cases[0].measurements[0];
    assert_eq!(first.samples_ns, vec![20; 4]);
    assert_eq!(first.statistics.as_ref().unwrap().median_ns, 10.0);
    assert_eq!(first.speedup, Some(1.0));
}

#[test]
fn drift_during_timing_invalidates_all_ratios() {
    let calls = Cell::new(0);
    let mut group = OracleGroup::new("drift");
    group
        .case("input", ())
        .reference("oracle", |_| Observation::value(1))
        .test_function("a", |_| Observation::value(1))
        .test_function("b", |_| {
            calls.set(calls.get() + 1);
            Observation::value(if calls.get() < 4 { 1 } else { 2 })
        });
    let mut ticks = 0;
    let report = group
        .bench_with_clock(config(), || {
            ticks += 10;
            ticks
        })
        .unwrap();
    assert_eq!(report.status, Status::Mismatch);
    assert_eq!(report.failure.as_ref().unwrap().phase, "sample");
    assert!(
        report
            .cases
            .iter()
            .flat_map(|case| &case.measurements)
            .all(|measurement| measurement.speedup.is_none())
    );
}

#[test]
fn rejects_bad_budget_before_calling_any_implementation() {
    let mut group = OracleGroup::new("budget");
    group
        .case("input", ())
        .reference("oracle", |_| panic!("must not run"))
        .test_function("a", |_| Observation::<i32>::value(1));
    let mut limited = config();
    limited.max_evaluations = 1;
    assert!(group.bench_with_clock(limited, || 0).is_err());
    let mut missing = config();
    missing.baseline = "missing".into();
    assert!(group.bench_with_clock(missing, || 0).is_err());
}

#[test]
fn rejects_zero_and_backwards_clocks_without_fast_results() {
    for backwards in [false, true] {
        let mut group = OracleGroup::new("clock");
        group
            .case("input", ())
            .reference("oracle", |_| Observation::value(1))
            .test_function("a", |_| Observation::value(1));
        let mut ticks = 1000;
        let report = group
            .bench_with_clock(config(), || {
                if backwards {
                    ticks -= 1;
                }
                ticks
            })
            .unwrap();
        assert_eq!(report.status, Status::Unknown);
        assert_eq!(report.failure.as_ref().unwrap().phase, "clock");
        assert!(report.cases[0].measurements[0].speedup.is_none());
    }
}

#[test]
fn mismatching_preflight_never_reads_clock() {
    let mut group = OracleGroup::new("preflight");
    group
        .case("input", ())
        .reference("oracle", |_| Observation::value(1))
        .test_function("a", |_| Observation::value(2));
    let report = group
        .bench_with_clock(config(), || panic!("must not time"))
        .unwrap();
    assert_eq!(report.status, Status::Mismatch);
    assert_eq!(report.evaluations, 2);
}

#[test]
fn replay_validates_cases_ids_aggregate_and_version_before_running_targets() {
    let calls = Cell::new(0);
    let mut group = OracleGroup::new("integrity");
    group
        .case("input", 1)
        .reference("oracle", |input| Observation::value(*input))
        .test_function("a", |input| {
            calls.set(calls.get() + 1);
            Observation::value(*input)
        });
    let report = group.check().unwrap();
    let mut broken = Vec::new();
    let mut saved = report.clone();
    saved.version = 2;
    broken.push(saved);
    let mut saved = report.clone();
    saved.status = Status::Mismatch;
    broken.push(saved);
    let mut saved = report.clone();
    saved.cases[0].case.input = 2;
    broken.push(saved);
    let mut saved = report.clone();
    saved.cases[0].comparisons.clear();
    broken.push(saved);
    let mut saved = report.clone();
    saved.cases[0].comparisons[0].id = "wrong".into();
    broken.push(saved);
    let mut saved = report.clone();
    saved.cases.push(saved.cases[0].clone());
    broken.push(saved);
    let mut saved = report.clone();
    saved.cases.clear();
    broken.push(saved);
    for saved in broken {
        assert!(group.replay(&saved).is_err());
    }
    assert_eq!(calls.get(), 1);
}

#[test]
fn replay_can_load_cases_and_preserves_a_failure_instead_of_recomputing_the_oracle() {
    let mut original = OracleGroup::new("replay");
    original
        .case("input", 7)
        .reference("oracle", |_| Observation::value(8))
        .test_function("a", |input| Observation::value(*input));
    let saved = original.check().unwrap();
    let mut replay = OracleGroup::new("replay");
    replay
        .reference("oracle", |_| panic!("must use recorded oracle"))
        .test_function("a", |input: &i32| Observation::value(*input));
    assert_eq!(replay.replay(&saved).unwrap(), saved);
}

#[test]
fn warmup_errors_remove_all_ratios_and_preserve_raw_observation() {
    let calls = Cell::new(0);
    let mut group = OracleGroup::new("warmup");
    group
        .case("input", ())
        .reference("oracle", |_| Observation::value(1))
        .test_function("a", |_| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                Observation::value(1)
            } else {
                Observation::Error {
                    message: "failed after preflight".into(),
                }
            }
        });
    let report = group
        .bench_with_clock(config(), || panic!("warmup must fail before timing"))
        .unwrap();
    assert_eq!(report.status, Status::Error);
    let failure = report.failure.unwrap();
    assert_eq!(failure.phase, "warmup");
    assert_eq!(
        failure.actual,
        Observation::Error {
            message: "failed after preflight".into()
        }
    );
    assert!(report.cases[0].measurements[0].samples_ns.is_empty());
}

#[test]
fn nonuniform_samples_use_per_invocation_statistics_and_baseline_over_target_ratio() {
    let mut group = OracleGroup::new("statistics");
    group
        .case("input", ())
        .reference("oracle", |_| Observation::value(1))
        .test_function("a", |_| Observation::value(1))
        .test_function("b", |_| Observation::value(1));
    // Paired interval lengths follow the recorded AB, BA order.
    let mut ticks = [0, 10, 11, 31, 32, 72, 73, 103].into_iter();
    let mut options = config();
    options.samples = 2;
    options.iterations = 1;
    let report = group
        .bench_with_clock(options, || ticks.next().unwrap())
        .unwrap();
    report.assert_matched();
    let measurements = &report.cases[0].measurements;
    assert_eq!(measurements[0].samples_ns, vec![10, 30]);
    let statistics = measurements[0].statistics.as_ref().unwrap();
    assert_eq!(
        (
            statistics.mean_ns,
            statistics.median_ns,
            statistics.min_ns,
            statistics.max_ns
        ),
        (20.0, 20.0, 10.0, 30.0)
    );
    assert_eq!(measurements[1].speedup, Some(2.0 / 3.0));
}

#[test]
fn clock_regression_or_nonadvancing_interval_is_unknown() {
    for readings in [
        vec![10, 20, 5, 25],
        vec![0, u64::MAX - 1, u64::MAX - 1, u64::MAX],
    ] {
        let mut group = OracleGroup::new("clock");
        group
            .case("input", ())
            .reference("oracle", |_| Observation::value(1))
            .test_function("a", |_| Observation::value(1));
        let mut ticks = readings.into_iter();
        let mut options = config();
        options.iterations = 3;
        let report = group
            .bench_with_clock(options, || ticks.next().unwrap_or(u64::MAX))
            .unwrap();
        assert_eq!(report.status, Status::Unknown);
        assert!(report.cases[0].measurements[0].speedup.is_none());
    }
}

#[test]
fn invalid_benchmark_options_and_evaluation_count_overflow_never_execute_callbacks() {
    let mut group = OracleGroup::new("config");
    group
        .case("input", ())
        .reference("oracle", |_| panic!("invalid config"))
        .test_function("a", |_| Observation::value(1))
        .test_function("b", |_| Observation::value(1));
    for (samples, iterations, warmup) in [(0, 1, 0), (1, 0, 0), (u32::MAX, u32::MAX, u32::MAX)] {
        let mut options = config();
        options.samples = samples;
        options.iterations = iterations;
        options.warmup = warmup;
        options.max_evaluations = u64::MAX;
        assert!(
            group
                .bench_with_clock(options, || panic!("invalid config"))
                .is_err()
        );
    }
}
