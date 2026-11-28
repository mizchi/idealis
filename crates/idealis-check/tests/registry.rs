//! Plan 0101: the registry answers the corpus's claims, falls back when the
//! reference cannot enumerate, and never reads a bound or a budget as a pass.
use idealis_check::{Budget, Coverage, Error, Selection, Verdict};
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
                    idealis_model::parse(&std::fs::read_to_string(root().join(path)).unwrap())
                        .unwrap();
                let check = claim["check"].as_str().unwrap();
                let auto = idealis_check::check(&model, check, &Selection::Auto, Budget::default())
                    .unwrap();
                assert_eq!(auto.answers.len(), 1);
                assert_eq!(auto.answers[0].checker, "reference", "{path} {check}");
                assert_eq!(
                    word(&auto.answers[0].verdict),
                    claim["expect"],
                    "{path} {check}"
                );
                let all = idealis_check::check(&model, check, &Selection::All, Budget::default())
                    .unwrap();
                assert_eq!(all.answers.len(), 4, "{path} {check}");
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

/// The unconstrained initial relation has 2^25 values. Keep its exploration bounded.
fn small() -> Budget {
    Budget {
        states: 10_000,
        work: 2_000_000,
        ..Budget::default()
    }
}

#[test]
fn auto_falls_back_to_explicit_and_says_unknown_when_it_runs_out() {
    let model = idealis_model::parse(&WIDE.replace("init { no r }", "")).unwrap();
    let reasons = match idealis_check::check(
        &model,
        "Loose",
        &Selection::Named("reference".into()),
        small(),
    ) {
        Err(Error::Unsupported(_, reasons)) => reasons,
        other => panic!("{other:?}"),
    };
    assert!(reasons[0].1.contains("25 possible tuples"), "{reasons:?}");
    let exhausted = Budget {
        states: 1,
        ..small()
    };
    for name in ["Loose", "Linked", "Declarations"] {
        let report = idealis_check::check(&model, name, &Selection::Auto, exhausted).unwrap();
        let answer = &report.answers[0];
        assert_eq!(answer.checker, "explicit", "{name}");
        assert_eq!(answer.verdict, Verdict::Unknown, "{name}: {answer:?}");
        let wire = serde_json::to_value(answer).unwrap();
        assert_eq!(wire["diagnostics"][0]["kind"], "budget", "{name}: {wire}");
        assert!(!answer.definite(), "{name}");
    }
    // SAT still answers within its bound.
    let sat =
        idealis_check::check(&model, "Loose", &Selection::Named("sat".into()), small()).unwrap();
    assert_eq!(sat.answers[0].verdict, Verdict::Violated);

    let fixed = idealis_model::parse(WIDE).unwrap();
    let report = idealis_check::check(&fixed, "Loose", &Selection::Auto, small()).unwrap();
    assert_eq!(report.answers[0].checker, "reference");
    assert_eq!(report.answers[0].verdict, Verdict::Violated);
    assert_eq!(report.answers[0].depth, Some(2));
}

