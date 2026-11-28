use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output};

const AUTOSAVE: &str = "examples/model-patterns/autosave/autosave.ideal";
const CHECKOUT: &str = "examples/model-patterns/checkout/checkout.ideal";
const TERMINAL: &str = "examples/model-patterns/stream-terminal/split-terminal.ideal";
const DRAIN: &str = "examples/model-patterns/subscriptions/delivery-ticket-drain.ideal";

fn invoke(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args(args)
        .output()
        .unwrap()
}

fn report(args: &[&str]) -> Value {
    let output = invoke(args);
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn question(report: &Value, name: &str) -> Value {
    report["questions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["question"]["name"] == name)
        .unwrap()
        .clone()
}

#[test]
fn scope_witness_is_discoverable_and_replays_the_first_out_of_scope_write() {
    let model = "fixtures/rcheck/feed-scope.model";
    let capabilities = report(&["capabilities", model]);
    let scope = &capabilities["scopeWitness"];
    assert_eq!(
        scope,
        &json!({
            "question": "Scope",
            "reference": {"status": "supported"},
            "rprob": {"status": "supported"}
        })
    );
    assert!(
        capabilities["questions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["question"]["name"] != "Scope")
    );

    let name = scope["question"].as_str().unwrap();
    let reference = report(&["witness", model, name]);
    let rprob = report(&["witness", model, name, "--checker", "rprob"]);
    assert_eq!(reference, rprob);
    assert_eq!(rprob["verdict"], "violated");
    assert_eq!(rprob["complete"], false);
    assert_eq!(
        rprob["diagnostics"],
        json!([{"kind": "scope_exit", "depth": 3}])
    );
    assert_eq!(
        rprob["trace"]["commands"],
        json!(vec![json!({"kind":"append"}); 3])
    );
    assert_eq!(
        rprob["trace"]["finalState"],
        json!({"head": 3, "sourceTotal": 6})
    );
}

#[test]
fn finite_numbers_report_the_same_unsupported_reasons_as_the_execution_paths() {
    let name = "DuplicateIsIdempotent";
    let row = question(&report(&["capabilities", CHECKOUT]), name);
    assert_eq!(row["question"]["kind"], "transition");
    let executed = report(&[
        CHECKOUT,
        "--question",
        name,
        "--checker",
        "all",
        "--max-states",
        "1",
    ]);
    let engines = row["checkers"].as_array().unwrap();
    for (capability, answer) in engines[..2]
        .iter()
        .zip(executed[0]["answers"].as_array().unwrap())
    {
        assert_eq!(capability["checker"], answer["checker"]);
        assert_eq!(capability["status"], "supported");
        assert_eq!(capability["coverage"], answer["coverage"]);
        assert_ne!(answer["verdict"], "holds");
        assert!(capability.get("verdict").is_none());
    }
    assert_eq!(engines[2]["checker"], "sat");
    assert_eq!(engines[2]["status"], "unsupported");
    assert_eq!(
        engines[2]["reason"],
        executed[0]["unsupported"][0]["reason"]
    );

    assert_eq!(row["rprobWitness"]["status"], "supported");
    let capture = report(&["witness", CHECKOUT, "CanCapture", "--checker", "rprob"]);
    assert_eq!(capture["verdict"], "reachable");
    assert_eq!(capture["complete"], true);
}

#[test]
fn sequence_support_covers_scalar_state_and_lazily_bound_value_arguments() {
    let capabilities = report(&["capabilities", DRAIN]);
    let sealed = question(&capabilities, "SealQueuedWhileRetiring");
    assert_eq!(sealed["rprobWitness"]["status"], "supported");
    let liveness = question(&capabilities, "SealedEventuallyReleases");
    assert_eq!(liveness["rprobWitness"]["status"], "supported");
    let progress = report(&[
        "witness",
        DRAIN,
        "SealedEventuallyReleases",
        "--checker",
        "rprob",
    ]);
    assert_eq!(progress["verdict"], "holds");
    let limited = report(&[
        "witness",
        DRAIN,
        "SealedEventuallyReleases",
        "--checker",
        "rprob",
        "--max-states",
        "1",
    ]);
    assert_eq!(limited["verdict"], "unknown");
    assert_eq!(limited["complete"], false);
    assert_eq!(limited["diagnostics"][0]["kind"], "budget");
    assert_eq!(limited["diagnostics"][0]["resource"], "states");

    let nested = "examples/model-patterns/node-event-loop/nested-io-batch.ideal";
    let name = "ObservedBatch";
    let row = question(&report(&["capabilities", nested]), name);
    assert_eq!(row["rprobWitness"]["status"], "supported");
    let reference = report(&["witness", nested, name]);
    let rprob = report(&["witness", nested, name, "--checker", "rprob"]);
    assert_eq!(reference, rprob);
    assert_eq!(rprob["verdict"], "reachable");
    assert_eq!(
        rprob["trace"]["commands"],
        json!([{"kind": "drainIoBatch"}])
    );

    let restored = "examples/model-patterns/crash-recovery/restored-queue.ideal";
    let name = "RestoresFullQueue";
    let row = question(&report(&["capabilities", restored]), name);
    assert_eq!(row["rprobWitness"]["status"], "supported");
    let reference = report(&["witness", restored, name]);
    let rprob = report(&["witness", restored, name, "--checker", "rprob"]);
    assert_eq!(reference, rprob);
    assert_eq!(rprob["trace"]["commands"], json!([{"kind": "restart"}]));

    let refill = "examples/model-patterns/crash-recovery/refill-queue.ideal";
    let name = "Refilled";
    let capabilities = report(&["capabilities", refill]);
    assert_eq!(
        question(&capabilities, name)["rprobWitness"]["status"],
        "supported"
    );
    assert_eq!(capabilities["scopeWitness"]["rprob"]["status"], "supported");
    let reference = report(&["witness", refill, name]);
    let rprob = report(&["witness", refill, name, "--checker", "rprob"]);
    assert_eq!(reference, rprob);

    let limited = report(&[
        "witness",
        refill,
        name,
        "--checker",
        "rprob",
        "--max-arguments",
        "1369",
        "--max-states",
        "4",
    ]);
    assert_eq!(limited["verdict"], "unknown");
    assert_eq!(limited["complete"], false);
    assert_eq!(
        limited["diagnostics"],
        json!([{"kind": "budget", "resource": "arguments"}])
    );
}

#[test]
fn an_optional_saved_revision_is_supported_by_rprob() {
    let name = "StaleKeepsSaved";
    let supported = question(&report(&["capabilities", AUTOSAVE]), name);
    assert_eq!(supported["rprobWitness"]["status"], "supported");
    let answer = report(&["witness", AUTOSAVE, name, "--checker", "rprob"]);
    assert_eq!(answer["complete"], true);
    assert_eq!(answer["verdict"], "holds");
}

#[test]
fn a_model_with_liveness_can_still_supply_a_finite_rprob_witness() {
    let capabilities = report(&["capabilities", TERMINAL]);
    let liveness = question(&capabilities, "EndEventuallyFollowsTheSeal");
    assert_eq!(liveness["question"]["kind"], "temporal");
    assert_eq!(liveness["checkers"][0]["status"], "unsupported");
    assert_eq!(liveness["checkers"][1]["status"], "supported");
    assert_eq!(liveness["checkers"][2]["status"], "unsupported");
    assert_eq!(liveness["rprobWitness"]["status"], "supported");

    let name = "ControlAndSecondOvertakeFirst";
    let reachable = question(&capabilities, name);
    assert_eq!(
        reachable["checkers"][2]["coverage"],
        json!({"kind":"bounded","steps":12})
    );
    assert_eq!(reachable["rprobWitness"]["status"], "supported");
    let witness = report(&["witness", TERMINAL, name, "--checker", "rprob"]);
    assert_eq!(witness["verdict"], "reachable");
    assert_eq!(witness["trace"]["commands"].as_array().unwrap().len(), 5);
}

#[test]
fn rprob_temporal_results_distinguish_group_and_instance_fairness() {
    let workers = "examples/model-patterns/worker-fairness/competitive.ideal";
    let caps = report(&["capabilities", workers]);
    for (name, expected) in [
        ("GroupStrong", "violated"),
        ("InstanceWeak", "violated"),
        ("InstanceStrong", "holds"),
    ] {
        let entry = question(&caps, name);
        assert_eq!(entry["rprobWitness"]["status"], "supported");
        let witness = report(&["witness", workers, name, "--checker", "rprob"]);
        let explicit = report(&["witness", workers, name]);
        assert_eq!(witness["verdict"], expected);
        assert_eq!(witness["verdict"], explicit["verdict"]);
        if expected == "violated" {
            let trace = &witness["trace"];
            assert!(
                trace["loopStart"].as_u64().unwrap()
                    < trace["commands"].as_array().unwrap().len() as u64
            );
        }
    }
}

#[test]
fn rprob_participates_in_regular_checks_and_preserves_the_replayed_lasso() {
    let workers = "examples/model-patterns/worker-fairness/competitive.ideal";
    let name = "GroupStrong";
    let report_ = report(&[workers, "--question", name, "--checker", "all"]);
    let answers = report_[0]["answers"].as_array().unwrap();
    assert_eq!(
        answers
            .iter()
            .map(|answer| answer["checker"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["explicit", "rprob"]
    );
    assert_eq!(report_[0]["disagreement"], false);
    let witness = report(&["witness", workers, name, "--checker", "rprob"]);
    assert_eq!(answers[1]["verdict"], "violated");
    assert_eq!(answers[1]["lasso"], witness["trace"]);
    let capabilities = report(&["capabilities", workers]);
    let known = question(&capabilities, name);
    let registered = known["checkers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|checker| checker["checker"] == "rprob")
        .unwrap();
    assert_eq!(registered["status"], known["rprobWitness"]["status"]);
    assert_eq!(registered["coverage"]["kind"], "complete");

    let model = idealis_model::parse(include_str!(
        "../../../examples/model-patterns/worker-fairness/competitive.ideal"
    ))
    .unwrap();
    let unverified = idealis_check::check(
        &model,
        name,
        &idealis_check::Selection::Named("rprob".into()),
        idealis_check::Budget {
            work: 1,
            ..idealis_check::Budget::default()
        },
    )
    .unwrap();
    assert_eq!(
        unverified.answers[0].verdict,
        idealis_check::Verdict::Unknown
    );
    assert!(
        unverified.answers[0]
            .detail
            .as_deref()
            .unwrap()
            .contains("verification work budget")
    );
    assert!(unverified.answers[0].lasso.is_none());

    let result = report(&[
        CHECKOUT,
        "--checker",
        "rprob",
        "--question",
        "CanCapture",
        "--expect",
        "reachable",
    ]);
    assert_eq!(result[0]["answers"][0]["depth"], 2);
    let limited = invoke(&[
        DRAIN,
        "--checker",
        "rprob",
        "--question",
        "SealedEventuallyReleases",
        "--max-states",
        "1",
        "--expect",
        "holds",
    ]);
    assert_eq!(limited.status.code(), Some(3));
    let report_: Value = serde_json::from_slice(&limited.stdout).unwrap();
    assert_eq!(report_[0]["answers"][0]["verdict"], "unknown");
    assert_eq!(
        report_[0]["answers"][0]["diagnostics"][0]["resource"],
        "states"
    );
    assert!(report_[0]["answers"][0].get("lasso").is_none());
}
