use serde_json::Value;
use std::path::Path;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const SPEC: &str = include_str!("../../../spec/rmodel-design.json");

fn shipment(spec: &mut Value) -> &mut Value {
    spec["spec"]["rules"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|rule| rule["id"] == "SHIPMENT-TRACKING")
        .unwrap()
}

#[test]
fn shipment_names_its_four_clause_checks() {
    let mut source: Value = serde_json::from_str(SPEC).unwrap();
    let clauses = shipment(&mut source)["clauses"]
        .as_array()
        .expect("shipment clauses");
    let names: Vec<_> = clauses
        .iter()
        .map(|clause| &clause["claim"]["check"])
        .collect();
    assert_eq!(
        names,
        [
            "StaysShipped",
            "KeepsTracking",
            "ConfirmCourier",
            "ConfirmPost"
        ]
    );
    let spec = idealis_spec::from_json(SPEC).unwrap();
    assert!(idealis_spec::links(&spec, Path::new(ROOT)).is_empty());
}

const MODEL: &str = "fixtures/rcheck/structured/shipped-transition.model";
const RULE: &str = "SHIPMENT-TRACKING";

fn report(source: &Value, root: &Path, budget: usize) -> idealis_spec::ClauseReport {
    let spec = idealis_spec::from_json(&source.to_string()).unwrap();
    idealis_spec::clause_report(&spec, root, RULE, budget).unwrap()
}

#[test]
fn reviews_follow_the_clause_and_model_checks_follow_the_current_behavior() {
    use idealis_spec::{LinkStatus, ReviewStatus};
    use serde_json::json;

    let root = Path::new(ROOT);
    let directory = std::env::temp_dir().join(format!("rspec-clauses-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let temp = Cleanup(directory);
    let path = temp.0.join(MODEL);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let original = std::fs::read_to_string(root.join(MODEL)).unwrap();
    std::fs::write(&path, &original).unwrap();
    let mut baseline: Value = serde_json::from_str(SPEC).unwrap();
    let initial = report(&baseline, &temp.0, 100);
    assert_eq!(initial.attention.len(), 4);
    for (index, entry) in initial.clauses.iter().enumerate() {
        assert_eq!(entry.link, LinkStatus::Resolved);
        assert_eq!(entry.review, ReviewStatus::Unreviewed);
        assert!(entry.check.as_ref().unwrap().matches_expectation);
        shipment(&mut baseline)["clauses"][index]["reviewed"] =
            json!(entry.revision.as_deref().unwrap());
    }
    assert!(report(&baseline, &temp.0, 100).attention.is_empty());

    let mut wording = baseline.clone();
    shipment(&mut wording)["clauses"][1]["text"]["ja"] = json!("確認時は追跡方法を変えてよい。");
    let changed = report(&wording, &temp.0, 100);
    assert_eq!(changed.attention, ["tracking"]);
    assert_eq!(changed.clauses[1].review, ReviewStatus::NeedsReview);
    assert_eq!(changed.clauses[1].check.as_ref().unwrap().outcome, "holds");

    let mut terms = baseline.clone();
    terms["spec"]["terms"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|term| term["id"] == "shipment-carrier")
        .unwrap()["definition"]["ja"] = json!("発送方法");
    assert_eq!(report(&terms, &temp.0, 100).attention.len(), 4);

    let without_tracking =
        original.replace("transition KeepsTracking {", "transition RenamedTracking {");
    std::fs::write(&path, without_tracking).unwrap();
    let missing = report(&baseline, &temp.0, 100);
    assert_eq!(missing.attention, ["tracking"]);
    assert_eq!(missing.clauses[1].link, LinkStatus::Invalid);
    assert!(missing.clauses[1].check.is_none());

    let mut no_claim = baseline.clone();
    shipment(&mut no_claim)["clauses"][1]["claim"] = Value::Null;
    assert_eq!(
        report(&no_claim, &temp.0, 100).clauses[1].link,
        LinkStatus::Unlinked
    );

    let weak_formula = original.replace(
        "{ payload(before(status), Shipped) = payload(after(status), Shipped) }",
        "{ true }",
    );
    assert_ne!(weak_formula, original);
    std::fs::write(&path, weak_formula).unwrap();
    let changed = report(&baseline, &temp.0, 100);
    assert_eq!(changed.attention, ["tracking"]);
    assert_eq!(changed.clauses[1].review, ReviewStatus::NeedsReview);
    assert_eq!(changed.clauses[1].check.as_ref().unwrap().outcome, "holds");

    let disable_courier = original.replace(
        "guard { is(status, Shipped) }",
        "guard { is(status, Shipped) payload(status, Shipped) = Post }",
    );
    std::fs::write(&path, disable_courier).unwrap();
    let changed = report(&baseline, &temp.0, 100);
    assert_eq!(changed.attention, ["confirm-courier"]);
    assert!(
        changed
            .clauses
            .iter()
            .all(|entry| entry.review == ReviewStatus::Reviewed)
    );
    assert_eq!(changed.clauses[1].check.as_ref().unwrap().outcome, "holds");
    assert_eq!(
        changed.clauses[2].check.as_ref().unwrap().outcome,
        "unreachable"
    );
    assert_eq!(
        changed.clauses[3].check.as_ref().unwrap().outcome,
        "reachable"
    );

    std::fs::write(&path, format!("// comment only\n{original}")).unwrap();
    assert!(report(&baseline, &temp.0, 100).attention.is_empty());
    let bounded = report(&baseline, &temp.0, 0);
    assert_eq!(bounded.attention.len(), 4);
    assert!(bounded.clauses.iter().all(|entry| {
        let check = entry.check.as_ref().unwrap();
        check.outcome == "unknown" && !check.matches_expectation
    }));

    let empty_initial = original.replace(
        "init { status = Ready() acknowledged = false }",
        "init { false }",
    );
    std::fs::write(&path, empty_initial).unwrap();
    let empty = report(&baseline, &temp.0, 100);
    assert_eq!(empty.attention.len(), 4);
    assert!(empty.clauses.iter().all(|entry| {
        let check = entry.check.as_ref().unwrap();
        check.outcome == "empty" && !check.matches_expectation
    }));
}

#[test]
fn named_shipment_clauses_cover_the_aggregate_in_the_fixed_model() {
    use idealis_model::reference::Outcome::Holds;
    let root = Path::new(ROOT);
    let source = std::fs::read_to_string(root.join(MODEL)).unwrap();
    for replacement in [
        "acknowledged = true }",
        "acknowledged = true status = Shipped(Post) }",
        "acknowledged = true status = Ready() }",
    ] {
        let edited = source.replace("acknowledged = true }", replacement);
        let model = idealis_model::parse(&edited).unwrap();
        let verdicts = idealis_model::reference::evaluate(&model, 100);
        let outcome = |name| {
            verdicts
                .iter()
                .find(|verdict| verdict.check == name)
                .unwrap()
                .outcome
        };
        assert_eq!(
            outcome("ShippedStays") == Holds,
            outcome("StaysShipped") == Holds && outcome("KeepsTracking") == Holds,
            "{verdicts:?}"
        );
    }
}
