//! Plan 0101: the registry answers the corpus's claims, falls back when the
//! reference cannot enumerate, and never reads a bound or a budget as a pass.
use rcheck::{Budget, Coverage, Error, Selection, Verdict};
use serde_json::Value;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn word(verdict: &Verdict) -> &'static str {
    match verdict {
        Verdict::Holds => "holds",
        Verdict::Violated => "violated",
        Verdict::Reachable => "reachable",
        Verdict::Unreachable => "unreachable",
        Verdict::Empty => "empty",
        Verdict::Unknown => "unknown",
        Verdict::Error => "error",
    }
}

#[test]
fn auto_meets_every_claim_and_all_agree() {
    let mut compared = 0;
    for spec in ["fixtures/rmodel/spec.json", "spec/idealis.json"] {
        let spec: Value =
            serde_json::from_str(&std::fs::read_to_string(root().join(spec)).unwrap()).unwrap();
        for rule in spec["spec"]["rules"].as_array().unwrap() {
            for claim in rule["claims"].as_array().unwrap() {
                let path = claim["model"].as_str().unwrap();
                let model =
                    rmodel::parse(&std::fs::read_to_string(root().join(path)).unwrap()).unwrap();
                let check = claim["check"].as_str().unwrap();
                let auto =
                    rcheck::check(&model, check, &Selection::Auto, Budget::default()).unwrap();
                assert_eq!(auto.answers.len(), 1);
                assert_eq!(auto.answers[0].checker, "reference", "{path} {check}");
                assert_eq!(
                    word(&auto.answers[0].verdict),
                    claim["expect"],
                    "{path} {check}"
                );
                let all = rcheck::check(&model, check, &Selection::All, Budget::default()).unwrap();
                assert_eq!(all.answers.len(), 3, "{path} {check}");
                assert!(!all.disagreement, "{path} {check}: {:?}", all.answers);
                for answer in &all.answers {
                    assert_ne!(answer.verdict, Verdict::Error, "{path} {check}: {answer:?}");
                }
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 32);
}

const WIDE: &str = "model Wide
sig A
var r: A -> set A
init { no r }
action link(a: A, b: A) { r' = r + a -> b }
invariant Loose { all a: A | lone a.r }
reachable Linked { some r }
scope A = 5, steps = 3";

/// Small budgets keep the wide model fast: the TLA+ lowering enumerates
/// `SUBSET D_r` for the initial states, 2^25 values here.
fn small() -> Budget {
    Budget {
        states: 10_000,
        work: 2_000_000,
        ..Budget::default()
    }
}

#[test]
fn auto_falls_back_to_explicit_and_says_unknown_when_it_runs_out() {
    let model = rmodel::parse(WIDE).unwrap();
    let reasons = match rcheck::check(
        &model,
        "Loose",
        &Selection::Named("reference".into()),
        small(),
    ) {
        Err(Error::Unsupported(_, reasons)) => reasons,
        other => panic!("{other:?}"),
    };
    assert!(reasons[0].1.contains("25 possible tuples"), "{reasons:?}");
    for name in ["Loose", "Linked", "Declarations"] {
        let report = rcheck::check(&model, name, &Selection::Auto, small()).unwrap();
        let answer = &report.answers[0];
        assert_eq!(answer.checker, "explicit", "{name}");
        assert_eq!(answer.verdict, Verdict::Unknown, "{name}: {answer:?}");
        assert!(!answer.definite(), "{name}");
    }
    // SAT still answers within its bound.
    let sat = rcheck::check(&model, "Loose", &Selection::Named("sat".into()), small()).unwrap();
    assert_eq!(sat.answers[0].verdict, Verdict::Violated);
}

#[test]
fn sat_answers_within_its_bound() {
    let model = rmodel::parse(
        "model Deep
sig A
var x: set A
init { no x }
action add(a: A) { x' = x + a }
invariant Small { #x < 3 }
scope A = 3, steps = 2",
    )
    .unwrap();
    let report = rcheck::check(&model, "Small", &Selection::All, Budget::default()).unwrap();
    let sat = report.answers.iter().find(|a| a.checker == "sat").unwrap();
    assert_eq!(sat.verdict, Verdict::Holds);
    assert_eq!(sat.coverage, Coverage::Bounded { steps: 2 });
    assert!(!sat.definite());
    let reference = report
        .answers
        .iter()
        .find(|a| a.checker == "reference")
        .unwrap();
    assert_eq!(reference.verdict, Verdict::Violated);
    assert_eq!(reference.depth, Some(3));
    // The violation lies beyond the bound: no contradiction.
    assert!(!report.disagreement);
}

#[test]
fn unknown_names_are_errors() {
    let model = rmodel::parse(WIDE).unwrap();
    assert_eq!(
        rcheck::check(&model, "Nope", &Selection::Auto, Budget::default()).unwrap_err(),
        Error::NoQuestion("Nope".into())
    );
    assert_eq!(
        rcheck::check(
            &model,
            "Loose",
            &Selection::Named("tlc".into()),
            Budget::default()
        )
        .unwrap_err(),
        Error::NoChecker("tlc".into())
    );
}

const MANY: &str = "model Many
sig S
enum E = Ea | Eb
rel r0: S -> some E
rel r1: S -> set S
var v: S -> set S
init { no v }
action a(s: S) { v' = v + s -> s }
invariant Inv { v in r1 + (S -> S) }
scope S = 4, steps = 3";

/// Seed 6130 of the 0100 confirmation run: each relation is enumerable, but
/// configurations times initial candidates are not. The budget must cover
/// them and answer unknown rather than spin.
#[test]
fn configurations_count_against_the_budget() {
    let model = rmodel::parse(MANY).unwrap();
    let started = std::time::Instant::now();
    let report = rcheck::check(
        &model,
        "Inv",
        &Selection::Named("reference".into()),
        Budget {
            states: 10_000,
            ..Budget::default()
        },
    )
    .unwrap();
    assert_eq!(
        report.answers[0].verdict,
        Verdict::Unknown,
        "{:?}",
        report.answers[0]
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(30));
}

#[test]
fn explicit_configurations_count_against_the_budget() {
    let model = rmodel::parse(MANY).unwrap();
    let started = std::time::Instant::now();
    let report = rcheck::check(
        &model,
        "Inv",
        &Selection::Named("explicit".into()),
        Budget {
            states: 10_000,
            work: 2_000_000,
            ..Budget::default()
        },
    )
    .unwrap();
    assert_eq!(
        report.answers[0].verdict,
        Verdict::Unknown,
        "{:?}",
        report.answers[0]
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(30),
        "{:?}",
        started.elapsed()
    );
}

const ESCAPING: &str = "model Bank
sig Account
var balance: Account -> one Int
init { all a: Account | a.balance = 0 }
action deposit(a: Account, n: Int) {
  n > 0
  balance' = balance - a -> a.balance + a -> (a.balance + n)
}
invariant NonNegative { all a: Account | a.balance >= 0 }
reachable Rich { some a: Account | a.balance >= 3 }
scope Account = 1, Int = 0..4, steps = 3";

/// Plan 0103: integers that leave the scope make every negative answer
/// unknown, and SAT declines integer values.
#[test]
fn values_leave_the_scope_and_sat_declines_them() {
    let model = rmodel::parse(ESCAPING).unwrap();
    let report = rcheck::check(&model, "NonNegative", &Selection::All, Budget::default()).unwrap();
    let checkers: Vec<_> = report.answers.iter().map(|a| a.checker).collect();
    assert_eq!(checkers, ["reference", "explicit"]);
    for answer in &report.answers {
        assert_eq!(answer.verdict, Verdict::Unknown, "{answer:?}");
    }
    let rich = rcheck::check(&model, "Rich", &Selection::All, Budget::default()).unwrap();
    for answer in &rich.answers {
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&Verdict::Reachable, Some(1)),
            "{answer:?}"
        );
    }
    match rcheck::check(
        &model,
        "Rich",
        &Selection::Named("sat".into()),
        Budget::default(),
    ) {
        Err(Error::Unsupported(_, reasons)) => {
            assert!(reasons[0].1.contains("integer"), "{reasons:?}")
        }
        other => panic!("{other:?}"),
    }
}
