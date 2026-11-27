#[test]
fn comparison_policy_is_part_of_the_self_specification() {
    let spec = idealis_spec::from_json(include_str!("../../../spec/rmodel-design.json")).unwrap();
    assert!(
        spec.rules
            .iter()
            .any(|rule| rule.id == "RMODEL-EXPECTED-CHECKERS")
    );
}

use idealis_spec::{
    AdoptionBlocker, Checker, ComparisonPlan, Evidence, GoalMeasurement, SemanticComparison,
    compare_receipts, run_checker,
};
use std::path::Path;

const SPEC: &str = include_str!("../../../spec/rmodel-design.json");
const MODEL: &str = include_str!("../../../spec/models/evidence-adoption.model");
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const RULE: &str = "RMODEL-EVIDENCE-PROVENANCE";

fn collect_plan(plan: &ComparisonPlan) -> [idealis_spec::CheckerReceipt; 2] {
    let reference = run_checker(plan, Checker::Reference, MODEL, |model, budget| {
        Ok(rmodel::reference::analyze(model, budget))
    })
    .unwrap();
    let fast = run_checker(plan, Checker::Rprob, MODEL, |model, budget| {
        Ok(rprob::Compiled::new(model).unwrap().analyze(budget))
    })
    .unwrap();
    [reference, fast]
}

#[test]
fn real_checker_results_feed_adoption() {
    use SemanticComparison::*;
    let mut source: serde_json::Value = serde_json::from_str(SPEC).unwrap();
    let spec = idealis_spec::from_json(SPEC).unwrap();
    let root = Path::new(ROOT);
    let before = idealis_spec::clause_report(&spec, root, RULE, 100_000).unwrap();
    let clauses = source["spec"]["rules"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|rule| rule["id"] == RULE)
        .unwrap()["clauses"]
        .as_array_mut()
        .unwrap();
    for (clause, evidence) in clauses.iter_mut().zip(&before.clauses) {
        clause["reviewed"] = serde_json::json!(evidence.revision);
    }
    let spec = idealis_spec::from_json(&source.to_string()).unwrap();
    let approved = idealis_spec::clause_report(&spec, root, RULE, 100_000).unwrap();
    let plan = ComparisonPlan::pin("new", &approved, 100_000).unwrap();
    let [reference, fast] = collect_plan(&plan);

    let compare = |receipts: &[_]| compare_receipts(&plan, receipts).evidence.outcome;
    let review = Evidence {
        revision: plan.revision.clone(),
        outcome: &approved,
    };
    let goal = Evidence {
        revision: plan.revision.clone(),
        outcome: GoalMeasurement::Improved,
    };
    let assess = |receipts: &[_]| {
        idealis_spec::assess_adoption(
            &plan.revision,
            &review,
            &compare_receipts(&plan, receipts).evidence,
            &goal,
        )
    };

    assert_eq!(assess(&[reference.clone(), fast.clone()]), Ok(()));
    let stale_plan = ComparisonPlan::pin("old", &approved, 100_000).unwrap();
    let [_, stale_fast] = collect_plan(&stale_plan);
    let incomplete_pair = [reference.clone(), stale_fast];
    assert_eq!(compare(&incomplete_pair), Missing);
    assert_eq!(
        assess(&incomplete_pair),
        Err(vec![AdoptionBlocker::ComparisonNotEquivalent])
    );
    assert_eq!(compare(&[reference.clone(), reference.clone()]), Missing);
    assert_eq!(
        compare(&[fast.clone(), reference.clone(), reference.clone()]),
        Equivalent
    );

    let limited = ComparisonPlan::pin("new", &approved, 0).unwrap();
    let reports = collect_plan(&limited);
    assert_eq!(
        compare_receipts(&limited, &reports).evidence.outcome,
        Incomplete
    );
    assert_eq!(compare(&[reference.clone(), reports[1].clone()]), Missing);
    assert!(
        run_checker(
            &plan,
            Checker::Reference,
            &format!("// edit\n{MODEL}"),
            |_, _| unreachable!()
        )
        .is_err()
    );

    let faulty = run_checker(&plan, Checker::Rprob, MODEL, |model, budget| {
        let mut report = rprob::Compiled::new(model).unwrap().analyze(budget);
        report.verdicts[0].outcome = rmodel::reference::Outcome::Violated { depth: 1 };
        Ok(report)
    })
    .unwrap();
    assert_eq!(compare(&[reference.clone(), faulty.clone()]), Diverges);
    let conflict = compare_receipts(&plan, &[reference, fast, faulty]);
    assert_eq!(conflict.evidence.outcome, Incomplete);
    assert_eq!(conflict.conflicting_checkers, [Checker::Rprob]);
}

#[test]
fn empty_and_unsupported_are_distinct_from_agreement() {
    use sha2::{Digest, Sha256};

    let root = Path::new(ROOT);
    let spec = idealis_spec::from_json(SPEC).unwrap();
    let shipment = idealis_spec::clause_report(&spec, root, "SHIPMENT-TRACKING", 100_000).unwrap();
    let mut plan = ComparisonPlan::pin("candidate", &shipment, 100_000).unwrap();
    let mut source = std::fs::read_to_string(root.join(&plan.model)).unwrap();
    // Keep the shipment checks and add an unused domain beyond the bitset limit.
    let members = (0..65).map(|n| format!("Spill{n}")).collect::<Vec<_>>();
    source.push_str(&format!("\nenum Unused = {}\n", members.join(" | ")));
    plan.source_sha256 = format!("{:x}", Sha256::digest(&source));
    let reference = run_checker(&plan, Checker::Reference, &source, |model, budget| {
        Ok(rmodel::reference::analyze(model, budget))
    })
    .unwrap();
    let fast = run_checker(&plan, Checker::Rprob, &source, |model, _| {
        let error = rprob::Compiled::new(model)
            .err()
            .expect("oversized atom domain");
        assert!(error.to_string().contains("64 atoms"));
        Err(error.to_string())
    })
    .unwrap();
    assert_eq!(
        compare_receipts(&plan, &[reference, fast]).evidence.outcome,
        SemanticComparison::Incomplete
    );

    let source = MODEL.replace("current = First\n  no adopted", "false\n  no adopted");
    assert_ne!(source, MODEL);
    let mut plan = ComparisonPlan::pin(
        "empty-fixture",
        &idealis_spec::clause_report(&spec, root, RULE, 100_000).unwrap(),
        100_000,
    )
    .unwrap();
    plan.source_sha256 = format!("{:x}", Sha256::digest(&source));
    let reference = run_checker(&plan, Checker::Reference, &source, |model, budget| {
        Ok(rmodel::reference::analyze(model, budget))
    })
    .unwrap();
    let fast = run_checker(&plan, Checker::Rprob, &source, |model, budget| {
        Ok(rprob::Compiled::new(model).unwrap().analyze(budget))
    })
    .unwrap();
    assert_eq!(
        compare_receipts(&plan, &[reference, fast]).evidence.outcome,
        SemanticComparison::Empty
    );
}