#[test]
fn sat_answers_within_its_bound() {
    let model = idealis_model::parse(
        "model Deep
sig A
var x: set A
init { no x }
action add(a: A) { x' = x + a }
invariant Small { #x < 3 }
scope A = 3, steps = 2",
    )
    .unwrap();
    let report = idealis_check::check(&model, "Small", &Selection::All, Budget::default()).unwrap();
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
    let model = idealis_model::parse(WIDE).unwrap();
    assert_eq!(
        idealis_check::check(&model, "Nope", &Selection::Auto, Budget::default()).unwrap_err(),
        Error::NoQuestion("Nope".into())
    );
    assert_eq!(
        idealis_check::check(
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
    let model = idealis_model::parse(MANY).unwrap();
    let started = std::time::Instant::now();
    let report = idealis_check::check(
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
fn normal_checks_use_fixed_initial_values_in_the_contention_model() {
    let path =
        root().join("examples/model-patterns/crash-recovery/receipt-restore-contention.ideal");
    let source = std::fs::read_to_string(path).unwrap();
    let budget = Budget {
        states: 20_000,
        ..Budget::default()
    };
    let run = |source: &str, name, expected, depth| {
        let model = idealis_model::parse(source).unwrap();
        let report = idealis_check::check(&model, name, &Selection::All, budget).unwrap();
        assert!(!report.disagreement);
        assert_eq!(report.answers.len(), 3);
        for answer in report.answers {
            assert_eq!(answer.verdict, expected, "{}: {name}", answer.checker);
            assert_eq!(answer.depth, depth, "{}: {name}", answer.checker);
        }
    };
    run(&source, "UniqueAssignments", Verdict::Holds, None);
    run(&source, "ContendersPublish", Verdict::Reachable, Some(8));
    let unchecked = source.replace("if { id = high + 1 }", "if { true }");
    assert_ne!(unchecked, source);
    run(&unchecked, "UniqueAssignments", Verdict::Violated, Some(4));
}

#[test]
fn explicit_configurations_count_against_the_budget() {
    let model = idealis_model::parse(MANY).unwrap();
    let started = std::time::Instant::now();
    let report = idealis_check::check(
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
    let model = idealis_model::parse(ESCAPING).unwrap();
    let report =
        idealis_check::check(&model, "NonNegative", &Selection::All, Budget::default()).unwrap();
    let checkers: Vec<_> = report.answers.iter().map(|a| a.checker).collect();
    assert_eq!(checkers, ["reference", "explicit", "rprob"]);
    for answer in &report.answers {
        assert_eq!(answer.verdict, Verdict::Unknown, "{answer:?}");
        let wire = serde_json::to_value(answer).unwrap();
        assert_eq!(wire["diagnostics"][0]["kind"], "scope_exit", "{wire}");
        assert!(wire["diagnostics"][0]["depth"].as_u64().is_some());
    }
    let wire = serde_json::to_value(&report).unwrap();
    assert_eq!(wire["unsupported"][0]["checker"], "sat", "{wire}");
    assert!(
        wire["unsupported"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("integer")
    );
    let rich = idealis_check::check(&model, "Rich", &Selection::All, Budget::default()).unwrap();
    for answer in &rich.answers {
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&Verdict::Reachable, Some(1)),
            "{answer:?}"
        );
    }
    match idealis_check::check(
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

#[test]
fn cli_reports_the_state_budget_and_unsupported_engine_separately() {
    let model = root().join("fixtures/rcheck/bank.model");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .arg(model)
        .args([
            "--checker",
            "all",
            "--question",
            "NonNegative",
            "--max-states",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let report = &response[0];
    assert_eq!(report["unsupported"][0]["checker"], "sat");
    assert_eq!(report["answers"][0]["verdict"], "unknown");
    assert_eq!(report["answers"][0]["diagnostics"][0]["resource"], "states");
    assert_eq!(report["disagreement"], false);
}

#[test]
fn alloy_declaration_capacity_is_reported_before_running_sat() {
    let model = root().join("examples/model-patterns/crash-recovery/decoder-retirement.ideal");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .arg(model)
        .args([
            "--checker",
            "all",
            "--question",
            "RetiredOnlyAfterAllOldTraffic",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reports: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let report = &reports[0];
    let answers = report["answers"].as_array().unwrap();
    assert_eq!(answers.len(), 3, "{report}");
    assert!(answers.iter().all(|answer| answer["verdict"] == "holds"));
    assert_eq!(report["unsupported"][0]["checker"], "sat");
    assert_eq!(
        report["unsupported"][0]["reason"],
        "Alloy declaration limit: 9 fields exceed the limit of 8"
    );
}

#[test]
fn sat_preflight_matches_the_generated_alloy_at_all_declaration_boundaries() {
    let cases = [
        ("signatures", 13, "{enums}"),
        ("fields", 8, "enum S = Atom"),
        ("predicates and assertions", 64, "enum S = Atom"),
        ("commands", 63, "enum S = Atom"),
    ];
    for (resource, at_limit, prefix) in cases {
        for count in [at_limit, at_limit + 1] {
            let mut text = format!("model Capacity {prefix}\n");
            match resource {
                "signatures" => {
                    let members = |range: std::ops::Range<usize>| {
                        range
                            .map(|index| format!("Member{index}"))
                            .collect::<Vec<_>>()
                            .join(" | ")
                    };
                    text = text.replace(
                        "{enums}",
                        &format!(
                            "enum S = {}
enum Other = {}",
                            members(0..7),
                            members(7..count)
                        ),
                    );
                }
                "fields" => {
                    for index in 0..count {
                        text.push_str(&format!("var field{index}: set S\n"));
                    }
                }
                "predicates and assertions" => {
                    for index in 0..count {
                        text.push_str(&format!("behavior act{index}() {{}}\n"));
                    }
                }
                "commands" => {
                    for index in 1..count {
                        let kind = if index % 2 == 0 {
                            "transition"
                        } else {
                            "invariant"
                        };
                        text.push_str(&format!("{kind} Test{index} {{ some S }}\n"));
                    }
                }
                _ => unreachable!(),
            }
            text.push_str("invariant Safe { some S } scope steps = 2");
            let model = idealis_model::parse(&text).unwrap();
            let direct = ralloy::Model::parse(&idealis_model::to_alloy(&model));
            let preflight = idealis_check::check(
                &model,
                "Safe",
                &Selection::Named("sat".into()),
                Budget { sat: 0, ..small() },
            );
            if count == at_limit {
                let report = preflight.unwrap();
                assert!(report.unsupported.is_empty(), "{resource}");
                let answer = &report.answers[0];
                if let Err(error) = direct {
                    // Declaration eligibility does not imply that type checking succeeds.
                    assert_eq!(resource, "predicates and assertions", "{error}");
                    assert_eq!(error.message, "type checking limit");
                    assert_eq!(answer.verdict, Verdict::Error);
                    assert_eq!(answer.detail.as_deref(), Some(error.to_string().as_str()));
                } else {
                    assert_ne!(answer.verdict, Verdict::Error, "{resource}");
                }
            } else {
                let direct = direct.err().expect("declaration cap");
                assert_eq!(direct.kind, ralloy::ErrorKind::Resource);
                assert!(direct.message.contains(resource), "{}", direct.message);
                let Error::Unsupported(_, reasons) = preflight.unwrap_err() else {
                    panic!("{resource}: expected SAT to be unavailable");
                };
                assert_eq!(reasons, vec![("sat", direct.message)]);
            }
        }
    }
}

#[test]
fn sat_capacity_excludes_transition_commands_omitted_by_the_lowering() {
    let mut source = "model Capacity sig S sig T sig U invariant Safe { some S }".to_owned();
    for index in 0..64 {
        source.push_str(&format!(" transition Count{index} {{ #S > 0 }}"));
    }
    source.push_str(" scope S = 6, T = 6, U = 6, steps = 2");
    let model = idealis_model::parse(&source).unwrap();
    assert!(ralloy::Model::parse(&idealis_model::to_alloy(&model)).is_ok());
    let selection = Selection::Named("sat".into());
    let budget = Budget { sat: 0, ..small() };
    let state = idealis_check::check(&model, "Safe", &selection, budget).unwrap();
    assert_ne!(state.answers[0].verdict, Verdict::Error);
    let Error::Unsupported(_, reasons) =
        idealis_check::check(&model, "Count0", &selection, budget).unwrap_err()
    else {
        panic!("transition scope should be unsupported");
    };
    assert!(reasons[0].1.contains("counts exceed"), "{reasons:?}");
}

#[test]
fn sat_distinguishes_an_empty_model_from_an_unreachable_goal() {
    const SOURCE: &str = "model Selection
enum Member = Ren
var selected: lone Member
init { no selected }
reachable Selected { some selected }
scope steps = 2";
    let empty = SOURCE.replace(
        "init { no selected }",
        "init { no selected, some selected }",
    );
    let reached = SOURCE.replace(
        "reachable Selected",
        "behavior choose() { update { selected = Ren } }\nreachable Selected",
    );
    let no_config = SOURCE.replace(
        "init { no selected }",
        "fact { no Member }\ninit { no selected }",
    );
    let broken = "model Selection
enum Member = Ren
var selected: one Member
init { selected = Ren }
behavior forget() { update { selected = none } }
reachable Selected { no selected }
scope steps = 2";
    for (source, expected) in [
        (SOURCE, Verdict::Unreachable),
        (&empty, Verdict::Empty),
        (&no_config, Verdict::Empty),
        (&reached, Verdict::Reachable),
        (broken, Verdict::Unreachable),
    ] {
        let model = idealis_model::parse(source).unwrap();
        for checker in ["reference", "explicit", "sat", "rprob"] {
            let report = idealis_check::check(
                &model,
                "Selected",
                &Selection::Named(checker.into()),
                Budget::default(),
            )
            .unwrap();
            assert_eq!(report.answers[0].verdict, expected, "{checker}: {source}");
        }
    }
}

#[test]
fn sat_spends_only_the_remaining_budget_to_check_for_initial_states() {
    let model = idealis_model::parse(
        "model Selection
enum Member = Ren
var selected: lone Member
init { no selected }
reachable Selected { some selected }
scope steps = 2",
    )
    .unwrap();
    let alloy = ralloy::Model::parse(&idealis_model::to_alloy(&model)).unwrap();
    let index = alloy
        .commands()
        .iter()
        .position(|command| command.label == idealis_model::alloy_label("Selected"))
        .unwrap();
    let run = alloy.analyze(index, Budget::default().sat).unwrap();
    assert_eq!(run.status, ralloy::Status::NoInstance);
    let report = idealis_check::check(
        &model,
        "Selected",
        &Selection::Named("sat".into()),
        Budget {
            sat: run.stats.work,
            ..Budget::default()
        },
    )
    .unwrap();
    assert_eq!(report.answers[0].verdict, Verdict::Unknown);
    let diagnostic = serde_json::to_value(&report.answers[0].diagnostics[0]).unwrap();
    assert_eq!(diagnostic["kind"], "budget");
    assert_eq!(diagnostic["resource"], "work");
}
