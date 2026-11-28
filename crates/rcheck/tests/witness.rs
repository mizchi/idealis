use serde_json::{Value, json};
use std::process::Command;

const MODEL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/model-patterns/outbox/delivery.model",
);

fn witness(model: &str, question: &str, budget: &str) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_rcheck"))
        .args(["witness", model, question, "--max-states", budget])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn a_generated_crash_path_replays_against_the_validator() {
    let result = witness(MODEL, "StoppedAfterSend", "20000");
    assert_eq!(result["verdict"], "reachable");
    let trace = &result["trace"];
    assert_eq!(
        trace["commands"],
        json!([
            {"kind": "commit", "request": "First"},
            {"kind": "send", "request": "First"},
            {"kind": "crash"}
        ])
    );
    let source = std::fs::read_to_string(MODEL).unwrap();
    let model = rmodel::parse(&source).unwrap();
    let validator = rcheck::conform::Validator::new(&model).unwrap();
    let mut session = validator
        .start(&trace["config"], &trace["initial"])
        .unwrap();
    let commands = trace["commands"].as_array().unwrap();
    for command in &commands[..2] {
        session.replay(command).unwrap();
    }
    assert_eq!(session.validate(&commands[2]).unwrap(), trace["finalState"]);

    let bounded = witness(MODEL, "StoppedAfterSend", "0");
    assert_eq!(bounded["verdict"], "unknown");
    assert_eq!(bounded["trace"], Value::Null);
}

#[test]
fn temporal_witness_replays_parameterized_edges_and_closes_its_loop() {
    let source = include_str!("../../../examples/model-patterns/worker-fairness/competitive.model");
    let model = rmodel::parse(source).unwrap();
    let result = rcheck::conform::witness(&model, "InstanceWeak", 20_000).unwrap();
    assert_eq!(result["verdict"], "violated");
    assert_eq!(result["complete"], true);
    let trace = &result["trace"];
    let commands = trace["commands"].as_array().unwrap();
    let loop_start = trace["loopStart"].as_u64().unwrap() as usize;
    let validator = rcheck::conform::Validator::new(&model).unwrap();
    let mut session = validator
        .start(&trace["config"], &trace["initial"])
        .unwrap();
    let mut state = trace["initial"].clone();
    let mut loop_state = state.clone();
    for (step, command) in commands.iter().enumerate() {
        if step == loop_start {
            loop_state = state.clone();
        }
        state = session.validate(command).unwrap();
        session.replay(command).unwrap();
    }
    assert_eq!(state, loop_state);
    assert_eq!(state, trace["finalState"]);
    assert!(commands.iter().any(|command| command["worker"] == "Left"));
    assert!(commands.iter().any(|command| command["worker"] == "Right"));
    assert!(loop_start < commands.len());
    let strong = rcheck::conform::witness(&model, "InstanceStrong", 20_000).unwrap();
    assert_eq!(strong["verdict"], "holds");
    assert_eq!(strong["trace"], Value::Null);
    let exhausted = rcheck::conform::witness(&model, "InstanceWeak", 0).unwrap();
    assert_eq!(exhausted["verdict"], "unknown");
    assert_eq!(exhausted["complete"], false);
    assert_eq!(exhausted["trace"], Value::Null);
    assert_eq!(exhausted["diagnostics"][0]["kind"], "budget");
}

#[test]
fn temporal_witness_preserves_a_stuttering_cycle() {
    let model = rmodel::parse(
        "model WaitForever var ready: one Bool init { ready = false }
         eventually Progress { ready } scope steps = 3",
    )
    .unwrap();
    let result = rcheck::conform::witness(&model, "Progress", 100).unwrap();
    assert_eq!(result["verdict"], "violated");
    assert_eq!(result["trace"]["commands"], json!([null]));
    assert_eq!(result["trace"]["loopStart"], 0);
    assert_eq!(result["trace"]["initial"], result["trace"]["finalState"]);
}

#[test]
fn a_transition_violation_keeps_the_final_resend_in_the_trace() {
    let source = std::fs::read_to_string(MODEL).unwrap().replace(
        "delivered = delivered + request sent = sent + request",
        "delivered = delivered + request sent = sent + request
         duplicated = if { request in delivered } then duplicated + request else duplicated",
    );
    let model = rmodel::parse(&source).unwrap();
    let result = rcheck::conform::witness(&model, "ResendKeepsEffect", 20_000).unwrap();
    assert_eq!(result["verdict"], "violated");
    assert_eq!(result["complete"], true);
    let trace = &result["trace"];
    assert_eq!(trace["commands"].as_array().unwrap().len(), 5);
    let commands = trace["commands"].as_array().unwrap();
    assert_eq!(commands[4]["kind"], "send");
    assert_eq!(commands[4]["request"], commands[1]["request"]);
    assert_eq!(
        trace["finalState"]["duplicated"],
        json!([commands[4]["request"]])
    );
}

#[test]
fn an_initial_violation_still_has_a_witness() {
    let model =
        rmodel::parse("model Start enum Item = A invariant Broken { no Item } scope steps = 2")
            .unwrap();
    let result = rcheck::conform::witness(&model, "Broken", 100).unwrap();
    assert_eq!(result["verdict"], "violated");
    assert_eq!(result["trace"]["commands"], json!([]));
    assert_eq!(result["trace"]["initial"], result["trace"]["finalState"]);
}

#[test]
fn a_witness_found_before_the_budget_runs_out_is_marked_incomplete() {
    let model = rmodel::parse(
        "model Early
enum Phase = Start | Done
var phase: one Phase
init { phase = Start }
behavior finish() { guard { phase = Start } update { phase = Done } }
reachable Initial { phase = Start }
scope steps = 3",
    )
    .unwrap();
    // Configuration, the fixed initial candidate, and the initial reachable state.
    let result = rcheck::conform::witness(&model, "Initial", 3).unwrap();
    assert_eq!(result["verdict"], "reachable");
    assert_eq!(result["complete"], false);
    assert_eq!(result["trace"]["commands"], json!([]));
    assert_eq!(
        result["diagnostics"],
        json!([{ "kind": "budget", "resource": "states" }])
    );
}

#[test]
fn built_in_violations_include_the_outside_state() {
    for (source, question, final_state, commands) in [
        (
            "model Overflow var n: one Nat init { n = 0 }
             behavior increment() { update { n = n + 1 } }
             scope Int = 0..1, steps = 3",
            "Scope",
            json!({"n":2}),
            json!([{"kind":"increment"},{"kind":"increment"}]),
        ),
        (
            "model Multiplicity enum Flag = A | B var active: lone Flag init { no active }
             behavior fill() { update { active = Flag } }
             scope steps = 3",
            "Declarations",
            json!({"active":["A","B"]}),
            json!([{"kind":"fill"}]),
        ),
    ] {
        let model = rmodel::parse(source).unwrap();
        let result = rcheck::conform::witness(&model, question, 100).unwrap();
        assert_eq!(result["verdict"], "violated");
        assert_eq!(result["trace"]["commands"], commands);
        assert_eq!(result["trace"]["finalState"], final_state);
        assert_eq!(
            rcheck::conform::rprob_witness(&model, question, 100).unwrap(),
            result
        );
    }
}

#[test]
fn rprob_scope_witness_keeps_numeric_rows_and_other_simultaneous_writes() {
    let model = rmodel::parse(
        "model RowLimit
         sig Padding
         enum Key = First | Second
         rel cap: one Nat
         var positions: Key -> one Nat
         var pending: set Nat
         var crossed: one Bool
         fact { cap = 1 }
         init { positions = Key -> 0, no pending, not crossed }
         behavior increment(key: Key, delta: Nat) {
           guard { key = First, delta = cap }
           update {
             let next = key.positions + delta
             positions = positions ++ key -> next
             pending = union(pending, next)
             crossed = bool { next > cap }
           }
         }
         scope Padding = 7, Int = 0..1, steps = 4",
    )
    .unwrap();
    let expected = rcheck::conform::witness(&model, "Scope", 100).unwrap();
    let result = rcheck::conform::rprob_witness(&model, "Scope", 100).unwrap();
    assert_eq!(result, expected);
    assert_eq!(result["complete"], false);
    assert_eq!(
        result["trace"]["commands"],
        json!([
            {"kind":"increment","key":"First","delta":1},
            {"kind":"increment","key":"First","delta":1}
        ])
    );
    assert_eq!(
        result["trace"]["finalState"],
        json!({
            "positions":{"First":2,"Second":0},
            "pending":[1,2],
            "crossed":true
        })
    );
    let limited = rcheck::conform::rprob_witness(&model, "Scope", 1).unwrap();
    assert_eq!(limited["verdict"], "unknown");
    assert_eq!(limited["trace"], Value::Null);
}

#[test]
fn temporal_witness_keeps_the_chosen_arguments_when_edges_have_the_same_states() {
    let model = rmodel::parse(
        "model Ambiguous
         enum Label = A | B
         var side: one Bool
         init { side = false }
         behavior flip(label: Label) { update { side = if { side } then false else true } }
         eventually Never with { weak flip } { false }
         scope steps = 3",
    )
    .unwrap();
    let result = rcheck::conform::witness(&model, "Never", 2_000).unwrap();
    assert_eq!(result["verdict"], "violated");
    let trace = &result["trace"];
    let validator = rcheck::conform::Validator::new(&model).unwrap();
    let mut session = validator
        .start(&trace["config"], &trace["initial"])
        .unwrap();
    let loop_start = trace["loopStart"].as_u64().unwrap() as usize;
    let mut state = trace["initial"].clone();
    let mut loop_entry = state.clone();
    let mut flips = 0;
    for (i, command) in trace["commands"].as_array().unwrap().iter().enumerate() {
        if i == loop_start {
            loop_entry = state.clone();
        }
        if !command.is_null() {
            assert!(matches!(command["label"].as_str(), Some("A" | "B")));
            state = session.validate(command).unwrap();
            session.replay(command).unwrap();
            flips += 1;
        }
    }
    assert!(flips >= 2);
    assert_eq!(state, loop_entry);
    assert_eq!(state, trace["finalState"]);
}

#[test]
fn temporal_witness_matches_structured_values_and_static_relations() {
    let model = rmodel::parse(
        "model Structured
         enum Part = A | B
         sequence Parts = Part, max = 1
         record Payload { items: Parts, amount: Nat }
         variant Status = Waiting | Sent(Payload)
         rel chosen: Part -> one Part
         fact { chosen = (A -> B) + (B -> A) }
         var status: one Status
         init { status = Waiting() }
         behavior send(payload: Payload) {
           guard { status = Waiting() payload = Payload{ items: Parts[A], amount: 1 } }
           update { status = Sent(payload) }
         }
         behavior reset() {
           guard { is(status, Sent) }
           update { status = Waiting() }
         }
         eventually Never with { weak send weak reset } { false }
         scope Int = 0..1, steps = 3",
    )
    .unwrap();
    let result = rcheck::conform::witness(&model, "Never", 20_000).unwrap();
    assert_eq!(result["verdict"], "violated");
    assert_eq!(
        result["trace"]["config"]["chosen"],
        json!({"A":"B","B":"A"})
    );
    let trace = &result["trace"];
    let validator = rcheck::conform::Validator::new(&model).unwrap();
    let mut session = validator
        .start(&trace["config"], &trace["initial"])
        .unwrap();
    assert!(trace["commands"].as_array().unwrap().contains(&json!({
        "kind":"send", "payload": {"items":{"$sequence":["A"]},"amount":1}
    })));
    for command in trace["commands"].as_array().unwrap() {
        if !command.is_null() {
            session.replay(command).unwrap();
        }
    }
}

#[test]
fn temporal_witness_reports_when_grounded_actions_exceed_the_compiler_limit() {
    let model = rmodel::parse(
        "model Wide
         entity Worker
         var done: one Bool
         init { done = false }
         behavior mark(a: Worker, b: Worker, c: Worker, d: Worker, e: Worker) {
           update { done = true }
         }
         eventually Progress { done }
         scope Worker = 7, steps = 3",
    )
    .unwrap();
    let error = rcheck::conform::witness(&model, "Progress", 2_000).unwrap_err();
    assert!(error.contains("explicit action branch limit"), "{error}");
}

#[test]
fn fixed_initial_relations_leave_budget_for_a_queued_delivery_witness() {
    let model = rmodel::parse(include_str!(
        "../../../examples/model-patterns/subscriptions/queued.model"
    ))
    .unwrap();
    let result =
        rcheck::conform::witness(&model, "BothQueuedAcrossReregistration", 20_000).unwrap();
    assert_eq!(result["verdict"], "reachable");
    assert_eq!(result["complete"], true);
    let trace = &result["trace"];
    assert_eq!(trace["commands"].as_array().unwrap().len(), 5);
    assert_eq!(
        trace["finalState"]["pending"],
        json!({"First":["Old"], "Second":["Old"]})
    );
    assert_eq!(trace["finalState"]["active"], "New");
}

#[test]
fn witness_initial_binding_keeps_all_legal_alternatives() {
    let model = rmodel::parse(
        "model InitialValues
         enum Choice = A | B
         var selected: one Choice
         var saved: lone Choice
         init { no saved and (selected = A or selected = B) }
         reachable First { selected = A }
         reachable Second { selected = B }
         scope steps = 2",
    )
    .unwrap();
    for (question, value) in [("First", "A"), ("Second", "B")] {
        let result = rcheck::conform::witness(&model, question, 10).unwrap();
        assert_eq!(result["verdict"], "reachable");
        assert_eq!(result["complete"], true);
        assert_eq!(result["trace"]["commands"], json!([]));
        assert_eq!(result["trace"]["initial"]["selected"], value);
        assert_eq!(result["trace"]["initial"]["saved"], Value::Null);
    }
}

