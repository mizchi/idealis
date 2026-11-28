use idealis_model::reference::{Data, Machine, Next, Outcome, Snapshot, analyze};

const SOURCE: &str = include_str!("../../../spec/models/rmodel-calibration.model");
const BUDGET: usize = 100;

fn verdict(source: &str, check: &str) -> Outcome {
    let model = idealis_model::parse(source).unwrap();
    let report = analyze(&model, BUDGET);
    assert_eq!(report.declarations, Outcome::Holds);
    assert_eq!(report.scope, Outcome::Holds);
    report
        .verdicts
        .into_iter()
        .find(|verdict| verdict.check == check)
        .unwrap()
        .outcome
}

#[test]
fn calibration_requires_both_kinds_of_evidence_and_can_finish() {
    for name in ["AdoptionNeedsEquivalence", "AdoptionNeedsImprovement"] {
        assert_eq!(verdict(SOURCE, name), Outcome::Holds, "{name}");
    }
    for (name, depth) in [
        ("EquivalentButUnmeasured", 1),
        ("ImprovedButDiverges", 2),
        ("ImprovedButEmpty", 2),
        ("ImprovedButIncomplete", 2),
        ("ImprovedButMissingComparison", 2),
        ("SupportedAdoption", 3),
    ] {
        assert_eq!(
            verdict(SOURCE, name),
            Outcome::Reachable { depth },
            "{name}"
        );
    }
}

#[test]
fn each_missing_adoption_gate_has_a_counterexample() {
    let guard = "guard { decision = Trial semantics = Equivalent goal = Improved }";
    assert_eq!(SOURCE.matches(guard).count(), 1);
    for (weakened, check) in [
        (
            "guard { decision = Trial goal = Improved }",
            "AdoptionNeedsEquivalence",
        ),
        (
            "guard { decision = Trial semantics = Equivalent }",
            "AdoptionNeedsImprovement",
        ),
    ] {
        let mutant = SOURCE.replacen(guard, weakened, 1);
        assert_eq!(verdict(&mutant, check), Outcome::Violated { depth: 2 });
    }
}

#[test]
fn the_checker_distinguishes_an_empty_model_from_an_incomplete_search() {
    let empty = SOURCE.replacen(
        "init { decision = Trial",
        "init { decision = Adopted decision = Trial",
        1,
    );
    assert_ne!(empty, SOURCE);
    for (source, budget, expected) in [
        (empty.as_str(), BUDGET, Outcome::Empty),
        (SOURCE, 1, Outcome::Unknown),
    ] {
        let model = idealis_model::parse(source).unwrap();
        let report = analyze(&model, budget);
        assert_eq!(report.declarations, expected);
        assert_eq!(report.scope, expected);
        assert_eq!(report.verdicts.len(), model.checks.len());
        assert!(
            report
                .verdicts
                .iter()
                .all(|verdict| verdict.outcome == expected)
        );
    }
}

fn step(
    machine: &Machine<'_>,
    config: &Snapshot,
    state: &Snapshot,
    action: &str,
    value: Option<&str>,
) -> Snapshot {
    machine
        .steps(config, state)
        .into_iter()
        .filter(|step| step.action == action)
        .find_map(|step| {
            let args_match = match (value, step.args.as_slice()) {
                (None, []) => true,
                (Some(value), [(_, Data::Atom(arg))]) => arg == value,
                _ => false,
            };
            match step.next {
                Some(Next::State(next)) if args_match => Some(next),
                _ => None,
            }
        })
        .unwrap_or_else(|| panic!("{action} should be enabled"))
}

fn revision_clears_evidence(source: &str) -> bool {
    let model = idealis_model::parse(source).unwrap();
    let machine = Machine::new(&model).unwrap();
    let config = machine.configurations().next().unwrap();
    let initial = machine.initial(&config);
    assert_eq!(initial.len(), 1);
    let equivalent = step(
        &machine,
        &config,
        &initial[0],
        "recordSemantics",
        Some("Equivalent"),
    );
    let improved = step(
        &machine,
        &config,
        &equivalent,
        "recordGoal",
        Some("Improved"),
    );
    let adopted = step(&machine, &config, &improved, "adopt", None);
    let revised = step(&machine, &config, &adopted, "revise", None);
    revised == initial[0]
}

#[test]
fn the_next_revision_must_collect_new_evidence() {
    assert!(revision_clears_evidence(SOURCE));
    let update = "update { decision = Trial semantics = Unchecked goal = Unmeasured }";
    assert_eq!(SOURCE.matches(update).count(), 1);
    for weakened in [
        "update { decision = Trial goal = Unmeasured }",
        "update { decision = Trial semantics = Unchecked }",
    ] {
        assert!(!revision_clears_evidence(
            &SOURCE.replacen(update, weakened, 1)
        ));
    }
}
