#[test]
fn the_self_spec_links_revision_provenance_to_adoption() {
    let spec = rspec::from_json(include_str!("../../../spec/rmodel-design.json")).unwrap();
    let rule = spec
        .rules
        .iter()
        .find(|rule| rule.id == "RMODEL-EVIDENCE-PROVENANCE");
    assert!(
        rule.is_some(),
        "the adoption rule must reference the versioned model"
    );
}

use rspec::{
    AdoptionBlocker::*, EvaluationRevision, Evidence, GoalMeasurement, SemanticComparison,
};
use serde_json::{Value, json};
use std::{fs, path::Path};

const SPEC: &str = include_str!("../../../spec/rmodel-design.json");
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const RULE: &str = "RMODEL-EVIDENCE-PROVENANCE";
const MODEL: &str = "spec/models/evidence-adoption.model";

fn report(spec: &Value, root: &Path, budget: usize) -> rspec::ClauseReport {
    let spec = rspec::from_json(&spec.to_string()).unwrap();
    rspec::clause_report(&spec, root, RULE, budget).unwrap()
}

fn evidence<T>(revision: &EvaluationRevision, outcome: T) -> Evidence<T> {
    Evidence {
        revision: revision.clone(),
        outcome,
    }
}

#[test]
fn a_late_result_cannot_authorize_the_current_candidate() {
    let root = Path::new(ROOT);
    let temp = std::env::temp_dir().join(format!("rspec-adoption-{}", std::process::id()));
    fs::create_dir(&temp).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let isolated = Cleanup(temp);
    let path = isolated.0.join(MODEL);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let original = fs::read_to_string(root.join(MODEL)).unwrap();
    fs::write(&path, &original).unwrap();

    let mut spec: Value = serde_json::from_str(SPEC).unwrap();
    let unreviewed = report(&spec, &isolated.0, 100_000);
    let current = EvaluationRevision::for_report("candidate-2", &unreviewed);
    let old_candidate = EvaluationRevision::for_report("candidate-1", &unreviewed);
    let good_goal = evidence(&current, GoalMeasurement::Improved);
    let fresh_comparison = evidence(&current, SemanticComparison::Equivalent);
    assert_eq!(
        rspec::assess_adoption(
            &current,
            &evidence(&current, &unreviewed),
            &fresh_comparison,
            &good_goal
        ),
        Err(vec![ReviewIncomplete])
    );

    let rule = spec["spec"]["rules"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|rule| rule["id"] == RULE)
        .unwrap();
    for (clause, item) in rule["clauses"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(&unreviewed.clauses)
    {
        clause["reviewed"] = json!(item.revision.as_deref().unwrap());
    }
    let approved = report(&spec, &isolated.0, 100_000);
    assert_eq!(approved.evaluation_revision, current.evaluation_revision);
    let review = evidence(&current, &approved);
    let stale_agreement = evidence(&old_candidate, SemanticComparison::Equivalent);
    assert_eq!(
        rspec::assess_adoption(&current, &review, &stale_agreement, &good_goal),
        Err(vec![ComparisonFromOtherRevision])
    );
    for status in [
        SemanticComparison::Diverges,
        SemanticComparison::Empty,
        SemanticComparison::Incomplete,
        SemanticComparison::Missing,
    ] {
        assert_eq!(
            rspec::assess_adoption(&current, &review, &evidence(&current, status), &good_goal),
            Err(vec![ComparisonNotEquivalent])
        );
    }
    assert_eq!(
        rspec::assess_adoption(
            &current,
            &review,
            &fresh_comparison,
            &evidence(&current, GoalMeasurement::Regressed)
        ),
        Err(vec![GoalNotImproved])
    );
    assert_eq!(
        rspec::assess_adoption(
            &current,
            &evidence(&old_candidate, &approved),
            &fresh_comparison,
            &evidence(&old_candidate, GoalMeasurement::Improved)
        ),
        Err(vec![ReviewFromOtherRevision, GoalFromOtherRevision])
    );
    assert_eq!(
        rspec::assess_adoption(&current, &review, &fresh_comparison, &good_goal),
        Ok(())
    );

    finite_model_agrees_with_the_gate(&original, &approved, &unreviewed);

    let limited = report(&spec, &isolated.0, 0);
    assert_eq!(
        rspec::assess_adoption(
            &current,
            &evidence(&current, &limited),
            &fresh_comparison,
            &good_goal
        ),
        Err(vec![ReviewIncomplete])
    );

    // Strengthening a guard keeps the correspondence revisions current.
    // Previously collected evidence nevertheless belongs to the old run.
    let modified = original.replace(
        "guard { current = First }",
        "guard { current = First some improved }",
    );
    assert_ne!(original, modified);
    fs::write(&path, modified).unwrap();
    let edited_report = report(&spec, &isolated.0, 100_000);
    assert!(edited_report.attention.is_empty());
    let edited = EvaluationRevision::for_report("candidate-2", &edited_report);
    assert_ne!(edited, current);
    assert_eq!(
        rspec::assess_adoption(&edited, &review, &fresh_comparison, &good_goal),
        Err(vec![
            ReviewFromOtherRevision,
            ComparisonFromOtherRevision,
            GoalFromOtherRevision
        ])
    );
    // Relabelling a clause report cannot make its source revision current.
    assert_eq!(
        rspec::assess_adoption(
            &edited,
            &evidence(&edited, &approved),
            &evidence(&edited, SemanticComparison::Equivalent),
            &evidence(&edited, GoalMeasurement::Improved)
        ),
        Err(vec![ReviewFromOtherRevision])
    );
}

fn finite_model_agrees_with_the_gate(
    source: &str,
    approved: &rspec::ClauseReport,
    unreviewed: &rspec::ClauseReport,
) {
    use rmodel::reference::{Data, Machine, Next, Snapshot};

    let model = rmodel::parse(source).unwrap();
    let machine = Machine::new(&model).unwrap();
    let config = machine.configurations().next().unwrap();
    let initial = machine.initial(&config).pop().unwrap();
    let next = |state: &Snapshot, action: &str, version: Option<&str>| {
        machine
            .match_step(
                &config,
                state,
                action,
                |_, arg| matches!(arg, Data::Atom(name) if Some(name.as_str()) == version),
            )
            .unwrap()
    };
    let apply = |state: &Snapshot, action, version| match next(state, action, version) {
        Some(Next::State(state)) => state,
        _ => panic!("{action} should be enabled for {version:?}"),
    };

    for current in ["First", "Second"] {
        let current_key = EvaluationRevision::for_report(current, approved);
        let versions = [None, Some("First"), Some("Second")];
        let versions = if current == "First" {
            &versions[..2]
        } else {
            &versions
        };
        let key_for = |origin: Option<&str>| {
            EvaluationRevision::for_report(origin.unwrap_or(current), approved)
        };
        for &review in versions {
            for &comparison in versions {
                for &goal in versions {
                    let mut state = initial.clone();
                    if current == "Second" {
                        state = apply(&state, "revise", None);
                    }
                    if review.is_some() {
                        state = apply(&state, "recordReview", review);
                    }
                    if let Some(origin) = comparison {
                        let action = if origin == current {
                            "recordSemantics"
                        } else {
                            "receiveLateSemantics"
                        };
                        state = apply(&state, action, comparison);
                    }
                    if goal.is_some() {
                        state = apply(&state, "recordGoal", goal);
                    }
                    let expected = matches!(next(&state, "adopt", None), Some(Next::State(_)));
                    let review_evidence = evidence(
                        &key_for(review),
                        if review.is_some() {
                            approved
                        } else {
                            unreviewed
                        },
                    );
                    let comparison_evidence = evidence(
                        &key_for(comparison),
                        if comparison.is_some() {
                            SemanticComparison::Equivalent
                        } else {
                            SemanticComparison::Missing
                        },
                    );
                    let goal_evidence = evidence(
                        &key_for(goal),
                        if goal.is_some() {
                            GoalMeasurement::Improved
                        } else {
                            GoalMeasurement::Unmeasured
                        },
                    );
                    let actual = rspec::assess_adoption(
                        &current_key,
                        &review_evidence,
                        &comparison_evidence,
                        &goal_evidence,
                    )
                    .is_ok();
                    assert_eq!(
                        actual, expected,
                        "{current}: {review:?}, {comparison:?}, {goal:?}"
                    );
                }
            }
        }
    }
}