#[test]
fn queued_notifications_keep_each_job_and_its_original_registration() {
    use rcheck::{Budget, Selection, Verdict};
    let model = include_str!("../../../examples/model-patterns/subscriptions/queued.model");
    let reroute = model
        .replace(
            "if { registration in active } then",
            "if { some active } then",
        )
        .replace(
            "delivered = delivered + notice -> registration",
            "delivered = delivered + notice -> active",
        );
    let collapse = model.replace(
        "pending = pending - notice -> registration",
        "pending = pending - Notice -> registration",
    );
    for (source, question, verdict, depth) in [
        (model, "EveryCapturedPairAccountedFor", Verdict::Holds, None),
        (model, "NoDeliveryToAReplacement", Verdict::Holds, None),
        (
            reroute.as_str(),
            "NoDeliveryToAReplacement",
            Verdict::Violated,
            Some(5),
        ),
        (
            collapse.as_str(),
            "EveryCapturedPairAccountedFor",
            Verdict::Violated,
            Some(4),
        ),
    ] {
        let model = rmodel::parse(source).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget {
                states: 20_000,
                ..Budget::default()
            },
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 20_000).unwrap();
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        assert_eq!(witness["complete"], true);
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn acknowledgement_must_match_both_the_notice_and_its_generation() {
    use rcheck::{Budget, Selection, Verdict};
    let normal = include_str!("../../../examples/model-patterns/subscriptions/ack.model");
    let id_only = normal
        .replace(
            "if { notice -> generation in pending }",
            "if { some notice.pending }",
        )
        .replace(
            "settled = settled + notice -> generation",
            "settled = settled + notice -> notice.pending",
        )
        .replace(
            "pending = pending - notice -> generation",
            "pending = pending - notice -> Generation",
        );
    let generation_only = normal
        .replace(
            "settled = settled + notice -> generation",
            "settled = settled + (pending & Notice -> generation)",
        )
        .replace(
            "pending = pending - notice -> generation",
            "pending = pending - Notice -> generation",
        );
    for (source, question, verdict, depth) in [
        (normal, "StaleAckKeepsPending", Verdict::Holds, None),
        (normal, "AckKeepsOtherIds", Verdict::Holds, None),
        (
            id_only.as_str(),
            "StaleAckKeepsPending",
            Verdict::Violated,
            Some(5),
        ),
        (
            generation_only.as_str(),
            "AckKeepsOtherIds",
            Verdict::Violated,
            Some(4),
        ),
    ] {
        let model = rmodel::parse(source).unwrap();
        let checked = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget {
                states: 20_000,
                ..Budget::default()
            },
        )
        .unwrap();
        let answer = &checked.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 20_000).unwrap();
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        assert_eq!(witness["complete"], true);
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn wait_notify_distinguishes_a_lost_signal_from_a_wake_without_readiness() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/wait-notify/wait.model");
    let lost = source.replace(
        "reader = if { ready } then Ready else Parked",
        "reader = Parked",
    );
    let unchecked = source.replace("update { reader = Idle }", "update { reader = Ready }");
    for (source, question, verdict, depth) in [
        (source, "NoSleepAfterNotify", Verdict::Holds, None),
        (source, "OnlyReadyCanProceed", Verdict::Holds, None),
        (
            lost.as_str(),
            "NoSleepAfterNotify",
            Verdict::Violated,
            Some(4),
        ),
        (
            unchecked.as_str(),
            "OnlyReadyCanProceed",
            Verdict::Violated,
            Some(3),
        ),
    ] {
        let model = rmodel::parse(source).unwrap();
        let report = rcheck::check(&model, question, &Selection::All, Budget::default()).unwrap();
        for answer in report
            .answers
            .iter()
            .filter(|answer| answer.coverage == rcheck::Coverage::Complete)
        {
            assert_eq!(
                (&answer.verdict, answer.depth),
                (&verdict, depth),
                "{answer:?}"
            );
        }
        assert_eq!(
            report
                .answers
                .iter()
                .filter(|answer| answer.coverage == rcheck::Coverage::Complete)
                .count(),
            3
        );
    }

    let normal = rcheck::conform::witness(
        &rmodel::parse(source).unwrap(),
        "CompletesAfterNotify",
        20_000,
    )
    .unwrap();
    assert_eq!(normal["verdict"], "holds");
    let bad = rcheck::conform::witness(
        &rmodel::parse(&lost).unwrap(),
        "CompletesAfterNotify",
        20_000,
    )
    .unwrap();
    assert_eq!(bad["verdict"], "violated");
    let trace = &bad["trace"];
    let commands = trace["commands"].as_array().unwrap();
    let loop_start = trace["loopStart"].as_u64().unwrap() as usize;
    assert!(loop_start < commands.len());
    assert!(commands[loop_start..].iter().all(Value::is_null));
    assert_eq!(
        trace["finalState"],
        json!({"reader":"Parked","ready":true,"notified":true})
    );
}

#[test]
fn notifying_only_the_first_waiter_can_leave_the_ready_one_queued() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/wait-notify/shared-queue.model");
    let single = source.replace(
        "queue = Queue[]\n    woken = elements(queue)",
        "queue = tail(queue)\n    woken = at(queue, 0)",
    );
    for (source, verdict, depth) in [
        (source, Verdict::Holds, None),
        (single.as_str(), Verdict::Violated, Some(4)),
    ] {
        let model = rmodel::parse(source).unwrap();
        let report = rcheck::check(
            &model,
            "ReadyWaiterIsNotLeftQueued",
            &Selection::All,
            Budget::default(),
        )
        .unwrap();
        assert_eq!(report.answers.len(), 3);
        for answer in report.answers {
            assert_eq!(
                (&answer.verdict, answer.depth),
                (&verdict, depth),
                "{answer:?}"
            );
        }
        let liveness = rcheck::conform::witness(&model, "ReadyWorkerFinishes", 20_000).unwrap();
        assert_eq!(liveness["verdict"], serde_json::to_value(verdict).unwrap());
        assert_eq!(liveness["complete"], true);
        if depth.is_some() {
            let trace = &liveness["trace"];
            let loop_start = trace["loopStart"].as_u64().unwrap() as usize;
            let commands = trace["commands"].as_array().unwrap();
            let final_state = &trace["finalState"];
            assert!(loop_start < commands.len());
            assert!(commands[loop_start..].iter().all(Value::is_null));
            assert_eq!(
                final_state["queue"]["$sequence"],
                json!([final_state["favored"]])
            );
            assert_eq!(final_state["finished"], json!([]));
        }
    }
}

#[test]
fn timed_handoff_has_distinct_leak_and_double_reservation_witnesses() {
    use rcheck::{Budget, Selection, Verdict};
    let model = include_str!("../../../examples/model-patterns/lease/handoff.model");
    let leak = model.replace(
        "stage = stage ++ client -> Cancelled\n      reserved = reserved - client",
        "stage = stage ++ client -> Cancelled",
    );
    let double = model.replace("if { no reserved } then", "if { true } then");
    for (source, question, expected, depth) in [
        (model, "AtMostOneReservation", Verdict::Holds, None),
        (model, "OnlyOfferedOrHeldOccupies", Verdict::Holds, None),
        (model, "TimeoutKeepsAcceptedLease", Verdict::Holds, None),
        (
            leak.as_str(),
            "OnlyOfferedOrHeldOccupies",
            Verdict::Violated,
            Some(2),
        ),
        (
            double.as_str(),
            "AtMostOneReservation",
            Verdict::Violated,
            Some(2),
        ),
    ] {
        let model = rmodel::parse(source).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 20_000).unwrap();
        assert_eq!(witness["complete"], true);
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn store_cutover_carries_the_fence_and_keeps_the_sealed_source_frozen() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/lease/store-cutover.model");
    let body_only = source.replace("    floor = floor ++ Target -> Source.floor\n", "");
    let old_route_open = source.replace(
        "store = Source and phase = ServingSource",
        "store = Source and (phase = ServingSource or phase = ServingTarget)",
    );
    for (text, question, expected, depth) in [
        (source, "TargetKeepsFence", Verdict::Holds, None),
        (source, "SealedSourceStaysFrozen", Verdict::Holds, None),
        (source, "OldCannotWriteAfterCutover", Verdict::Holds, None),
        (&body_only, "TargetKeepsFence", Verdict::Violated, Some(3)),
        (
            &body_only,
            "OldCannotWriteAfterCutover",
            Verdict::Violated,
            Some(4),
        ),
        (
            &old_route_open,
            "SealedSourceStaysFrozen",
            Verdict::Violated,
            Some(4),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 20_000).unwrap();
        assert_eq!(witness["complete"], true);
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn change_feed_cutover_rejects_duplicate_effects_and_early_routing() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/lease/feed-cutover.model");
    let duplicate = source
        .replace(
            "if { position = cursor + 1 } then",
            "if { position <= cursor + 1 and total < 3 } then",
        )
        .replace(
            "cursor = position",
            "if { position > cursor } then { cursor = position }",
        );
    let early = source.replace("if { cursor = head } then", "if { true } then");
    for (text, question, expected, depth) in [
        (source, "EachDeltaAppliedOnce", Verdict::Holds, None),
        (source, "OnlyCaughtUpCanServe", Verdict::Holds, None),
        (source, "DuplicateDoesNotChangeData", Verdict::Holds, None),
        (
            &duplicate,
            "EachDeltaAppliedOnce",
            Verdict::Violated,
            Some(3),
        ),
        (
            &duplicate,
            "DuplicateDoesNotChangeData",
            Verdict::Violated,
            Some(3),
        ),
        (&early, "OnlyCaughtUpCanServe", Verdict::Violated, Some(3)),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 30_000).unwrap();
        assert_eq!(witness["complete"], true);
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn compacted_feed_distinguishes_a_missing_prefix_from_the_retention_boundary() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/lease/feed-compaction.model");
    let missing_gap = source.replace("then { gap = true }", "then { gap = false }");
    let off_by_one = source.replace("if { cursor < floor }", "if { cursor <= floor }");
    let live_position = source.replace("cursor = snapshotAt", "cursor = head");
    for (text, question, expected, depth) in [
        (source, "ReplicaValueMatchesPrefix", Verdict::Holds, None),
        (source, "ReportGapOnPull", Verdict::Holds, None),
        (source, "PullAtRetentionBoundary", Verdict::Holds, None),
        (&missing_gap, "ReportGapOnPull", Verdict::Violated, Some(3)),
        (
            &off_by_one,
            "PullAtRetentionBoundary",
            Verdict::Violated,
            Some(2),
        ),
        (
            &live_position,
            "ReplicaValueMatchesPrefix",
            Verdict::Violated,
            Some(6),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 30_000).unwrap();
        assert_eq!(witness["complete"], true);
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn resync_checks_request_epoch_and_capture_position_separately() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/lease/resync-order.model");
    let ignore_epoch = source.replace(
        "epoch = latest and epoch.captured >= cursor",
        "epoch.captured >= cursor",
    );
    let ignore_position = source.replace(
        "epoch = latest and epoch.captured >= cursor",
        "epoch = latest",
    );
    for (text, question, expected, depth) in [
        (
            source,
            "SupersededResponseLeavesReplicaUnchanged",
            Verdict::Holds,
            None,
        ),
        (source, "AcceptedPositionCannotRecede", Verdict::Holds, None),
        (
            &ignore_epoch,
            "SupersededResponseLeavesReplicaUnchanged",
            Verdict::Violated,
            Some(4),
        ),
        (
            &ignore_position,
            "AcceptedPositionCannotRecede",
            Verdict::Violated,
            Some(5),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 30_000).unwrap();
        assert_eq!(witness["complete"], true);
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn source_generation_resync_validates_a_fixed_wide_record_relation() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/lease/resync-source.model");
    let ignore_origin = source.replace(" and ticket.inbox.origin = active", "");
    let compare_across_origins = source.replace("saved.origin != active or ", "");
    for (text, question, expected, depth) in [
        (
            source,
            "WrongSourceDoesNotChangeReplica",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "CurrentSourceStartsItsOwnOffset",
            Verdict::Holds,
            None,
        ),
        (
            &ignore_origin,
            "WrongSourceDoesNotChangeReplica",
            Verdict::Violated,
            Some(7),
        ),
        (
            &compare_across_origins,
            "CurrentSourceStartsItsOwnOffset",
            Verdict::Violated,
            Some(4),
        ),
        (
            source,
            "WrongOriginAtSameOffset",
            Verdict::Reachable,
            Some(7),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 30_000).unwrap();
        assert_eq!(witness["complete"], true);
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }

    let invalid =
        rmodel::parse(&source.replace("Ticket -> lone Snapshot", "Ticket -> one Snapshot"))
            .unwrap();
    let question = "WrongOriginAtSameOffset";
    let witness = rcheck::conform::witness(&invalid, question, 30_000).unwrap();
    assert_eq!(witness["verdict"], "empty");
    assert_eq!(witness["complete"], true);
    assert!(witness["trace"].is_null());

    let valid = rmodel::parse(source).unwrap();
    let report = rmodel::reference::analyze(&valid, 10_000);
    assert!(report.verdicts.iter().any(|answer| {
        answer.check == question
            && answer.outcome == rmodel::reference::Outcome::Reachable { depth: 7 }
    }));

    let model =
        rmodel::parse(&source.replace("no latest no pending no inbox", "no latest no pending"))
            .unwrap();
    let error = rmodel::reference::enumerable(&model).unwrap_err();
    assert!(error.contains("inbox has 36 possible tuples"), "{error}");
}

#[test]
fn source_name_reuse_cannot_authorize_old_incarnation_reads_or_publication() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/lease/resync-reused-source.model");
    let publish_by_name = source.replace(
        "if { buffered.owner = active }",
        "if { buffered.owner.endpoint = active.endpoint }",
    );
    let read_by_name = source.replace(
        "if { saved.owner = active }",
        "if { saved.owner.endpoint = active.endpoint }",
    );
    for (text, question, expected, depth) in [
        (
            source,
            "OldIncarnationCannotReplaceReplica",
            Verdict::Holds,
            None,
        ),
        (source, "ReadChecksIncarnation", Verdict::Holds, None),
        (
            &publish_by_name,
            "OldIncarnationCannotReplaceReplica",
            Verdict::Violated,
            Some(4),
        ),
        (
            &read_by_name,
            "ReadChecksIncarnation",
            Verdict::Violated,
            Some(3),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 20_000).unwrap();
        assert_eq!(witness["complete"], true);
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn offline_merge_preserves_disjoint_edits_and_rejects_conflicts_atomically() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/offline-merge/merge.model");
    let replace = source.replace(
        "title: if { edit.title = editor.bases.title } then live.title else edit.title,",
        "title: edit.title,",
    );
    let last_writer = source.replace(
        "(edit.title = editor.bases.title or live.title = editor.bases.title or edit.title = live.title) and",
        "",
    );
    let partial = source.replace(
        "results = results ++ editor -> Conflict",
        "live = Document { title: live.title, label: edit.label }
         results = results ++ editor -> Conflict",
    );
    for (text, question, expected, depth) in [
        (
            source,
            "MergePreservesUntouchedFields",
            Verdict::Holds,
            None,
        ),
        (source, "MergeAppliesEditedFields", Verdict::Holds, None),
        (source, "DivergingChangesAreConflicts", Verdict::Holds, None),
        (
            source,
            "AConflictDoesNotChangeTheDocument",
            Verdict::Holds,
            None,
        ),
        (
            &replace,
            "MergePreservesUntouchedFields",
            Verdict::Violated,
            Some(4),
        ),
        (
            &last_writer,
            "DivergingChangesAreConflicts",
            Verdict::Violated,
            Some(4),
        ),
        (
            &partial,
            "AConflictDoesNotChangeTheDocument",
            Verdict::Violated,
            Some(4),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 20_000).unwrap();
        assert_eq!(witness["complete"], true);
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn offline_edit_retry_binds_the_answer_and_history_to_the_original_body() {
    use rcheck::Verdict;
    let source = include_str!("../../../examples/model-patterns/offline-merge/retry.model");
    let recompute = source.replace(
        "then request.receipts",
        "then (if { live = edit.baseValue or live = edit.desired } then request.receipts
          else Reply { outcome: Conflict, atRevision: revision })",
    );
    let duplicate = source.replace(
        "if { some request.edits } then {",
        "if { some request.edits } then {
           if { edit = request.edits and revision = 1 } then { revision = revision + 1 }",
    );
    let ignore_body = source.replace(
        "if { edit = request.edits } then request.receipts",
        "if { true } then request.receipts",
    );
    for (text, question, expected, depth) in [
        (source, "ExactRetryKeepsHistory", Verdict::Holds, None),
        (source, "ExactRetryUsesOriginalReply", Verdict::Holds, None),
        (
            source,
            "ReusedKeyKeepsOriginalReceipt",
            Verdict::Holds,
            None,
        ),
        (
            &duplicate,
            "ExactRetryKeepsHistory",
            Verdict::Violated,
            Some(3),
        ),
        (
            &recompute,
            "ExactRetryUsesOriginalReply",
            Verdict::Violated,
            Some(5),
        ),
        (
            &ignore_body,
            "ReusedKeyKeepsOriginalReceipt",
            Verdict::Violated,
            Some(3),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let witness = rcheck::conform::witness(&model, question, 20_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn retention_keeps_receipts_for_admitted_tickets_and_rejects_expired_edits() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/offline-merge/retention.model");
    let no_gate = source
        .replace(
            "ticket.issued < floor or some ticket.receipts",
            "some ticket.receipts",
        )
        .replacen("if { ticket.issued < floor }", "if { false }", 1);
    let collect_inclusive = source.replace(
        "if { ticket.issued < floor } then { receipts =",
        "if { ticket.issued <= floor } then { receipts =",
    );
    for (text, question, expected, depth) in [
        (source, "ExpiredTicketIsRejected", Verdict::Holds, None),
        (
            source,
            "AdmittedTicketKeepsItsReceipt",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "RetriedTicketKeepsItsRevision",
            Verdict::Holds,
            None,
        ),
        (
            &no_gate,
            "ExpiredTicketIsRejected",
            Verdict::Violated,
            Some(2),
        ),
        (
            &collect_inclusive,
            "AdmittedTicketKeepsItsReceipt",
            Verdict::Violated,
            Some(2),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn batch_replay_uses_item_receipts_and_reports_completion_only_after_all_items() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/offline-merge/batch-retry.model");
    let duplicate = source.replace(
        "effects = effects ++ item -> Once\n    }",
        "effects = effects ++ item -> Once\n    } else { effects = effects ++ item -> Twice }",
    );
    let omit_missing = source.replace(
        "if { item not in receipts }",
        "if { item not in receipts and (not disconnected or no receipts) }",
    );
    let early_reply = source.replace("if { receipts = Item }", "if { some receipts }");
    for (text, question, expected, depth) in [
        (
            source,
            "AlreadyCommittedItemHasNoNewEffect",
            Verdict::Holds,
            None,
        ),
        (source, "ApplyingMissingItemCommitsIt", Verdict::Holds, None),
        (source, "CompleteRequiresEveryItem", Verdict::Holds, None),
        (source, "ResponseListsCommittedItems", Verdict::Holds, None),
        (
            &duplicate,
            "AlreadyCommittedItemHasNoNewEffect",
            Verdict::Violated,
            Some(3),
        ),
        (
            &omit_missing,
            "ApplyingMissingItemCommitsIt",
            Verdict::Violated,
            Some(5),
        ),
        (
            &early_reply,
            "CompleteRequiresEveryItem",
            Verdict::Violated,
            Some(3),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn http_drain_rejects_late_admission_and_keeps_existing_responses() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/http-drain/drain.model");
    let late = source.replace("if { phase = Accepting }", "if { phase != Halted }");
    let early = source.replace(
        "if { no results.Pending } then { phase = Halted }",
        "phase = Halted
         results = results ++ (results.Pending -> Dropped)",
    );
    for (text, question, expected, depth) in [
        (
            source,
            "AdmissionClosesBeforeShutdown",
            Verdict::Holds,
            None,
        ),
        (source, "StopKeepsInFlightWork", Verdict::Holds, None),
        (source, "HaltedHasNoInFlightWork", Verdict::Holds, None),
        (
            &late,
            "AdmissionClosesBeforeShutdown",
            Verdict::Violated,
            Some(2),
        ),
        (&early, "StopKeepsInFlightWork", Verdict::Violated, Some(3)),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }

    let missing_completion_fairness = source.replace("weak finish(*) weak stop", "weak stop");
    let question = "DrainStopsWithFairCompletion";
    for (text, verdict) in [
        (source, Verdict::Holds),
        (&missing_completion_fairness, Verdict::Violated),
    ] {
        let model = rmodel::parse(text).unwrap();
        let answer = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        assert_eq!(answer.answers[0].verdict, verdict, "{answer:?}");
        if verdict == Verdict::Violated {
            let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
            assert_eq!(witness["complete"], true, "{witness}");
            assert_eq!(witness["verdict"], "violated");
            assert!(witness["trace"]["loopStart"].is_u64());
        }
    }
}

#[test]
fn drain_deadline_preserves_the_first_outcome_and_has_a_fair_shutdown_path() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/http-drain/deadline.model");
    let early = source.replace("due and outcome = Pending", "outcome = Pending");
    let expire_overwrites = source.replace(
        "due and outcome = Pending",
        "due and (outcome = Pending or (outcome = Success and releaseCount = 1))",
    );
    let finish_overwrites = source.replace(
        "if { outcome = Pending }",
        "if { outcome = Pending or (outcome = Aborted and releaseCount = 1) }",
    );
    for (text, question, expected, depth) in [
        (
            source,
            "ExpiryBeforeDeadlineHasNoEffect",
            Verdict::Holds,
            None,
        ),
        (source, "FirstOutcomeIsFinal", Verdict::Holds, None),
        (source, "SettledWorkReleasesOnce", Verdict::Holds, None),
        (source, "HaltedHasNoPendingWork", Verdict::Holds, None),
        (
            &early,
            "ExpiryBeforeDeadlineHasNoEffect",
            Verdict::Violated,
            Some(3),
        ),
        (
            &expire_overwrites,
            "FirstOutcomeIsFinal",
            Verdict::Violated,
            Some(5),
        ),
        (
            &finish_overwrites,
            "FirstOutcomeIsFinal",
            Verdict::Violated,
            Some(5),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }

    let stopped_clock = source.replace(
        "weak reachDeadline weak expire weak halt",
        "weak expire weak halt",
    );
    let question = "DeadlineEventuallyStopsDraining";
    for (text, verdict) in [
        (source, Verdict::Holds),
        (&stopped_clock, Verdict::Violated),
    ] {
        let model = rmodel::parse(text).unwrap();
        let answer = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        assert_eq!(answer.answers[0].verdict, verdict, "{answer:?}");
        if verdict == Verdict::Violated {
            let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
            assert_eq!(witness["complete"], true, "{witness}");
            assert_eq!(witness["verdict"], "violated");
            assert!(witness["trace"]["loopStart"].is_u64());
        }
    }
}

#[test]
fn stream_resume_uses_commits_and_does_not_apply_the_boundary_twice() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/stream-terminal/resume.model");
    let resume_from_sent = source.replace("online = true sent = cursor", "online = true");
    let duplicate = source
        .replace(
            "inbox = cursor + 1",
            "inbox = cursor + 1 or (inbox = cursor and applied = cursor)",
        )
        .replace("Int = 0..3", "Int = 0..4");
    for (text, question, expected, depth) in [
        (
            source,
            "ReconnectUsesCommittedPosition",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "BoundaryReplayHasNoSecondEffect",
            Verdict::Holds,
            None,
        ),
        (source, "EachPositionAppliedOnce", Verdict::Holds, None),
        (
            &resume_from_sent,
            "ReconnectUsesCommittedPosition",
            Verdict::Violated,
            Some(4),
        ),
        (
            &duplicate,
            "BoundaryReplayHasNoSecondEffect",
            Verdict::Violated,
            Some(7),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn event_source_header_and_application_commit_are_independent() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/stream-terminal/event-source.model");
    let trust_header = source.replace("sent = cursor", "sent = browserId");
    for (text, question, expected, depth) in [
        (source, "HeaderReportsBrowserPosition", Verdict::Holds, None),
        (source, "ResumeUsesApplicationCommit", Verdict::Holds, None),
        (
            source,
            "NextEventContinuesTheCommitPrefix",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "ApplicationCommitsContiguousPrefix",
            Verdict::Holds,
            None,
        ),
        (
            &trust_header,
            "ResumeUsesApplicationCommit",
            Verdict::Violated,
            Some(4),
        ),
        (
            &trust_header,
            "NextEventContinuesTheCommitPrefix",
            Verdict::Violated,
            Some(5),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn multi_stream_resume_keeps_subscription_cursors_independent() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/stream-terminal/multi-stream.model");
    let shared_offset = source
        .replace(
            "var online: set Client",
            "var online: set Client\nvar opened: set Client",
        )
        .replace("no online no inbox", "no online no opened no inbox")
        .replace(
            "online = online + client sent = sent ++ client -> client.cursors",
            "online = online + client opened = opened + client
             sent = sent ++ client -> (if { client in opened } then
               (if { Fast.cursors > Slow.cursors } then Fast.cursors else Slow.cursors)
               else client.cursors)",
        );
    let update_all = source.replace(
        "cursors = cursors ++ client -> client.inbox",
        "cursors = Client -> client.inbox",
    );
    for (text, question, expected, depth) in [
        (source, "ConnectionUsesItsOwnCursor", Verdict::Holds, None),
        (
            source,
            "ClientCommitsOnlyItsOwnPosition",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "NextDeliveryFollowsItsOwnCursor",
            Verdict::Holds,
            None,
        ),
        (source, "EveryClientHasItsOwnPrefix", Verdict::Holds, None),
        (
            &shared_offset,
            "ConnectionUsesItsOwnCursor",
            Verdict::Violated,
            Some(6),
        ),
        (
            &shared_offset,
            "NextDeliveryFollowsItsOwnCursor",
            Verdict::Violated,
            Some(7),
        ),
        (
            &update_all,
            "ClientCommitsOnlyItsOwnPosition",
            Verdict::Violated,
            Some(3),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn subscription_retention_requires_valid_cursors_and_reports_expired_gaps() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/stream-terminal/retention.model");
    let use_max = source.replace(
        "all client: Client | client in protected implies client.cursors >= through",
        "no protected or (some client: Client | client in protected and client.cursors >= through)",
    );
    let hide_gap = source.replace("if { client.cursors < floor }", "if { false }");
    for (text, question, expected, depth) in [
        (
            source,
            "ProtectedClientsKeepTheirPrefix",
            Verdict::Holds,
            None,
        ),
        (source, "GapRequiresSnapshot", Verdict::Holds, None),
        (source, "ExactFloorCanResume", Verdict::Holds, None),
        (source, "SnapshotRepinsAtHead", Verdict::Holds, None),
        (
            &use_max,
            "ProtectedClientsKeepTheirPrefix",
            Verdict::Violated,
            Some(2),
        ),
        (&hide_gap, "GapRequiresSnapshot", Verdict::Violated, Some(5)),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn filtered_stream_progress_crosses_only_safe_gaps() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/stream-terminal/filter-progress.model");
    let ignore = source.replace(
        "cursor = if { some pending } then pending - 1 else scanned",
        "cursor = cursor",
    );
    let leap = source.replace(
        "cursor = if { some pending } then pending - 1 else scanned",
        "cursor = scanned",
    );
    for (text, question, expected, depth) in [
        (
            source,
            "ProgressAdvancesOverScannedGap",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "CheckpointNeverCoversUnappliedMatch",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "CompactionNeverPassesCheckpoint",
            Verdict::Holds,
            None,
        ),
        (
            &ignore,
            "ProgressAdvancesOverScannedGap",
            Verdict::Violated,
            Some(3),
        ),
        (
            &leap,
            "CheckpointNeverCoversUnappliedMatch",
            Verdict::Violated,
            Some(4),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn shared_fetch_preserves_other_waiters_and_aborts_after_the_last() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/cancellation/shared-fetch.model");
    let abort_first = source.replace(
        "results = results ++ caller -> Cancelled
      waiting = waiting - caller
      if { waiting = caller } then { phase = Aborted }",
        "results = results ++ (caller -> Cancelled) ++ ((waiting - caller) -> Failed)
         waiting = none
         phase = Aborted",
    );
    let never_abort = source.replace("if { waiting = caller } then { phase = Aborted }", "");
    for (text, question, expected, depth) in [
        (source, "CancellingOneKeepsSharedWork", Verdict::Holds, None),
        (source, "LastWaiterAbortsTransport", Verdict::Holds, None),
        (source, "TerminalCallerResultStays", Verdict::Holds, None),
        (source, "CompletionSettlesTheWaiters", Verdict::Holds, None),
        (source, "RunningHasWaiters", Verdict::Holds, None),
        (
            &abort_first,
            "CancellingOneKeepsSharedWork",
            Verdict::Violated,
            Some(3),
        ),
        (
            &never_abort,
            "LastWaiterAbortsTransport",
            Verdict::Violated,
            Some(2),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn shared_fetch_cleanup_releases_only_the_generation_that_owns_the_key() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/cancellation/shared-fetch-generation.model");
    let delete_by_key = source.replace("if { registered = generation }", "if { true }");
    let never_delete = source.replace("if { registered = generation }", "if { false }");
    for (text, question, expected, depth) in [
        (
            source,
            "StaleCleanupKeepsTheRegistration",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "OwnerCanReleaseTheRegistration",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "AttachmentKeepsTheSameRequest",
            Verdict::Holds,
            None,
        ),
        (
            &delete_by_key,
            "StaleCleanupKeepsTheRegistration",
            Verdict::Violated,
            Some(4),
        ),
        (
            &never_delete,
            "OwnerCanReleaseTheRegistration",
            Verdict::Violated,
            Some(3),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn task_group_waits_for_every_cleanup_and_needs_fair_cleanup_to_return() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/cancellation/task-group.model");
    let early = source.replace(
        "if { stages.Exited + child = Child }",
        "if { stages.Exited + child = Child or child = failedBy }",
    );
    let no_cancel = source
        .replace("signalled = stages.Running - child", "signalled = none")
        .replace("(stages.Running -> Cleaning)", "child -> Cleaning");
    let stalled = source.replace("with { weak exitCleanup(*) }", "with {}");
    let explicit = Selection::Named("explicit".into());
    for (text, question, expected, depth) in [
        (source, "RunningSiblingsGetCancelled", Verdict::Holds, None),
        (source, "ParentWaitsForAllCleanups", Verdict::Holds, None),
        (source, "ParentReportsRecordedFailure", Verdict::Holds, None),
        (
            &early,
            "ParentWaitsForAllCleanups",
            Verdict::Violated,
            Some(2),
        ),
        (
            &no_cancel,
            "RunningSiblingsGetCancelled",
            Verdict::Violated,
            Some(1),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
    let question = "FailedGroupEventuallyReturns";
    for (text, expected) in [(source, Verdict::Holds), (&stalled, Verdict::Violated)] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        assert_eq!(report.answers[0].verdict, expected, "{report:?}");
    }
}

#[test]
fn a_task_group_does_not_overwrite_the_first_terminal_cause() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/cancellation/task-group-cause.model");
    let parent_overwrites = source.replace(
        "cause = if { no cause } then ParentCancelled else cause",
        "cause = ParentCancelled",
    );
    let child_overwrites = source.replace(
        "cause = if { no cause } then child.failures else cause",
        "cause = child.failures",
    );
    for (text, question, expected, depth) in [
        (source, "FirstCauseDoesNotChange", Verdict::Holds, None),
        (
            source,
            "ParentReturnsFirstCauseAfterCleanup",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "EveryChildExitedMeansParentReturned",
            Verdict::Holds,
            None,
        ),
        (
            &parent_overwrites,
            "FirstCauseDoesNotChange",
            Verdict::Violated,
            Some(2),
        ),
        (
            &child_overwrites,
            "FirstCauseDoesNotChange",
            Verdict::Violated,
            Some(2),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn task_group_preserves_root_and_cleanup_failures() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/cancellation/task-group-cleanup.model");
    let drop_cleanup = source.replace(
        "publishedErrors = if { outcome = Broken } then cleanupErrors + child.cleanupFailure else cleanupErrors",
        "publishedErrors = none",
    );
    let replace_primary = source.replacen(
        "publishedPrimary = primary",
        "publishedPrimary = if { outcome = Broken } then child.cleanupFailure else primary",
        1,
    );
    for (text, question, expected, depth) in [
        (
            source,
            "ReportsOnlyAfterEveryChildExits",
            Verdict::Holds,
            None,
        ),
        (source, "PreservesOriginalFailure", Verdict::Holds, None),
        (source, "IncludesEveryCleanupError", Verdict::Holds, None),
        (
            source,
            "CleanupFailureRejectsTheGroup",
            Verdict::Holds,
            None,
        ),
        (
            &drop_cleanup,
            "IncludesEveryCleanupError",
            Verdict::Violated,
            Some(3),
        ),
        (
            &replace_primary,
            "PreservesOriginalFailure",
            Verdict::Violated,
            Some(3),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            question,
            &Selection::Named("explicit".into()),
            Budget::default(),
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn task_group_freezes_admission_but_waits_for_children_already_added() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/cancellation/task-group-admission.model");
    let allow_late = source.replace("if { phase = Open }", "if { true }");
    let snapshot_early = source.replace(
        "phase = Closing and running = child",
        "phase = Closing and (running = child or child = Initial)",
    );
    let explicit = Selection::Named("explicit".into());
    for (text, question, expected, depth) in [
        (source, "LateOfferIsRejected", Verdict::Holds, None),
        (source, "OpenOfferStartsItsChild", Verdict::Holds, None),
        (source, "JoinedHasNoActiveChildren", Verdict::Holds, None),
        (
            &allow_late,
            "LateOfferIsRejected",
            Verdict::Violated,
            Some(2),
        ),
        (
            &snapshot_early,
            "JoinedHasNoActiveChildren",
            Verdict::Violated,
            Some(3),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
    let stalled = source.replace("with { weak finish(*) }", "with {}");
    for (text, expected) in [(source, Verdict::Holds), (&stalled, Verdict::Violated)] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            "ClosedGroupEventuallyJoins",
            &explicit,
            Budget::default(),
        )
        .unwrap();
        assert_eq!(report.answers[0].verdict, expected, "{report:?}");
    }
}

#[test]
fn worker_pool_stop_waits_for_running_work_and_invalidates_reserved_dispatch() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/cancellation/worker-pool-stop.model");
    let forget_queue = source.replace(
        "(jobs.Queued + jobs.Reserved) -> Cancelled",
        "jobs.Reserved -> Cancelled",
    );
    let stale_dispatch = source.replace(
        "if { phase = Open and Waiting.jobs = Reserved }",
        "if { true }",
    );
    let join_early = source.replace("if { no jobs.Running }", "if { true }");
    let explicit = Selection::Named("explicit".into());
    for (text, question, expected, depth) in [
        (source, "StopCancelsUnstartedJobs", Verdict::Holds, None),
        (source, "DispatchCannotStartAfterStop", Verdict::Holds, None),
        (source, "HaltedHasNoUnfinishedWork", Verdict::Holds, None),
        (
            &forget_queue,
            "StopCancelsUnstartedJobs",
            Verdict::Violated,
            Some(1),
        ),
        (
            &stale_dispatch,
            "DispatchCannotStartAfterStop",
            Verdict::Violated,
            Some(4),
        ),
        (
            &join_early,
            "HaltedHasNoUnfinishedWork",
            Verdict::Violated,
            Some(2),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
    let stalled = source.replace("with { weak finish(*) weak join }", "with { weak join }");
    for (text, expected) in [(source, Verdict::Holds), (&stalled, Verdict::Violated)] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            "StoppingEventuallyHalts",
            &explicit,
            Budget::default(),
        )
        .unwrap();
        assert_eq!(report.answers[0].verdict, expected, "{report:?}");
    }
}

#[test]
fn worker_exit_reports_failure_and_releases_the_slot() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/cancellation/worker-pool-crash.model");
    let success = source.replace(
        "jobs = jobs ++ Active -> Failed",
        "jobs = jobs ++ Active -> Done",
    );
    let leak = source.replace("live = none\n    slot = none", "live = none");
    let explicit = Selection::Named("explicit".into());
    for (text, question, expected, depth) in [
        (source, "CrashReportsFailure", Verdict::Holds, None),
        (source, "CrashFreesTheSlot", Verdict::Holds, None),
        (source, "RunningJobOwnsTheSlot", Verdict::Holds, None),
        (source, "NoWorkerMeansNoAssignedJob", Verdict::Holds, None),
        (&success, "CrashReportsFailure", Verdict::Violated, Some(1)),
        (&leak, "CrashFreesTheSlot", Verdict::Violated, Some(1)),
    ] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &report.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&expected, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(expected).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
    let stalled = source.replace(
        "with { weak replace weak startWaiting weak finish }",
        "with { weak startWaiting weak finish }",
    );
    for (text, expected) in [(source, Verdict::Holds), (&stalled, Verdict::Violated)] {
        let model = rmodel::parse(text).unwrap();
        let report = rcheck::check(
            &model,
            "QueuedWorkSurvivesOriginalCrash",
            &explicit,
            Budget::default(),
        )
        .unwrap();
        assert_eq!(report.answers[0].verdict, expected, "{report:?}");
    }
}

#[test]
fn worker_retry_distinguishes_unknown_outcome_from_durable_commit() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/cancellation/worker-retry.model");
    let duplicate = source.replace("if { no receipt } then {", "if { true } then {");
    let split = source.replace(
        "if { point = AfterCommit } then {\n      effects = 1",
        "effects = 1\n    if { point = AfterCommit } then {",
    );
    let failed = source.replace("outcome = Unknown\n    if", "outcome = Failed\n    if");
    assert!(
        [&duplicate, &split, &failed]
            .iter()
            .all(|text| *text != source)
    );
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (source, "EffectAndReceiptAreAtomic", Verdict::Holds, None),
        (source, "ReplyConfirmsReceipt", Verdict::Holds, None),
        (source, "ExitLeavesOutcomeUnknown", Verdict::Holds, None),
        (source, "RetryPreservesCommittedWork", Verdict::Holds, None),
        (source, "LostBeforeCommit", Verdict::Reachable, Some(1)),
        (source, "LostAfterCommit", Verdict::Reachable, Some(1)),
        (
            &duplicate,
            "RetryPreservesCommittedWork",
            Verdict::Violated,
            Some(2),
        ),
        (
            &split,
            "EffectAndReceiptAreAtomic",
            Verdict::Violated,
            Some(1),
        ),
        (
            &failed,
            "ExitLeavesOutcomeUnknown",
            Verdict::Violated,
            Some(1),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }

    let stalled = source.replace("with { weak retry } ", "");
    for (text, expected) in [(source, Verdict::Holds), (&stalled, Verdict::Violated)] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(
            &model,
            "UnknownEventuallyReplies",
            &explicit,
            Budget::default(),
        )
        .unwrap();
        assert_eq!(result.answers[0].verdict, expected, "{result:?}");
    }
}

#[test]
fn worker_fence_separates_stale_writes_from_stale_replies() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/cancellation/worker-fence.model");
    let old_write = source
        .replace(
            "if { attempt = current } then { ready",
            "if { true } then { ready",
        )
        .replace("attempt = current and no receipt", "no receipt");
    let old_reply = source.replace(
        "if { attempt = current } then {\n      replyFrom",
        "if { true } then {\n      replyFrom",
    );
    assert!([&old_write, &old_reply].iter().all(|text| *text != source));
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (source, "ReceiptMatchesOneEffect", Verdict::Holds, None),
        (
            source,
            "AcceptedReplyComesFromCurrent",
            Verdict::Holds,
            None,
        ),
        (source, "AcceptedReplyCarriesReceipt", Verdict::Holds, None),
        (source, "RejectOldWrite", Verdict::Holds, None),
        (source, "ReuseCommittedReceipt", Verdict::Holds, None),
        (source, "RejectOldReply", Verdict::Holds, None),
        (
            source,
            "ReplacementBeforeAnyCommit",
            Verdict::Reachable,
            Some(1),
        ),
        (
            source,
            "OldCommitAwaitingReply",
            Verdict::Reachable,
            Some(2),
        ),
        (&old_write, "RejectOldWrite", Verdict::Violated, Some(2)),
        (&old_write, "RejectOldReply", Verdict::Holds, None),
        (&old_reply, "RejectOldWrite", Verdict::Holds, None),
        (&old_reply, "RejectOldReply", Verdict::Violated, Some(3)),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn only_one_worker_can_claim_the_last_task() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/worker-deque/last-task.model");
    let unchecked = source.replace("some (actor.candidate & queued)", "some actor.candidate");
    assert_ne!(unchecked, source);
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (source, "BothReadLastTask", Verdict::Reachable, Some(4)),
        (source, "ReadOppositeEnds", Verdict::Reachable, Some(2)),
        (source, "OneClaimantPerTask", Verdict::Holds, None),
        (source, "EveryTaskQueuedOrClaimed", Verdict::Holds, None),
        (source, "SnapshotPrefersOwnEnd", Verdict::Holds, None),
        (
            source,
            "ClaimOfRemovedTaskLeavesResultsUnchanged",
            Verdict::Holds,
            None,
        ),
        (&unchecked, "OneClaimantPerTask", Verdict::Violated, Some(6)),
        (
            &unchecked,
            "ClaimOfRemovedTaskLeavesResultsUnchanged",
            Verdict::Violated,
            Some(6),
        ),
        (&unchecked, "EveryTaskQueuedOrClaimed", Verdict::Holds, None),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn reusing_a_ring_slot_does_not_reuse_the_readers_ticket() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/worker-deque/ring-reuse.model");
    let slot_only = source.replace(
        "if { reader.observed = reader.observed.location.cells }",
        "if { some reader.observed.location.cells }",
    );
    assert_ne!(slot_only, source);
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (
            source,
            "WrappedWithOldCandidate",
            Verdict::Reachable,
            Some(4),
        ),
        (source, "TicketStaysAtItsSlot", Verdict::Holds, None),
        (
            source,
            "PublishedTicketIsQueuedOrClaimed",
            Verdict::Holds,
            None,
        ),
        (source, "OneClaimantPerTicket", Verdict::Holds, None),
        (
            source,
            "StaleCandidateCannotConsumeReplacement",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "FreshCandidateClaimsItsTicket",
            Verdict::Holds,
            None,
        ),
        (source, "ClaimLeavesOtherSlotAlone", Verdict::Holds, None),
        (
            &slot_only,
            "StaleCandidateCannotConsumeReplacement",
            Verdict::Violated,
            Some(5),
        ),
        (
            &slot_only,
            "PublishedTicketIsQueuedOrClaimed",
            Verdict::Holds,
            None,
        ),
        (&slot_only, "OneClaimantPerTicket", Verdict::Holds, None),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn ring_publication_requires_every_body_word_for_that_ticket() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/worker-deque/ring-publication.model");
    let on_reserve = source.replace(
        "update { reserved = true }",
        "update { reserved = true visible = Next }",
    );
    let after_header = source.replace(
        "update { body = body ++ field -> Next }",
        "update {\n    body = body ++ field -> Next\n    if { field = Header } then { visible = Next }\n  }",
    );
    assert!(
        [&on_reserve, &after_header]
            .iter()
            .all(|text| *text != source)
    );
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (
            source,
            "HeaderStagedWithoutPublication",
            Verdict::Reachable,
            Some(2),
        ),
        (source, "PublishedUnread", Verdict::Reachable, Some(4)),
        (source, "VisibleTicketHasCompleteBody", Verdict::Holds, None),
        (source, "ReaderGetsBodyForTicket", Verdict::Holds, None),
        (source, "BodyIsPreservedOnPublish", Verdict::Holds, None),
        (source, "EmptySlotCannotBeRead", Verdict::Holds, None),
        (
            &on_reserve,
            "VisibleTicketHasCompleteBody",
            Verdict::Violated,
            Some(1),
        ),
        (
            &on_reserve,
            "ReaderGetsBodyForTicket",
            Verdict::Violated,
            Some(2),
        ),
        (
            &after_header,
            "VisibleTicketHasCompleteBody",
            Verdict::Violated,
            Some(2),
        ),
        (
            &after_header,
            "ReaderGetsBodyForTicket",
            Verdict::Violated,
            Some(3),
        ),
        (
            &after_header,
            "BodyIsPreservedOnPublish",
            Verdict::Holds,
            None,
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn ring_reader_keeps_ownership_until_copy_is_complete() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/worker-deque/ring-copy.model");
    let early = source.replace(
        "update { copy = copy + (field -> field.body) }",
        "update {\n    copy = copy + (field -> field.body)\n    if { field = Header } then { slot = Free }\n  }",
    );
    assert_ne!(early, source);
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (source, "PausedAfterHeader", Verdict::Reachable, Some(2)),
        (source, "ReusedAfterReturning", Verdict::Reachable, Some(5)),
        (source, "ReaderOwnsSlotWhileCopying", Verdict::Holds, None),
        (source, "CopiedWordsBelongToOldTicket", Verdict::Holds, None),
        (source, "ReturnedCopyIsComplete", Verdict::Holds, None),
        (source, "ProducerDoesNotTouchHeldSlot", Verdict::Holds, None),
        (
            &early,
            "ReaderOwnsSlotWhileCopying",
            Verdict::Violated,
            Some(2),
        ),
        (
            &early,
            "CopiedWordsBelongToOldTicket",
            Verdict::Violated,
            Some(4),
        ),
        (&early, "ReturnedCopyIsComplete", Verdict::Violated, Some(5)),
        (&early, "ProducerDoesNotTouchHeldSlot", Verdict::Holds, None),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn ring_reclaim_requires_stop_and_late_cleanup_checks_ownership() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/worker-deque/ring-reclaim.model");
    let early = source.replace(
        "update { status = TimedOut }",
        "update { status = TimedOut owner = none }",
    );
    let untagged = source.replace(
        "update { if { owner = Original } then { owner = none } }",
        "update { owner = none }",
    );
    assert!([&early, &untagged].iter().all(|text| *text != source));
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (source, "TimedOutButStillOwned", Verdict::Reachable, Some(1)),
        (source, "StoppedButStillOwned", Verdict::Reachable, Some(2)),
        (source, "LiveReaderKeepsSlot", Verdict::Holds, None),
        (
            source,
            "OriginalReaderOnlyCopiesOriginal",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "LateReleaseCannotClearReplacement",
            Verdict::Holds,
            None,
        ),
        (&early, "LiveReaderKeepsSlot", Verdict::Violated, Some(1)),
        (
            &early,
            "OriginalReaderOnlyCopiesOriginal",
            Verdict::Violated,
            Some(3),
        ),
        (
            &untagged,
            "LateReleaseCannotClearReplacement",
            Verdict::Violated,
            Some(5),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }

    let stalled = source.replace(" with { weak reclaim }", "");
    for (text, expected) in [(source, Verdict::Holds), (&stalled, Verdict::Violated)] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(
            &model,
            "StoppedReaderEventuallyReclaimed",
            &explicit,
            Budget::default(),
        )
        .unwrap();
        assert_eq!(result.answers[0].verdict, expected, "{result:?}");
    }
}

#[test]
fn ring_reclaim_deducts_exactly_the_stopped_readers_reference() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/worker-deque/ring-shared-readers.model");
    let release = "if { reader in pins } then { pins = pins - reader refs = refs.lower }";
    let clear_all = source.replacen(
        release,
        "if { reader in pins } then { pins = none refs = Zero }",
        1,
    );
    let double_debit = source.replacen(
        release,
        &format!("{release} else {{ if {{ refs = One }} then {{ refs = Zero }} }}"),
        1,
    );
    assert!(
        [&clear_all, &double_debit]
            .iter()
            .all(|text| *text != source)
    );
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (
            source,
            "FirstStoppedWithBothPins",
            Verdict::Reachable,
            Some(1),
        ),
        (
            source,
            "SecondReadsAfterFirstReclaimed",
            Verdict::Reachable,
            Some(3),
        ),
        (
            source,
            "ReusedAfterLastReference",
            Verdict::Reachable,
            Some(5),
        ),
        (source, "RefCountMatchesPins", Verdict::Holds, None),
        (source, "LiveReadersKeepTheirPins", Verdict::Holds, None),
        (source, "ReadersCopyOnlyOriginal", Verdict::Holds, None),
        (source, "RepeatedReclaimIsNoOp", Verdict::Holds, None),
        (source, "ReuseRequiresNoPins", Verdict::Holds, None),
        (
            &clear_all,
            "LiveReadersKeepTheirPins",
            Verdict::Violated,
            Some(2),
        ),
        (&clear_all, "RefCountMatchesPins", Verdict::Holds, None),
        (
            &clear_all,
            "ReadersCopyOnlyOriginal",
            Verdict::Violated,
            Some(4),
        ),
        (
            &double_debit,
            "RefCountMatchesPins",
            Verdict::Violated,
            Some(3),
        ),
        (
            &double_debit,
            "RepeatedReclaimIsNoOp",
            Verdict::Violated,
            Some(3),
        ),
        (
            &double_debit,
            "ReuseRequiresNoPins",
            Verdict::Violated,
            Some(4),
        ),
        (
            &double_debit,
            "ReadersCopyOnlyOriginal",
            Verdict::Violated,
            Some(5),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn ring_registration_checks_both_admission_and_generation() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/worker-deque/ring-admission.model");
    let allow_retired = source.replace("accepting and seen = current", "seen = current");
    let ignore_generation = source.replace("accepting and seen = current", "accepting");
    assert!(
        [&allow_retired, &ignore_generation]
            .iter()
            .all(|text| *text != source)
    );
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (
            source,
            "ObservedBeforeRetirement",
            Verdict::Reachable,
            Some(2),
        ),
        (source, "OldViewAfterReuse", Verdict::Reachable, Some(3)),
        (source, "PinSurvivesRetirement", Verdict::Reachable, Some(3)),
        (source, "NextPinAfterFreshView", Verdict::Reachable, Some(4)),
        (source, "PinMatchesObservedGeneration", Verdict::Holds, None),
        (
            source,
            "ClosedGenerationRejectsNewPins",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "OldObservationCannotPinReplacement",
            Verdict::Holds,
            None,
        ),
        (source, "RetirementKeepsExistingPin", Verdict::Holds, None),
        (source, "ExistingPinPreventsReuse", Verdict::Holds, None),
        (
            &allow_retired,
            "ClosedGenerationRejectsNewPins",
            Verdict::Violated,
            Some(3),
        ),
        (
            &allow_retired,
            "OldObservationCannotPinReplacement",
            Verdict::Holds,
            None,
        ),
        (
            &ignore_generation,
            "ClosedGenerationRejectsNewPins",
            Verdict::Holds,
            None,
        ),
        (
            &ignore_generation,
            "OldObservationCannotPinReplacement",
            Verdict::Violated,
            Some(4),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn wasm_view_refresh_and_payload_address_are_independent() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/wasm-memory/view-after-grow.model");
    let update = "update { view = epoch used = Payload value = Expected }";
    let detached = source.replace(
        update,
        "update { used = Payload value = if { view = epoch } then Expected else none }",
    );
    let offset = source.replace(
        update,
        "update {\n    view = epoch\n    if { view = epoch } then { used = Payload value = Expected }\n    else { used = Adjacent value = Neighbor }\n  }",
    );
    assert!([&detached, &offset].iter().all(|text| *text != source));
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (
            source,
            "DetachedWithoutMorePages",
            Verdict::Reachable,
            Some(2),
        ),
        (source, "DetachedWithMorePages", Verdict::Reachable, Some(2)),
        (source, "ReadUsesLiveBuffer", Verdict::Holds, None),
        (source, "ReadUsesPayloadAddress", Verdict::Holds, None),
        (source, "ReadReturnsExpectedByte", Verdict::Holds, None),
        (
            source,
            "GrowthDoesNotRefreshCachedView",
            Verdict::Holds,
            None,
        ),
        (&detached, "ReadUsesLiveBuffer", Verdict::Violated, Some(3)),
        (&detached, "ReadUsesPayloadAddress", Verdict::Holds, None),
        (&offset, "ReadUsesLiveBuffer", Verdict::Holds, None),
        (
            &offset,
            "ReadUsesPayloadAddress",
            Verdict::Violated,
            Some(3),
        ),
        (
            &offset,
            "ReadReturnsExpectedByte",
            Verdict::Violated,
            Some(3),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn shared_wasm_views_keep_prefix_aliasing_but_need_more_extent_for_suffix() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/wasm-memory/shared-view-after-grow.model");
    let cached_copy = source.replace(
        "then { value = prefix }",
        "then { value = if { view = FirstPage } then Original else prefix }",
    );
    let short_view = source.replace(
        "view = pages\n      value = suffix",
        "value = if { view = BothPages } then suffix else none",
    );
    assert!(
        [&cached_copy, &short_view]
            .iter()
            .all(|text| *text != source)
    );
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (source, "OldViewAfterGrowth", Verdict::Reachable, Some(2)),
        (source, "ViewDoesNotExceedMemory", Verdict::Holds, None),
        (source, "GrowthKeepsFixedView", Verdict::Holds, None),
        (source, "PrefixReadSeesSharedWrites", Verdict::Holds, None),
        (source, "SuffixReadRefreshesView", Verdict::Holds, None),
        (source, "MissingPageReturnsNoByte", Verdict::Holds, None),
        (
            &cached_copy,
            "PrefixReadSeesSharedWrites",
            Verdict::Violated,
            Some(3),
        ),
        (
            &cached_copy,
            "SuffixReadRefreshesView",
            Verdict::Holds,
            None,
        ),
        (
            &short_view,
            "PrefixReadSeesSharedWrites",
            Verdict::Holds,
            None,
        ),
        (
            &short_view,
            "SuffixReadRefreshesView",
            Verdict::Violated,
            Some(3),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn wasm_saved_output_survives_dispatch_storage_reuse() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/wasm-memory/output-lifetime.model");
    let borrowed = source.replace(
        "update { current = output }",
        "update { current = output if { some captured } then { retained = output } }",
    );
    assert_ne!(borrowed, source);
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (
            source,
            "RetainedFirstThenDispatchedSecond",
            Verdict::Reachable,
            Some(3),
        ),
        (source, "SavedOutputMatchesCapture", Verdict::Holds, None),
        (source, "RetainCapturesCurrentOutput", Verdict::Holds, None),
        (source, "DispatchKeepsSavedOutput", Verdict::Holds, None),
        (
            &borrowed,
            "RetainCapturesCurrentOutput",
            Verdict::Holds,
            None,
        ),
        (
            &borrowed,
            "SavedOutputMatchesCapture",
            Verdict::Violated,
            Some(3),
        ),
        (
            &borrowed,
            "DispatchKeepsSavedOutput",
            Verdict::Violated,
            Some(3),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn failed_wasm_growth_preserves_the_cached_view_and_api_result() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/wasm-memory/grow-failure.model");
    let failure = "result = if { entry = Instruction } then MinusOne else RangeFailure";
    let dropped = source.replace(failure, &format!("{failure}\n      view = none"));
    let collapsed = source.replace(failure, "result = MinusOne");
    assert!([&dropped, &collapsed].iter().all(|text| *text != source));
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (
            source,
            "InstructionFailedWithView",
            Verdict::Reachable,
            Some(2),
        ),
        (source, "HostFailedWithView", Verdict::Reachable, Some(2)),
        (source, "FailedGrowthPreservesMemory", Verdict::Holds, None),
        (
            source,
            "InstructionFailureReturnsMinusOne",
            Verdict::Holds,
            None,
        ),
        (source, "HostFailureReturnsRangeError", Verdict::Holds, None),
        (
            source,
            "SuccessfulGrowthRefreshesBuffer",
            Verdict::Holds,
            None,
        ),
        (source, "ReadUsesCurrentView", Verdict::Holds, None),
        (
            &dropped,
            "FailedGrowthPreservesMemory",
            Verdict::Violated,
            Some(2),
        ),
        (
            &dropped,
            "HostFailureReturnsRangeError",
            Verdict::Holds,
            None,
        ),
        (
            &collapsed,
            "FailedGrowthPreservesMemory",
            Verdict::Holds,
            None,
        ),
        (
            &collapsed,
            "HostFailureReturnsRangeError",
            Verdict::Violated,
            Some(1),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn worker_message_transfer_detaches_on_send_and_copy_captures_on_send() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/worker-transfer/array-buffer.model");
    let detach = "if { mode = Move } then { sender = none }";
    let copy_instead = source.replace(detach, "");
    let move_instead = source.replace(detach, "sender = none");
    let deferred = source.replace(
        "update { sender = SenderEdit }",
        "update { sender = SenderEdit if { sent = Copy and some pending } then { pending = SenderEdit } }",
    );
    assert!(
        [&copy_instead, &move_instead, &deferred]
            .iter()
            .all(|text| *text != source)
    );
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (source, "MovedBeforeDelivery", Verdict::Reachable, Some(1)),
        (source, "CopiedBeforeDelivery", Verdict::Reachable, Some(1)),
        (source, "MoveDetachesSenderOnSend", Verdict::Holds, None),
        (source, "CopyRetainsSenderOnSend", Verdict::Holds, None),
        (
            source,
            "SendCapturesBytesBeforeDelivery",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "SenderEditsDoNotChangePostedBytes",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "ReceiverEditsDoNotChangeSender",
            Verdict::Holds,
            None,
        ),
        (source, "DeliveryReceivesPostedBytes", Verdict::Holds, None),
        (
            &copy_instead,
            "MoveDetachesSenderOnSend",
            Verdict::Violated,
            Some(1),
        ),
        (
            &move_instead,
            "CopyRetainsSenderOnSend",
            Verdict::Violated,
            Some(1),
        ),
        (
            &deferred,
            "SendCapturesBytesBeforeDelivery",
            Verdict::Holds,
            None,
        ),
        (
            &deferred,
            "SenderEditsDoNotChangePostedBytes",
            Verdict::Violated,
            Some(2),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn rejected_buffer_transfer_preserves_views_and_does_not_enqueue() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/worker-transfer/rejection.model");
    let detach = source.replace(
        "result = Rejected\n",
        "result = Rejected held = held - (Left -> Byte)\n",
    );
    let enqueue = source.replace("result = Rejected\n", "result = Rejected pending = held\n");
    assert!([&detach, &enqueue].iter().all(|text| *text != source));
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (source, "RejectedDuplicate", Verdict::Reachable, Some(1)),
        (source, "RejectedUncloneable", Verdict::Reachable, Some(1)),
        (source, "RejectedBlocked", Verdict::Reachable, Some(1)),
        (
            source,
            "DeliveredAfterValidSend",
            Verdict::Reachable,
            Some(2),
        ),
        (source, "RejectionKeepsBothBuffers", Verdict::Holds, None),
        (source, "RejectionDoesNotEnqueue", Verdict::Holds, None),
        (source, "RetryMovesOnlyListedBuffer", Verdict::Holds, None),
        (source, "AcceptedSendQueuesBothValues", Verdict::Holds, None),
        (
            source,
            "PollWithoutSendDoesNotDeliver",
            Verdict::Holds,
            None,
        ),
        (
            &detach,
            "RejectionKeepsBothBuffers",
            Verdict::Violated,
            Some(1),
        ),
        (&detach, "RejectionDoesNotEnqueue", Verdict::Holds, None),
        (&enqueue, "RejectionKeepsBothBuffers", Verdict::Holds, None),
        (
            &enqueue,
            "RejectionDoesNotEnqueue",
            Verdict::Violated,
            Some(1),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn transferring_a_buffer_does_not_confirm_remote_processing() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/worker-transfer/acknowledgment.model");
    let premature = source.replace(
        "pending = true status = Pending",
        "pending = true status = Confirmed",
    );
    let failure = source.replace("then { status = Unknown }", "then { status = Failed }");
    assert!([&premature, &failure].iter().all(|text| *text != source));
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (source, "ClosedBeforeDelivery", Verdict::Reachable, Some(2)),
        (
            source,
            "ClosedAfterDeliveryWithoutAck",
            Verdict::Reachable,
            Some(3),
        ),
        (source, "ConfirmedThenClosed", Verdict::Reachable, Some(4)),
        (source, "ConfirmedNeedsAcknowledgment", Verdict::Holds, None),
        (source, "AckFollowsDelivery", Verdict::Holds, None),
        (source, "ClosedHasNoPendingDelivery", Verdict::Holds, None),
        (source, "SendMovesWithoutConfirming", Verdict::Holds, None),
        (
            source,
            "UnacknowledgedCloseLeavesOutcomeUnknown",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "ClosePreservesFinishedDelivery",
            Verdict::Holds,
            None,
        ),
        (
            &premature,
            "ConfirmedNeedsAcknowledgment",
            Verdict::Violated,
            Some(1),
        ),
        (
            &premature,
            "SendMovesWithoutConfirming",
            Verdict::Violated,
            Some(1),
        ),
        (
            &failure,
            "UnacknowledgedCloseLeavesOutcomeUnknown",
            Verdict::Violated,
            Some(2),
        ),
        (
            &failure,
            "ClosePreservesFinishedDelivery",
            Verdict::Holds,
            None,
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn round_trip_transfer_creates_a_fresh_view_without_restoring_old_handles() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!("../../../examples/model-patterns/worker-transfer/round-trip.model");
    let copied = source.replace(
        "inFlight = workerView workerView = none",
        "inFlight = workerView",
    );
    let rebound = source
        .replace(
            "stage = Returned returned = inFlight",
            "stage = Returned returned = inFlight original = inFlight",
        )
        .replace(
            "update { returned = Final }",
            "update { returned = Final original = Final }",
        );
    assert!([&copied, &rebound].iter().all(|text| *text != source));
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (source, "ReturnedUpdated", Verdict::Reachable, Some(5)),
        (source, "NewHandleWasEdited", Verdict::Reachable, Some(5)),
        (source, "OriginalHandleNeverRevives", Verdict::Holds, None),
        (
            source,
            "WorkerViewExistsOnlyWhileOwned",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "OutboundTransferDetachesOriginal",
            Verdict::Holds,
            None,
        ),
        (source, "ReturnTransferDetachesWorker", Verdict::Holds, None),
        (source, "ReceiveMakesFreshHandle", Verdict::Holds, None),
        (
            source,
            "EditingReturnedBytesDoesNotReviveOldHandles",
            Verdict::Holds,
            None,
        ),
        (
            &copied,
            "ReturnTransferDetachesWorker",
            Verdict::Violated,
            Some(3),
        ),
        (&copied, "OriginalHandleNeverRevives", Verdict::Holds, None),
        (
            &rebound,
            "ReturnTransferDetachesWorker",
            Verdict::Holds,
            None,
        ),
        (
            &rebound,
            "OriginalHandleNeverRevives",
            Verdict::Violated,
            Some(4),
        ),
        (
            &rebound,
            "EditingReturnedBytesDoesNotReviveOldHandles",
            Verdict::Violated,
            Some(5),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn structured_copy_preserves_internal_sharing_without_merging_equal_objects() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/worker-transfer/shared-references.model");
    let split = source.replace(
        "target = (Left -> CopyShared) + (Right -> CopyShared)",
        "target = (Left -> CopyShared) + (Right -> CopyRight)",
    );
    let merge = source.replace("(Twin -> CopyTwin)", "(Twin -> CopyShared)");
    assert!([&split, &merge].iter().all(|text| *text != source));
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (source, "MessageCopied", Verdict::Reachable, Some(1)),
        (source, "ReceiverPreservesAlias", Verdict::Holds, None),
        (source, "EqualTwinsStayDistinct", Verdict::Holds, None),
        (
            source,
            "SenderAndReceiverHaveIndependentNodes",
            Verdict::Holds,
            None,
        ),
        (source, "SendCopiesFieldValues", Verdict::Holds, None),
        (
            source,
            "ReceiverEditKeepsSenderValues",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "ReceiverEditIsVisibleThroughAlias",
            Verdict::Holds,
            None,
        ),
        (source, "ReceiverEditKeepsTwinValue", Verdict::Holds, None),
        (
            source,
            "SenderEditKeepsReceiverValues",
            Verdict::Holds,
            None,
        ),
        (&split, "ReceiverPreservesAlias", Verdict::Violated, Some(1)),
        (&split, "EqualTwinsStayDistinct", Verdict::Holds, None),
        (
            &split,
            "ReceiverEditIsVisibleThroughAlias",
            Verdict::Violated,
            Some(2),
        ),
        (&merge, "ReceiverPreservesAlias", Verdict::Holds, None),
        (&merge, "EqualTwinsStayDistinct", Verdict::Violated, Some(1)),
        (
            &merge,
            "ReceiverEditKeepsTwinValue",
            Verdict::Violated,
            Some(2),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn structured_copy_closes_the_cycle_inside_the_clone() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/worker-transfer/cyclic-references.model");
    let cut = source.replace(" + (CopyB -> CopyA)", "");
    let reused = source.replace("(CopyB -> CopyA)", "(CopyB -> SourceA)");
    assert!([&cut, &reused].iter().all(|text| *text != source));
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (source, "MessageCopied", Verdict::Reachable, Some(1)),
        (source, "CopiedRootEdited", Verdict::Reachable, Some(2)),
        (source, "OriginalCycleIsIntact", Verdict::Holds, None),
        (source, "CopyHasBackEdge", Verdict::Holds, None),
        (source, "CopyCycleReturnsToItsRoot", Verdict::Holds, None),
        (source, "CopyDoesNotPointIntoSource", Verdict::Holds, None),
        (
            source,
            "EditThroughCycleChangesCopiedRoot",
            Verdict::Holds,
            None,
        ),
        (
            source,
            "EditThroughCycleKeepsSourceValues",
            Verdict::Holds,
            None,
        ),
        (&cut, "CopyHasBackEdge", Verdict::Violated, Some(1)),
        (&cut, "CopyDoesNotPointIntoSource", Verdict::Holds, None),
        (
            &cut,
            "EditThroughCycleChangesCopiedRoot",
            Verdict::Violated,
            Some(2),
        ),
        (&reused, "CopyHasBackEdge", Verdict::Holds, None),
        (
            &reused,
            "CopyDoesNotPointIntoSource",
            Verdict::Violated,
            Some(1),
        ),
        (
            &reused,
            "EditThroughCycleKeepsSourceValues",
            Verdict::Violated,
            Some(2),
        ),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn graph_reachability_finds_nodes_beyond_two_hops_and_requires_closure() {
    use rcheck::{Budget, Selection, Verdict};
    let source =
        include_str!("../../../examples/model-patterns/worker-transfer/graph-reachability.model");
    let short = source.replace("if { External in seen }", "if { External in short }");
    let early = source.replace(
        "guard { started no frontier no result }",
        "guard { started no result }",
    );
    assert!([&short, &early].iter().all(|text| *text != source));
    let explicit = Selection::Named("explicit".into());
    for (text, question, verdict, depth) in [
        (source, "FinishedWithoutSource", Verdict::Reachable, Some(6)),
        (source, "FinishedWithSource", Verdict::Reachable, Some(8)),
        (source, "FinishedSetIsClosed", Verdict::Holds, None),
        (
            source,
            "FinishedSetContainsTwoHopResult",
            Verdict::Holds,
            None,
        ),
        (source, "IsolatedNodeStaysOut", Verdict::Holds, None),
        (source, "FrontierOnlyContainsNewNodes", Verdict::Holds, None),
        (source, "FinishClassifiesExternalNode", Verdict::Holds, None),
        (&short, "FinishedSetIsClosed", Verdict::Holds, None),
        (
            &short,
            "FinishClassifiesExternalNode",
            Verdict::Violated,
            Some(8),
        ),
        (&early, "FinishedSetIsClosed", Verdict::Violated, Some(2)),
    ] {
        let model = rmodel::parse(text).unwrap();
        let result = rcheck::check(&model, question, &explicit, Budget::default()).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (&answer.verdict, answer.depth),
            (&verdict, depth),
            "{answer:?}"
        );
        let witness = rcheck::conform::witness(&model, question, 10_000).unwrap();
        assert_eq!(witness["complete"], true, "{question}: {witness}");
        assert_eq!(witness["verdict"], serde_json::to_value(verdict).unwrap());
        if let Some(depth) = depth {
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                depth
            );
        }
    }
}

#[test]
fn reachability_expression_agrees_with_the_walk_and_round_trips_through_ir() {
    use rcheck::{Budget, Selection, Verdict};
    let source = include_str!(
        "../../../examples/model-patterns/worker-transfer/reachability-expression.model"
    );
    let walk =
        include_str!("../../../examples/model-patterns/worker-transfer/graph-reachability.model");
    let model = rmodel::parse(source).unwrap();
    let original = rcheck::ir::to_ir(&model).unwrap();
    let serialized = serde_json::to_string(&original).unwrap();
    let reparsed = rcheck::ir::parse(&serialized).unwrap().0;
    assert_eq!(rcheck::ir::to_ir(&reparsed).unwrap(), original);
    let walk = rmodel::parse(walk).unwrap();
    for (question, walk_steps, expression_steps) in [
        ("FinishedWithoutSource", 6, 1),
        ("FinishedWithSource", 8, 2),
    ] {
        for (input, steps) in [(&walk, walk_steps), (&reparsed, expression_steps)] {
            let witness = rcheck::conform::witness(input, question, 10_000).unwrap();
            assert_eq!(witness["complete"], true);
            assert_eq!(witness["verdict"], "reachable");
            assert_eq!(
                witness["trace"]["commands"].as_array().unwrap().len(),
                steps
            );
        }
    }
    for (checker, question, expected) in [
        ("explicit", "AttachAddsTheExternalNode", Verdict::Holds),
        ("explicit", "EmptyRootHasNoReachableNodes", Verdict::Holds),
        ("sat", "FinishedWithSource", Verdict::Reachable),
        ("sat", "EmptyRootHasNoReachableNodes", Verdict::Holds),
    ] {
        let result = rcheck::check(
            &reparsed,
            question,
            &Selection::Named(checker.into()),
            Budget::default(),
        )
        .unwrap();
        assert_eq!(result.answers[0].verdict, expected, "{result:?}");
    }
}
#[test]
fn observation_pair_returns_independent_witnesses_for_a_single_model() {
    use serde_json::{Value, json};
    use std::process::{Command, Stdio};
    let model = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/model-patterns/object-store/multipart-reconcile.model"
    );
    let query = |at: &str, observe: &[&str], limit: &str| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rcheck"))
            .args(["observation-pair", model, "-", "--max-states", limit])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        serde_json::to_writer(
            child.stdin.take().unwrap(),
            &json!({"at": at, "observe": observe, "distinguish": ["versions"]}),
        )
        .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let observation = ["observed", "result", "proof"];
    let report = query("LatestObservation", &observation, "1000");
    assert_eq!(report["verdict"], "reachable");
    assert_eq!(report["complete"], true);
    let pair = report["pair"].as_array().unwrap();
    assert_eq!(pair.len(), 2);
    for trace in pair {
        assert_eq!(trace["commands"].as_array().unwrap().len(), 3);
        assert_eq!(trace["finalState"]["observed"], "Other");
        assert_eq!(trace["finalState"]["result"], "Unknown");
    }
    assert_ne!(
        pair[0]["finalState"]["versions"],
        pair[1]["finalState"]["versions"]
    );
    assert_eq!(
        query("HistoryObservation", &observation, "1000"),
        json!({"verdict":"unreachable","complete":true,"pair":null})
    );
    assert_eq!(
        query("LatestObservation", &observation, "1"),
        json!({"verdict":"unknown","complete":false,"pair":null})
    );
    let coarse = query("HistoryObservation", &["observed"], "1000");
    assert_eq!(coarse["verdict"], "reachable");
    let pair = coarse["pair"].as_array().unwrap();
    assert!(
        pair.iter()
            .all(|trace| trace["commands"].as_array().unwrap().len() == 4)
    );
    assert_ne!(
        pair[0]["finalState"]["proof"],
        pair[1]["finalState"]["proof"]
    );
}

#[test]
fn observation_pair_compares_only_within_a_configuration_and_reports_empty() {
    use rcheck::conform::{ObservationQuery, observation_pair};
    let source = "model Compare
        enum Choice = A | B
        rel mode: one Choice
        var shown: one Bool
        var internal: one Choice
        init { shown internal = mode }
        reachable Observed { shown }
        scope steps = 2";
    let query = ObservationQuery {
        at: "Observed".into(),
        observe: vec!["shown".into()],
        distinguish: vec!["internal".into()],
        comparison: Default::default(),
        silent: Vec::new(),
        emit: Vec::new(),
    };
    for (source, verdict) in [
        (source.to_owned(), "unreachable"),
        (
            source.replace("shown internal = mode", "shown not shown internal = mode"),
            "empty",
        ),
    ] {
        let model = rmodel::parse(&source).unwrap();
        let answer = observation_pair(&model, &query, 100).unwrap();
        assert_eq!(
            answer,
            json!({"verdict": verdict, "complete": true, "pair": null})
        );
    }
    let invalid = ObservationQuery {
        observe: vec!["shwon".into()],
        ..query
    };
    let model = rmodel::parse(source).unwrap();
    assert!(
        observation_pair(&model, &invalid, 100)
            .unwrap_err()
            .contains("shwon")
    );
}

#[test]
fn observation_history_distinguishes_transient_notifications() {
    use std::process::Stdio;
    let model = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/model-patterns/subscriptions/notice-history.model"
    );
    let query = |at, comparison, limit| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rcheck"))
            .args(["observation-pair", model, "-", "--max-states", limit])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        serde_json::to_writer(child.stdin.take().unwrap(), &json!({"at": at, "observe": ["phase", "notice"], "distinguish": ["delivered"], "comparison": comparison})).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    assert_eq!(
        query("StrictDone", "snapshot", "1000")["verdict"],
        "reachable"
    );
    assert_eq!(
        query("StrictDone", "history", "1000"),
        json!({"complete": true, "verdict": "unreachable", "pair": null})
    );
    assert_eq!(
        query("StrictDone", "history", "1"),
        json!({"complete": false, "verdict": "unknown", "pair": null})
    );
    let report = query("OptionalDone", "history", "1000");
    assert_eq!(report["verdict"], "reachable");
    assert_eq!(report["complete"], true);
    let pair = report["pair"].as_array().unwrap();
    for trace in pair {
        assert_eq!(trace["commands"].as_array().unwrap().len(), 2);
        assert_eq!(trace["commands"][0]["visible"], false);
        assert_eq!(trace["finalState"]["notice"], false);
    }
    assert_ne!(
        pair[0]["finalState"]["delivered"],
        pair[1]["finalState"]["delivered"]
    );
}

#[test]
fn observation_history_handles_zero_step_pairs_and_scope_escape() {
    use rcheck::conform::{ObservationComparison, ObservationQuery, observation_pair};
    let model = rmodel::parse(
        "model Initial
        var secret: one Bool
        var notice: one Bool
        init { not notice }
        reachable Seen { not notice }
        scope steps = 2",
    )
    .unwrap();
    let query = ObservationQuery {
        at: "Seen".into(),
        observe: vec!["notice".into()],
        distinguish: vec!["secret".into()],
        comparison: ObservationComparison::History,
        silent: Vec::new(),
        emit: Vec::new(),
    };
    let report = observation_pair(&model, &query, 100).unwrap();
    assert_eq!(report["complete"], true);
    let pair = report["pair"].as_array().unwrap();
    assert!(pair.iter().all(|trace| trace["commands"] == json!([])));
    assert_ne!(pair[0]["initial"]["secret"], pair[1]["initial"]["secret"]);

    let model = rmodel::parse(
        "model Escape
        var secret: one Nat
        var notice: one Bool
        init { secret = 0 not notice }
        behavior advance() { update { secret = secret + 1 } }
        reachable Seen { not notice }
        scope Int = 0..0, steps = 2",
    )
    .unwrap();
    assert_eq!(
        observation_pair(&model, &query, 100).unwrap(),
        json!({"complete": false, "verdict": "unknown", "pair": null})
    );
}

#[test]
fn observation_history_can_skip_internal_retries() {
    use std::process::Stdio;
    let model = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/model-patterns/subscriptions/retry-observation.model"
    );
    let query = |at: &str, silent: &[&str], limit: &str| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rcheck"))
            .args(["observation-pair", model, "-", "--max-states", limit])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        serde_json::to_writer(child.stdin.take().unwrap(), &json!({"at": at, "observe": ["phase", "notice"], "distinguish": ["retried"], "comparison": "history", "silent": silent})).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    assert_eq!(query("QuietDone", &[], "1000")["verdict"], "unreachable");
    assert_eq!(
        query("ExposedDone", &["retry"], "1000")["verdict"],
        "unreachable"
    );
    assert_eq!(
        query("QuietDone", &["retry"], "1"),
        json!({"verdict":"unknown","complete":false,"pair":null})
    );
    let answer = query("QuietDone", &["retry"], "1000");
    assert_eq!(answer["verdict"], "reachable");
    assert_eq!(answer["complete"], true);
    let pair = answer["pair"].as_array().unwrap();
    let mut lengths: Vec<_> = pair
        .iter()
        .map(|trace| trace["commands"].as_array().unwrap().len())
        .collect();
    lengths.sort_unstable();
    assert_eq!(lengths, [2, 3]);
    assert_ne!(
        pair[0]["finalState"]["retried"],
        pair[1]["finalState"]["retried"]
    );
}

#[test]
fn silent_history_minimizes_total_commands_instead_of_product_edges() {
    use rcheck::conform::{ObservationComparison, ObservationQuery, observation_pair};
    let model = rmodel::parse("model Weighted
        enum Phase = Ready | Silent1 | Silent2 | Silent3 | Public1 | Public2 | Done
        var phase: one Phase
        var internal: one Bool
        init { phase = Ready not internal }
        behavior hide1() { guard { phase = Ready } update { phase = Silent1 } }
        behavior hide2() { guard { phase = Silent1 } update { phase = Silent2 } }
        behavior hide3() { guard { phase = Silent2 } update { phase = Silent3 } }
        behavior direct() {
            guard { phase in Ready + Silent3 }
            update { internal = (if { phase = Silent3 } then true else false) phase = Done }
        }
        behavior show1() { guard { phase = Ready } update { phase = Public1 } }
        behavior show2() { guard { phase = Public1 } update { phase = Public2 } }
        behavior show3(value: Bool) { guard { phase = Public2 } update { internal = value phase = Done } }
        reachable Final { phase = Done }
        scope steps = 8").unwrap();
    let mut query = ObservationQuery {
        at: "Final".into(),
        observe: vec!["phase".into()],
        distinguish: vec!["internal".into()],
        comparison: ObservationComparison::History,
        silent: vec!["hide1".into(), "hide2".into(), "hide3".into()],
        emit: Vec::new(),
    };
    let result = observation_pair(&model, &query, 1000).unwrap();
    assert_eq!(result["complete"], true);
    let mut lengths: Vec<_> = result["pair"]
        .as_array()
        .unwrap()
        .iter()
        .map(|trace| trace["commands"].as_array().unwrap().len())
        .collect();
    lengths.sort_unstable();
    assert_eq!(lengths, [1, 4]);
    query.silent.push("missing".into());
    assert!(
        observation_pair(&model, &query, 1000)
            .unwrap_err()
            .contains("missing")
    );
    query.comparison = ObservationComparison::Snapshot;
    assert!(
        observation_pair(&model, &query, 1000)
            .unwrap_err()
            .contains("require history")
    );
}

#[test]
fn history_events_can_depend_on_arguments_and_both_snapshots() {
    use std::process::Stdio;
    let model = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/model-patterns/subscriptions/retry-threshold.model"
    );
    let query = |at, emit| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rcheck"))
            .args(["observation-pair", model, "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        serde_json::to_writer(child.stdin.take().unwrap(), &json!({"at": at, "observe": ["phase"], "distinguish": ["attempts"], "comparison": "history", "emit": emit})).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let emit = json!([{"action":"retry", "event":"exhausted", "when":"after(attempts) = Exhausted and (cause = Fatal or before(attempts) = Once)"}]);
    let pair = query("Finished", &emit);
    assert_eq!(pair["verdict"], "reachable");
    let traces = pair["pair"].as_array().unwrap();
    assert!(traces.iter().all(|trace| trace["events"] == json!([])));
    assert!(traces.iter().any(|trace| {
        trace["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|call| call == &json!({"kind":"retry","cause":"Transient"}))
    }));
    assert_eq!(
        query("WithoutFirstRetry", &emit),
        json!({"verdict":"unreachable","complete":true,"pair":null})
    );
    let disabled =
        json!([{"action":"retry", "event":"exhausted", "when":"after(attempts) = NoneYet"}]);
    assert_eq!(
        query("WithoutFirstRetry", &disabled)["verdict"],
        "reachable"
    );
    let named = json!([{"action":"finish", "event":"finished"}, emit[0].clone()]);
    let result = query("Finished", &named);
    let traces = result["pair"].as_array().unwrap();
    for trace in traces {
        let step = trace["commands"].as_array().unwrap().len();
        assert_eq!(
            trace["events"],
            json!([{"step":step,"event":"finished","observation":{"phase":"Done"}}])
        );
    }
}

#[test]
fn history_requires_equal_event_names_even_when_payload_fields_agree() {
    use rcheck::conform::{
        ObservationComparison, ObservationEmission, ObservationQuery, observation_pair,
    };
    let model = rmodel::parse(
        "model Events
        var done: one Bool
        var accepted: one Bool
        init { not done not accepted }
        behavior accept() { guard { not done } update { done = true accepted = true } }
        behavior reject() { guard { not done } update { done = true } }
        reachable Final { done }
        scope steps = 4",
    )
    .unwrap();
    let mut query = ObservationQuery {
        at: "Final".into(),
        observe: vec!["done".into()],
        distinguish: vec!["accepted".into()],
        comparison: ObservationComparison::History,
        silent: vec![],
        emit: vec![
            ObservationEmission {
                action: "accept".into(),
                event: "accepted".into(),
                when: Some("after(done)".into()),
            },
            ObservationEmission {
                action: "reject".into(),
                event: "rejected".into(),
                when: None,
            },
        ],
    };
    assert_eq!(
        observation_pair(&model, &query, 1000).unwrap()["verdict"],
        "unreachable"
    );
    query.emit[1].event = "accepted".into();
    assert_eq!(
        observation_pair(&model, &query, 1000).unwrap()["verdict"],
        "reachable"
    );
    query.emit[0].when = Some("after(typo)".into());
    assert!(
        observation_pair(&model, &query, 1000)
            .unwrap_err()
            .contains("typo")
    );
}

#[test]
fn action_with_a_kind_parameter_round_trips_through_witness_and_validation() {
    let model = rmodel::parse(
        "
model PickKind
enum Choice = First | Second
var picked: lone Choice
init { no picked }
behavior choose(kind: Choice) { update { picked = kind } }
transition PicksArgument on choose { after(picked) = kind }
reachable Picked { picked = Second }
scope steps = 3
",
    )
    .unwrap();
    let result = rcheck::conform::witness(&model, "Picked", 100).unwrap();
    let trace = &result["trace"];
    assert_eq!(
        trace["commands"],
        json!([{ "kind": "choose", "$kind": "Second" }])
    );
    let validator = rcheck::conform::Validator::new(&model).unwrap();
    let session = validator
        .start(&trace["config"], &trace["initial"])
        .unwrap();
    assert_eq!(
        session.validate(&trace["commands"][0]).unwrap(),
        trace["finalState"]
    );
    assert!(session.validate(&json!({"kind": "choose"})).is_err());
    assert!(session.validate(&json!({"kind": "Second"})).is_err());
}

#[test]
fn integer_boundary_cases_vary_kind_argument_and_preserve_the_action_tag() {
    let model = rmodel::parse(
        "
model PickCount
var count: one Nat
init { count = 0 }
behavior pick(kind: Nat) { guard { kind > 0 } update { count = kind } }
scope Int = 0..2, steps = 2
",
    )
    .unwrap();
    let corpus = rcheck::conform::boundary_cases(&model, 100).unwrap();
    let examples = corpus["pairs"][0]["examples"].as_array().unwrap();
    assert_eq!(examples[0]["command"], json!({"kind": "pick", "$kind": 0}));
    assert_eq!(examples[0]["accepted"], false);
    assert_eq!(examples[1]["command"], json!({"kind": "pick", "$kind": 1}));
    assert_eq!(examples[1]["accepted"], true);
    assert_eq!(examples[1]["state"], json!({"count": 1}));
}

#[test]
fn delivery_gap_progress_distinguishes_strong_and_weak_retry_fairness() {
    let source =
        include_str!("../../../examples/model-patterns/subscriptions/delivery-ticket-gap.model");
    let model = rmodel::parse(source).unwrap();
    let question = "SealedEventuallyRecycles";
    let answer = |model, budget| {
        rcheck::check(
            model,
            question,
            &rcheck::Selection::Named("explicit".into()),
            budget,
        )
        .unwrap()
        .answers
        .remove(0)
    };
    let budget = rcheck::Budget::default();
    let normal = answer(&model, budget);
    assert_eq!(normal.verdict, rcheck::Verdict::Holds, "{normal:?}");

    let weak = rmodel::parse(&source.replace("strong arrive(*)", "weak arrive(*)")).unwrap();
    let fault = answer(&weak, budget);
    assert_eq!(fault.verdict, rcheck::Verdict::Violated, "{fault:?}");
    assert!(fault.lasso.is_some());

    let bounded = answer(&model, rcheck::Budget { work: 1, ..budget });
    assert_eq!(bounded.verdict, rcheck::Verdict::Unknown, "{bounded:?}");
}

#[test]
fn witness_snapshots_use_validator_set_order_and_keep_sequence_order() {
    let model = rmodel::parse(
        "
model SnapshotOrder
enum Item = Zeta | Alpha
enum Phase = Idle | Started | Done
sequence Trail = Item, max = 2
record Envelope { trail: Trail }
rel allowed: Item -> set Item
fact { allowed = Zeta -> Item }
var received: set Item
var inbox: Item -> set Item
var last: one Envelope
var phase: one Phase
init {
  received = Item inbox = Alpha -> Item
  last = Envelope { trail: Trail[Zeta, Alpha] } phase = Idle
}
behavior start() { guard { phase = Idle } update { phase = Started } }
behavior finish() { guard { phase = Started } update { phase = Done } }
reachable Finished { phase = Done }
eventually FinishesWithoutFairness { phase = Done }
scope steps = 3
",
    )
    .unwrap();

    let reached = rcheck::conform::witness(&model, "Finished", 20_000).unwrap();
    let trace = &reached["trace"];
    assert_eq!(
        trace["commands"],
        json!([{ "kind": "start" }, { "kind": "finish" }])
    );
    assert_eq!(trace["config"]["allowed"]["Zeta"], json!(["Alpha", "Zeta"]));
    let expected_history = json!({ "trail": { "$sequence": ["Zeta", "Alpha"] } });
    for snapshot in [&trace["initial"], &trace["finalState"]] {
        assert_eq!(snapshot["received"], json!(["Alpha", "Zeta"]));
        assert_eq!(snapshot["inbox"]["Alpha"], json!(["Alpha", "Zeta"]));
        assert_eq!(snapshot["last"], expected_history);
    }

    let validator = rcheck::conform::Validator::new(&model).unwrap();
    // Set order on input can differ from the canonical output.
    let mut unordered = trace["initial"].clone();
    unordered["received"] = json!(["Zeta", "Alpha"]);
    unordered["inbox"]["Alpha"] = json!(["Zeta", "Alpha"]);
    let mut unordered_config = trace["config"].clone();
    unordered_config["allowed"]["Zeta"] = json!(["Zeta", "Alpha"]);
    let mut wrong_sequence = unordered.clone();
    wrong_sequence["last"]["trail"]["$sequence"] = json!(["Alpha", "Zeta"]);
    assert!(validator.start(&unordered_config, &wrong_sequence).is_err());

    let mut session = validator.start(&unordered_config, &unordered).unwrap();
    session.replay(&trace["commands"][0]).unwrap();
    assert_eq!(
        session.validate(&trace["commands"][1]).unwrap(),
        trace["finalState"]
    );

    let lasso = rcheck::conform::witness(&model, "FinishesWithoutFairness", 20_000).unwrap();
    assert_eq!(lasso["verdict"], "violated");
    assert_eq!(lasso["trace"]["config"], trace["config"]);
    assert_eq!(lasso["trace"]["initial"], trace["initial"]);
    assert_eq!(lasso["trace"]["finalState"]["last"], expected_history);
}

#[test]
fn positive_transition_query_replays_a_same_checkpoint_obsolete_reply() {
    let model_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/model-patterns/subscriptions/partition-restart.model",
    );
    let predicate = "attempt = Second and before(active) = Third and                      some before(Third.requested) and unchanged(accepted)";
    let query = |predicate: &str, budget: &str| {
        Command::new(env!("CARGO_BIN_EXE_rcheck"))
            .args([
                "witness-step",
                model_path,
                "receive",
                predicate,
                "--max-states",
                budget,
            ])
            .output()
            .unwrap()
    };
    let parse = |output: std::process::Output| -> Value {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    };
    let result = parse(query(predicate, "20000"));
    assert_eq!(result["verdict"], "reachable");
    assert_eq!(result["complete"], true);
    let trace = &result["trace"];
    let commands = trace["commands"].as_array().unwrap();
    assert_eq!(commands.len(), 7);
    assert_eq!(
        commands.last().unwrap(),
        &json!({"kind": "receive", "attempt": "Second"})
    );
    let model = rmodel::parse(&std::fs::read_to_string(model_path).unwrap()).unwrap();
    let validator = rcheck::conform::Validator::new(&model).unwrap();
    let mut replay = validator
        .start(&trace["config"], &trace["initial"])
        .unwrap();
    for command in &commands[..commands.len() - 1] {
        replay.replay(command).unwrap();
    }
    assert_eq!(
        replay.validate(commands.last().unwrap()).unwrap(),
        trace["finalState"]
    );
    assert_eq!(trace["finalState"]["accepted"], Value::Null);

    let missed = parse(query(
        "attempt = First and before(active) = Third and after(accepted) = First",
        "20000",
    ));
    assert_eq!(missed["verdict"], "unreachable");
    assert_eq!(missed["trace"], Value::Null);
    let bounded = parse(query(predicate, "0"));
    assert_eq!(bounded["verdict"], "unknown");
    assert_eq!(bounded["diagnostics"][0]["kind"], "budget");
    assert_eq!(bounded["trace"], Value::Null);
    let invalid = query("some Missing", "20000");
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("unknown name Missing"));
}

#[test]
fn positive_transition_queries_distinguish_empty_and_out_of_scope() {
    let empty = rmodel::parse(
        "model Empty enum Item = A init { no Item }
         behavior advance() {} scope steps = 2",
    )
    .unwrap();
    let result = rcheck::conform::transition_witness(&empty, "advance", "some Item", 100).unwrap();
    assert_eq!(result["verdict"], "empty");
    assert_eq!(result["trace"], Value::Null);

    let scoped = rmodel::parse(
        "model Bounded var n: one Nat init { n = 0 }
         behavior advance() { update { n = n + 1 } }
         scope Int = 0..0, steps = 2",
    )
    .unwrap();
    let result =
        rcheck::conform::transition_witness(&scoped, "advance", "after(n) = 1", 100).unwrap();
    assert_eq!(result["verdict"], "unknown");
    assert_eq!(result["complete"], false);
    assert_eq!(result["diagnostics"][0]["kind"], "scope_exit");
    assert_eq!(result["trace"], Value::Null);
}

#[test]
fn named_derivations_work_in_queries_after_ir_roundtrip() {
    use rcheck::conform::{ObservationQuery, observation_pair, transition_witness};
    use serde_json::json;
    let model = rmodel::parse(
        "model Derived
         enum Stage = Ready | Done
         var stage: one Stage
         var detail: one Bool
         derive completed = stage & Done
         derive outcome = completed -> detail
         init { stage = Ready not detail }
         behavior finish(result: Bool) {
           guard { no completed }
           update { stage = Done detail = result }
         }
         reachable Final { some completed }
         scope steps = 3",
    )
    .unwrap();
    let ir = rcheck::ir::to_ir(&model).unwrap();
    let restored = rcheck::ir::from_ir(&ir).unwrap();
    for input in [&model, &restored] {
        let report = transition_witness(
            input,
            "finish",
            "no before(completed) and some after(completed)",
            1000,
        )
        .unwrap();
        assert_eq!(report["verdict"], "reachable");
        for comparison in ["snapshot", "history"] {
            let query: ObservationQuery = serde_json::from_value(json!({
                "at": "Final", "observe": ["completed"], "distinguish": ["outcome"],
                "comparison": comparison,
            }))
            .unwrap();
            let report = observation_pair(input, &query, 1000).unwrap();
            assert_eq!(report["verdict"], "reachable", "{report}");
        }
        let query: ObservationQuery = serde_json::from_value(json!({
            "at": "Final", "observe": ["completed"], "distinguish": ["outcome"],
            "comparison": "history",
            "emit": [{"action": "finish", "event": "done", "when": "some after(completed)"}],
        }))
        .unwrap();
        let report = observation_pair(input, &query, 1000).unwrap();
        assert_eq!(
            report["pair"][0]["events"][0]["observation"],
            json!({"completed": [["Done"]]})
        );
    }
}

#[test]
fn cli_distinguishes_an_argument_budget_from_a_state_budget() {
    use serde_json::{Value, json};
    use std::process::Command;

    let model = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/model-patterns/crash-recovery/refill-queue.model"
    );
    for (limit, verdict, found) in [("1369", "unknown", false), ("1370", "reachable", true)] {
        let output = Command::new(env!("CARGO_BIN_EXE_rcheck"))
            .args([
                "witness-step",
                model,
                "refill",
                "some after(pending)",
                "--max-arguments",
                limit,
                "--max-states",
                "4",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["verdict"], verdict);
        assert_eq!(report["complete"], false);
        assert_eq!(report["trace"].is_object(), found);
        assert_eq!(
            report["diagnostics"],
            json!([{"kind":"budget","resource":"arguments"}])
        );
    }

    let output = Command::new(env!("CARGO_BIN_EXE_rcheck"))
        .args([
            "dead-code",
            model,
            "--max-states",
            "4",
            "--max-arguments",
            "1370",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["coverage"], "unknown");
    assert_eq!(
        report["actions"],
        json!([{"name":"refill","status":"reachable"}])
    );
}
