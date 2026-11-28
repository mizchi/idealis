//! Plan 0108: the kernel IR as JSON round-trips through rmodel's checker
//! without changing the model or its verdicts, and malformed IR is
//! rejected before type checking.
#[path = "../../idealis-model/examples/support/generator.rs"]
#[allow(dead_code)]
mod generator;

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn verdicts(model: &idealis_model::Model) -> String {
    let report = idealis_model::reference::analyze(model, 100_000);
    format!(
        "{:?} {:?} {:?}",
        report.verdicts, report.declarations, report.scope
    )
}

fn round_trip(label: &str, model: &idealis_model::Model) {
    let ir = idealis_check::ir::to_ir(model).unwrap_or_else(|e| panic!("{label}: {e}"));
    let json = serde_json::to_string(&ir).unwrap();
    let (back, omitted) =
        idealis_check::ir::parse(&json).unwrap_or_else(|e| panic!("{label}: {e}\n{json}"));
    assert!(omitted.is_empty());
    assert_eq!(
        idealis_check::ir::to_ir(&back).unwrap(),
        ir,
        "{label}: the IR changed"
    );
    assert_eq!(
        verdicts(&back),
        verdicts(model),
        "{label}: the verdicts changed"
    );
}

#[test]
fn delivery_revision_resolves_record_projections_again_after_ir_import() {
    let model = idealis_model::parse(include_str!(
        "../../../examples/model-patterns/subscriptions/delivery-revision.ideal"
    ))
    .unwrap();
    round_trip("delivery revision projection", &model);
}

#[test]
fn a_selection_preserves_its_binding_and_prestate_through_ir() {
    let model = idealis_model::parse(
        r#"model Selection
enum Item = Old | Boundary | Future
rel generation: Item -> one Nat
fact { generation = (Old -> 0) + (Boundary -> 1) + (Future -> 2) }
var kept: set Item
var floor: one Nat
init { kept = Item floor = 0 }
behavior prune() {
  update { kept = kept - select(item: Item | item.generation < floor + 1) floor = 1 }
}

reachable BoundaryAndFuture { kept = Boundary + Future }
scope Int = 0..2, steps = 2
"#,
    )
    .unwrap();
    round_trip("atom selection", &model);
    let report = idealis_check::check(
        &model,
        "BoundaryAndFuture",
        &idealis_check::Selection::All,
        idealis_check::Budget::default(),
    )
    .unwrap();
    assert_eq!(report.answers.len(), 3);
    for answer in report.answers {
        assert_eq!(
            answer.verdict,
            idealis_check::Verdict::Reachable,
            "{}",
            answer.checker
        );
        assert_eq!(answer.depth, Some(1));
    }
}

#[test]
fn sat_reports_selection_as_unsupported_and_other_checkers_answer() {
    let model = idealis_model::parse(
        r#"model SelectAtoms
enum Item = Left | Right
var kept: set Item
init { kept = Item }
behavior prune() { update { kept = select(item: Item | item in kept and item != Left) } }
reachable OnlyRight { kept = Right }
scope steps = 2
"#,
    )
    .unwrap();
    let report = idealis_check::check(
        &model,
        "OnlyRight",
        &idealis_check::Selection::All,
        idealis_check::Budget::default(),
    )
    .unwrap();
    assert_eq!(report.answers.len(), 3);
    assert!(
        report
            .answers
            .iter()
            .all(|answer| answer.verdict == idealis_check::Verdict::Reachable)
    );
    assert_eq!(report.unsupported.len(), 1);
    assert_eq!(report.unsupported[0].checker, "sat");
    assert!(report.unsupported[0].reason.contains("comprehension"));
}

#[test]
fn integer_set_operations_round_trip_without_becoming_arithmetic() {
    let model = idealis_model::parse(
        r#"model IntegerSet
var values: some Int
init { values = -2 }
behavior replace() {
  guard { -2 in values 1 not in values }
  update { values = union(difference(values, -2), 1) }
}
reachable Replacement { 1 in values and -2 not in values }
scope Int = -2..1, steps = 2
"#,
    )
    .unwrap();
    round_trip("integer sets", &model);
    let ir = idealis_check::ir::to_ir(&model).unwrap();
    let json = serde_json::to_value(&ir).unwrap().to_string();
    assert!(json.contains("\"set_union\""));
    assert!(json.contains("\"set_difference\""));
    let model = idealis_check::ir::from_ir(&ir).unwrap();
    let report = idealis_check::check(
        &model,
        "Replacement",
        &idealis_check::Selection::All,
        idealis_check::Budget::default(),
    )
    .unwrap();
    assert_eq!(report.answers.len(), 3);
    for answer in report.answers {
        assert_eq!(
            answer.verdict,
            idealis_check::Verdict::Reachable,
            "{}",
            answer.checker
        );
        assert_eq!(answer.depth, Some(1));
    }
}

#[test]
fn checkout_round_trips_fields_shared_by_different_record_types() {
    let model = idealis_model::parse(include_str!(
        "../../../examples/model-patterns/checkout/checkout.ideal"
    ))
    .unwrap();
    round_trip("checkout", &model);
}

#[test]
fn a_transition_local_keeps_its_binding_phase_when_joined_with_after_state() {
    let prefix = "model Captured
enum Ticket = First | Second
enum Reader = Active
var pending: one Ticket
var claimed: Ticket -> set Reader
init { pending = First no claimed }
behavior claim(reader: Reader) {
  update { pending = Second claimed = claimed + (pending -> reader) }
}
transition ClaimsPrevious on claim {";
    let suffix = "} scope steps = 3";
    let with_local = "let ticket = before(pending) reader in ticket.after(claimed)";
    let inline = "reader in before(pending).after(claimed)";
    let scoped = idealis_model::parse(&format!("{prefix} {with_local} {suffix}")).unwrap();
    let direct = idealis_model::parse(&format!("{prefix} {inline} {suffix}")).unwrap();
    assert_eq!(
        idealis_check::ir::to_ir(&scoped).unwrap(),
        idealis_check::ir::to_ir(&direct).unwrap()
    );
    round_trip("transition local", &scoped);
    let check = |model: &idealis_model::Model, expected| {
        let report = idealis_check::check(
            model,
            "ClaimsPrevious",
            &idealis_check::Selection::All,
            idealis_check::Budget::default(),
        )
        .unwrap();
        assert_eq!(report.answers.len(), 4);
        assert!(report.answers.iter().all(|a| a.verdict == expected));
    };
    check(&scoped, idealis_check::Verdict::Holds);
    let wrong = prefix.replace("(pending -> reader)", "(Second -> reader)");
    check(
        &idealis_model::parse(&format!("{wrong} {with_local} {suffix}")).unwrap(),
        idealis_check::Verdict::Violated,
    );
    let regrouped = "let ticket = before(pending) reader in after(ticket.claimed)";
    let error = idealis_model::parse(&format!("{prefix} {regrouped} {suffix}")).unwrap_err();
    assert!(error.message.contains("outside before/after"), "{error}");
}

#[test]
fn equivalence_checks_both_directions_of_the_delivery_conflict() {
    let base = include_str!("../../../examples/model-patterns/subscriptions/delivery-race.ideal");
    let inline = "  (conflict implies some readout.Delivered and some readout.Missing) and\n  (some readout.Delivered and some readout.Missing implies conflict)";
    let equivalence = "  conflict iff some readout.Delivered and some readout.Missing";
    assert!(base.contains(equivalence));
    let direct = idealis_model::parse(&base.replace(equivalence, inline)).unwrap();
    let model = idealis_model::parse(base).unwrap();
    assert_eq!(
        idealis_check::ir::to_ir(&model).unwrap(),
        idealis_check::ir::to_ir(&direct).unwrap()
    );
    round_trip("delivery conflict equivalence", &model);
    for (source, verdict) in [
        (base.to_owned(), idealis_check::Verdict::Holds),
        (
            base.replace("conflict = true", "conflict = false"),
            idealis_check::Verdict::Violated,
        ),
        (
            base.replace("winner = attempt", "winner = attempt conflict = true"),
            idealis_check::Verdict::Violated,
        ),
    ] {
        let model = idealis_model::parse(&source).unwrap();
        let report = idealis_check::check(
            &model,
            "ConflictHasTwoCompleteAnswers",
            &idealis_check::Selection::All,
            idealis_check::Budget::default(),
        )
        .unwrap();
        assert_eq!(report.answers.len(), 4);
        for answer in report.answers {
            assert_eq!(answer.verdict, verdict, "{}", answer.checker);
        }
    }
    let precedence = base.replace(equivalence, "  false implies false iff false");
    let explicit = precedence.replace(
        "false implies false iff false",
        "(false implies false) iff false",
    );
    assert_eq!(
        idealis_check::ir::to_ir(&idealis_model::parse(&precedence).unwrap()).unwrap(),
        idealis_check::ir::to_ir(&idealis_model::parse(&explicit).unwrap()).unwrap()
    );
    let chain = base.replace(equivalence, "  conflict iff true iff false");
    assert!(
        idealis_model::parse(&chain)
            .unwrap_err()
            .message
            .contains("parentheses")
    );
}

#[test]
fn removing_keys_preserves_a_row_with_the_same_value_and_ignores_missing_keys() {
    let source = r#"model RemoveKeys
enum Key = Old | Keep | Missing
enum Item = Shared | Extra
var rows: Key -> set Item
init { rows = (Old -> (Shared + Extra)) + (Keep -> Shared) }
behavior prune() { update { rows = withoutKeys(rows, Old + Missing) } }
reachable Kept { rows = (Keep -> Shared) }
scope steps = 2
"#;
    let model = idealis_model::parse(source).unwrap();
    round_trip("remove keys", &model);
    let report = idealis_check::check(
        &model,
        "Kept",
        &idealis_check::Selection::All,
        idealis_check::Budget::default(),
    )
    .unwrap();
    assert_eq!(report.answers.len(), 4);
    for answer in report.answers {
        assert_eq!(
            answer.verdict,
            idealis_check::Verdict::Reachable,
            "{}",
            answer.checker
        );
    }
    assert!(
        idealis_model::parse(&source.replace("Old + Missing)", "Shared)"))
            .unwrap_err()
            .message
            .contains("different sorts")
    );
    assert!(
        idealis_model::parse(&source.replace("withoutKeys(rows,", "withoutKeys(Old,"))
            .unwrap_err()
            .message
            .contains("binary relation")
    );
}

#[test]
fn update_bindings_expand_to_the_same_ir_as_direct_assignments() {
    let prefix = "model Local var number: one Nat var receipt: one Nat
                  init { number = 0 receipt = 0 } behavior write() { guard { number = 0 } update {";
    let suffix = "} } reachable Written { number = 1 receipt = 1 } scope Int = 0..1, steps = 2";
    let scoped = idealis_model::parse(&format!(
        "{prefix} let next = number + 1 number = next receipt = next {suffix}"
    ))
    .unwrap();
    let direct = idealis_model::parse(&format!(
        "{prefix} number = number + 1 receipt = number + 1 {suffix}"
    ))
    .unwrap();
    assert_eq!(
        idealis_check::ir::to_ir(&scoped).unwrap(),
        idealis_check::ir::to_ir(&direct).unwrap()
    );
    round_trip("update bindings", &scoped);
}

#[test]
fn entity_definitions_survive_ir_and_cannot_name_an_unknown_sort() {
    let model = idealis_model::parse(include_str!("../../../fixtures/rcheck/bank.model")).unwrap();
    let mut ir = idealis_check::ir::to_ir(&model).unwrap();
    let (round_tripped, _) =
        idealis_check::ir::parse(&serde_json::to_string(&ir).unwrap()).unwrap();
    assert_eq!(
        round_tripped
            .entity_definitions
            .get("Account")
            .map(String::as_str),
        Some("入出金によって残高が変わる口座。")
    );
    ir.entity_definitions
        .insert("Missing".into(), "unknown entity".into());
    assert!(
        idealis_check::ir::from_ir(&ir)
            .unwrap_err()
            .contains("unknown entity Missing")
    );
}

#[test]
fn localized_glossary_is_the_same_from_ideal_source_and_ir() {
    let source = r#"model Ledger
entity Account description = {
  default: "Account",
  ja: """残高を持つ口座。
入出金で変わる。""",
  "en-US": "An account that holds a balance."
}
entity Currency description = { en: "Currency" }
scope Account = 1, Currency = 1, steps = 2
"#;
    let model = idealis_model::parse(source).unwrap();
    round_trip("localized glossary", &model);
    let ir = idealis_check::ir::to_ir(&model).unwrap();
    let directory = std::env::temp_dir();
    let name = format!("rcheck-localized-glossary-{}", std::process::id());
    let ideal = directory.join(format!("{name}.ideal"));
    let json = directory.join(format!("{name}.json"));
    std::fs::write(&ideal, source).unwrap();
    std::fs::write(&json, serde_json::to_vec(&ir).unwrap()).unwrap();
    let generate = |path: &std::path::Path, target: &str| {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_idealis-check"))
            .arg("generate")
            .arg(path)
            .arg(target)
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    let expected = serde_json::json!({
        "Account": {
            "default": "Account",
            "en-us": "An account that holds a balance.",
            "ja": "残高を持つ口座。\n入出金で変わる。"
        },
        "Currency": {"en": "Currency"}
    });
    let from_source = generate(&ideal, "glossary-localized");
    let from_ir = generate(&json, "glossary-localized");
    let legacy = generate(&ideal, "glossary");
    std::fs::remove_file(ideal).unwrap();
    std::fs::remove_file(json).unwrap();
    assert_eq!(from_source, expected);
    assert_eq!(from_ir, expected);
    assert_eq!(legacy, serde_json::json!({ "Account": "Account" }));

    let mut invalid = ir;
    invalid.entity_translations.insert(
        "Missing".into(),
        [("ja".into(), "説明".into())].into_iter().collect(),
    );
    assert!(
        idealis_check::ir::from_ir(&invalid)
            .unwrap_err()
            .contains("unknown entity Missing")
    );
}

#[test]
fn the_corpus_round_trips() {
    let mut count = 0;
    for dir in ["fixtures/rmodel", "spec/models", "fixtures/rcheck"] {
        for entry in std::fs::read_dir(root().join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "model") {
                let model = idealis_model::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
                round_trip(&path.display().to_string(), &model);
                count += 1;
            }
        }
    }
    assert!(count >= 8, "{count} models");
}

#[test]
fn generated_models_round_trip() {
    let mut count = 0;
    for seed in 9800..=9899 {
        if let Ok(model) = idealis_model::parse(&generator::generate(seed)) {
            round_trip(&format!("seed {seed}"), &model);
            count += 1;
        }
    }
    assert!(count > 90, "{count} models");
}

#[test]
fn malformed_ir_is_rejected() {
    let model = idealis_model::parse(
        &std::fs::read_to_string(root().join("fixtures/rmodel/permissions.model")).unwrap(),
    )
    .unwrap();
    let good = serde_json::to_value(idealis_check::ir::to_ir(&model).unwrap()).unwrap();
    let mut cases = Vec::new();
    let mut unknown_op = good.clone();
    unknown_op["facts"][0]["op"] = "xor".into();
    cases.push(("unknown op", unknown_op, "unknown variant"));
    let mut missing = good.clone();
    missing["relations"][0]
        .as_object_mut()
        .unwrap()
        .remove("shape");
    cases.push(("missing field", missing, "missing field"));
    let mut extra = good.clone();
    extra["actions"][0]["note"] = "x".into();
    cases.push(("unknown field", extra, "unknown field"));
    let mut version = good.clone();
    version["version"] = 2.into();
    cases.push(("version", version, "version"));
    let future = serde_json::json!({
        "version": 2,
        "futureRootField": true,
        "facts": [{"op": "future-opcode"}],
        "scope": {"steps": 2}
    });
    cases.push(("future schema", future, "IR version 2 (want 1)"));
    let mut ill_typed = good.clone();
    ill_typed["relations"][0]["shape"]["to"] = "Nowhere".into();
    cases.push(("unknown sort", ill_typed, "unknown sig"));
    let mut steps = good.clone();
    steps["scope"]["steps"] = 0.into();
    cases.push(("trace scope", steps, "steps"));
    let mut sort_size = good.clone();
    sort_size["scope"]["sizes"]["User"] = 8.into();
    cases.push(("sort scope", sort_size, "outside"));
    let mut ints = good;
    ints["scope"]["ints"] = serde_json::json!([i64::MIN, i64::MAX]);
    cases.push(("integer scope", ints, "at most 20"));
    for (label, json, needle) in cases {
        let error = idealis_check::ir::parse(&json.to_string()).unwrap_err();
        assert!(error.contains(needle), "{label}: {error}");
    }
}

#[test]
fn previously_published_pkl_v1_retains_its_verdicts_and_witness() {
    // This fixture was produced by the original Pkl adapter in 898b101d3.
    let legacy = root().join("fixtures/rcheck/ir-compat/pkl-bank-v1.json");
    let source = root().join("fixtures/rcheck/bank.model");
    let invoke = |args: &[&str]| {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_idealis-check"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    for question in ["NonNegative", "Rich"] {
        let report = |path: &std::path::Path| {
            invoke(&[
                path.to_str().unwrap(),
                "--checker",
                "all",
                "--question",
                question,
            ])
        };
        assert_eq!(report(&legacy), report(&source));
    }
    let witness = |path: &std::path::Path| invoke(&["witness", path.to_str().unwrap(), "Rich"]);
    let old = witness(&legacy);
    assert_eq!(old, witness(&source));
    assert_eq!(
        old["trace"]["commands"],
        serde_json::json!([{"kind":"deposit","a":"Account0","n":3}])
    );
    assert_eq!(old["trace"]["finalState"]["balance"]["Account0"], 3);
    let source_model = idealis_model::parse(&std::fs::read_to_string(source).unwrap()).unwrap();
    let validator = idealis_check::conform::Validator::new(&source_model).unwrap();
    let trace = &old["trace"];
    let mut session = validator
        .start(&trace["config"], &trace["initial"])
        .unwrap();
    for command in trace["commands"].as_array().unwrap() {
        session.replay(command).unwrap();
    }
    assert_eq!(session.snapshot(), trace["finalState"]);
}

#[test]
fn linked_models_round_trip() {
    for name in ["withdrawal", "raft"] {
        let path = root().join(format!("fixtures/rcheck/linked/{name}/project.rmodel.json"));
        let source = idealis_check::project::load_source(path.to_str().unwrap()).unwrap();
        round_trip(name, &idealis_model::parse(&source).unwrap());
    }
}

#[test]
fn cli_withholds_verdicts_when_an_adapter_omits_a_transition() {
    let source = "model Gate
        enum Flag = Off | On
        var x: one Flag
        init { x = Off }
        behavior open() { update { x = On } }
        invariant Closed { x = Off }
        scope steps = 3";
    let mut ir = idealis_check::ir::to_ir(&idealis_model::parse(source).unwrap()).unwrap();
    ir.actions.clear();
    ir.omitted.push(idealis_check::ir::Omitted {
        name: "open".into(),
        reason: "unsupported transition".into(),
    });
    let path = std::env::temp_dir().join(format!("rcheck-partial-{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_vec(&ir).unwrap()).unwrap();
    let executable = env!("CARGO_BIN_EXE_idealis-check");
    let check = std::process::Command::new(executable)
        .arg(&path)
        .args(["--question", "Closed"])
        .output()
        .unwrap();
    let capabilities = std::process::Command::new(executable)
        .arg("capabilities")
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();

    for output in [check, capabilities] {
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("open (unsupported transition)"));
    }
}

#[test]
fn the_adoption_self_model_uses_the_existing_transition_ir() {
    let source = include_str!("../../../spec/models/evidence-adoption.model");
    let explicit = source
        .replace("unchanged(current)", "(before(current) = after(current))")
        .replace("unchanged(adopted)", "before(adopted) = after(adopted)");
    assert_ne!(source, explicit);
    let model = idealis_model::parse(source).unwrap();
    let explicit = idealis_model::parse(&explicit).unwrap();
    assert_eq!(
        idealis_check::ir::to_ir(&model).unwrap(),
        idealis_check::ir::to_ir(&explicit).unwrap()
    );
    round_trip("adoption", &model);
}

#[test]
fn fifo_tail_round_trips_through_the_ir() {
    let source = concat!(
        include_str!("../../../examples/model-patterns/fifo/messages.ideal"),
        "\n",
        include_str!("../../../examples/model-patterns/fifo/mailbox.ideal"),
    );
    let model = idealis_model::parse(source).unwrap();
    let ir = idealis_check::ir::to_ir(&model).unwrap();
    let encoded = serde_json::to_string(&ir).unwrap();
    assert!(encoded.contains("\"op\":\"tail\""));
    round_trip("fifo", &model);

    let mut malformed: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    let actions = malformed["actions"].as_array_mut().unwrap();
    let dequeue = actions
        .iter_mut()
        .find(|action| action["name"] == "dequeue")
        .unwrap();
    let assign = dequeue["assignments"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|assignment| assignment["var"] == "queue")
        .unwrap();
    let of = assign["value"]["of"].take();
    assign["value"] = serde_json::json!({"op": "variant", "tag": "tail", "payload": of});
    let malformed: idealis_check::ir::Ir = serde_json::from_value(malformed).unwrap();
    assert!(
        idealis_check::ir::from_ir(&malformed)
            .unwrap_err()
            .contains("unknown variant tag tail")
    );
}

#[test]
fn a_conditional_update_preserves_the_single_completion_command_in_ir() {
    let model = idealis_model::parse(include_str!(
        "../../../examples/model-patterns/latest-request/search.ideal"
    ))
    .unwrap();
    let ir = idealis_check::ir::to_ir(&model).unwrap();
    let action = ir
        .actions
        .iter()
        .find(|action| action.name == "resolve")
        .unwrap();
    assert!(matches!(
        action.assignments[1].value,
        idealis_check::ir::E::If { .. }
    ));
    assert_eq!(ir.actions.len(), 2);
    round_trip("latest request", &model);
}

#[test]
fn history_prefix_round_trips_through_the_ir() {
    let model = idealis_model::parse(include_str!(
        "../../../examples/model-patterns/auth-history/history.ideal"
    ))
    .unwrap();
    round_trip("auth history", &model);
}

#[test]
fn an_action_scoped_transition_preserves_its_binding_in_the_ir() {
    let source = include_str!("../../../examples/model-patterns/retry/delivery.ideal");
    let model = idealis_model::parse(source).unwrap();
    round_trip("retry delivery", &model);

    let mut ir = idealis_check::ir::to_ir(&model).unwrap();
    ir.transitions[0].action = Some("loseResponse".into());
    assert!(idealis_check::ir::from_ir(&ir).is_ok());
    ir.transitions[0].action = Some("unknown".into());
    assert!(
        idealis_check::ir::from_ir(&ir)
            .unwrap_err()
            .contains("unknown action")
    );
}

#[test]
fn tab_settings_round_trip_the_relation_override() {
    let source = include_str!("../../../examples/model-patterns/tab-settings/settings.ideal");
    let model = idealis_model::parse(source).unwrap();
    round_trip("tab settings", &model);
}

#[test]
fn blocked_write_cancellation_round_trips_value_based_removal() {
    let model = idealis_model::parse(include_str!(
        "../../../examples/model-patterns/backpressure/channel.ideal"
    ))
    .unwrap();
    round_trip("backpressure", &model);
}

#[test]
fn stream_terminal_round_trips_sequence_elements() {
    let model = idealis_model::parse(include_str!(
        "../../../examples/model-patterns/stream-terminal/stream.ideal"
    ))
    .unwrap();
    round_trip("stream terminal", &model);
}

#[test]
fn guarded_optional_arithmetic_round_trips_through_ir() {
    let source = include_str!("../../../examples/model-patterns/lease/resync-order.ideal");
    let model = idealis_model::parse(source).unwrap();
    round_trip("guarded optional capture", &model);
}

#[test]
fn keys_preserves_its_operand_and_type_through_ir() {
    let model = idealis_model::parse(include_str!(
        "../../../fixtures/rmodel/relations/keys.model"
    ))
    .unwrap();
    round_trip("relation keys", &model);
    let ir = idealis_check::ir::to_ir(&model).unwrap();
    let mut json = serde_json::to_value(ir).unwrap();
    assert_eq!(json["actions"][0]["assignments"][0]["value"]["op"], "keys");
    json["actions"][0]["assignments"][0]["value"]["of"] =
        serde_json::json!({"op": "name", "name": "active"});
    let error = idealis_check::ir::parse(&json.to_string()).unwrap_err();
    assert!(
        error.to_string().contains("keys needs one binary relation"),
        "{error}"
    );
}

#[test]
fn transitive_closure_round_trips_with_its_binary_result() {
    let model = idealis_model::parse(include_str!(
        "../../../fixtures/rmodel/relations/closure.model"
    ))
    .unwrap();
    round_trip("positive closure", &model);
}

#[test]
fn named_derivations_expand_into_the_same_ir_with_phase_and_guard_reads() {
    let inline = "model Derived enum Item = A | B
rel available: set Item fact { available = Item }
var processed: set Item init { no processed }
behavior add(item: Item) {
  guard { item in available - processed }
  update { let chosen = item & (available - processed) processed = processed + chosen }
}
transition RemovesOne on add {
  after(available - processed) = before(available - processed) - item
}
reachable Done { no (available - processed) }
scope steps = 3";
    let named = inline
        .replace(
            "fact { available = Item }",
            "derive source = available fact { source = Item }",
        )
        .replace(
            "var processed: set Item init",
            "var processed: set Item derive remaining = source - processed init",
        )
        .replace("item in available - processed", "item in remaining")
        .replace("item & (available - processed)", "item & remaining")
        .replace(
            "after(available - processed) = before(available - processed)",
            "after(remaining) = before(remaining)",
        )
        .replace("no (available - processed)", "no remaining");
    let named = idealis_model::parse(&named).unwrap();
    let inline = idealis_model::parse(inline).unwrap();
    let mut named_ir = idealis_check::ir::to_ir(&named).unwrap();
    assert_eq!(named_ir.derivations.len(), 2);
    named_ir.derivations.clear();
    assert_eq!(named_ir, idealis_check::ir::to_ir(&inline).unwrap());
    round_trip("derivations", &named);
}

#[test]
fn count_of_a_binary_relation_round_trips_through_the_existing_count_node() {
    let model = idealis_model::parse(include_str!(
        "../../../fixtures/rmodel/relations/count-pairs.model"
    ))
    .unwrap();
    round_trip("binary cardinality", &model);
}

#[test]
fn intersections_narrow_both_columns_when_the_operand_sorts_are_contained() {
    let source = "model Narrow
        enum Legacy = Original | Secondary
        enum Current = Replacement
        type Registration = Legacy + Current
        rel delivered: Registration -> set Registration
        fact { delivered = (Original -> Secondary) + (Replacement -> Original) }
        var kept: Legacy -> set Legacy
        init { no kept }
        behavior retain() { update { kept = delivered & (Legacy -> Legacy) } }
        reachable Exported { kept = Original -> Secondary }
        scope steps = 2";
    let model = idealis_model::parse(source).unwrap();
    round_trip("narrowed intersection", &model);
    let report = idealis_check::check(
        &model,
        "Exported",
        &idealis_check::Selection::All,
        idealis_check::Budget::default(),
    )
    .unwrap();
    assert_eq!(report.answers.len(), 4);
    assert!(
        report
            .answers
            .iter()
            .all(|answer| answer.verdict == idealis_check::Verdict::Reachable)
    );
    assert!(idealis_model::parse(&source.replace("delivered &", "delivered +")).is_err());
}

#[test]
fn graph_paths_preserve_the_destination_sort_when_it_excludes_the_origin() {
    let source = "model Positioned
        enum Origin = Zero
        enum Entry = One | Two | Three
        type Position = Origin + Entry
        rel next: Position -> lone Entry
        fact { next = (Zero -> One) + (One -> Two) + (Two -> Three) }
        var reached: set Entry
        var paths: Position -> set Entry
        init { no reached no paths }
        behavior expand() {
          update { reached = reachableFrom(One, next) paths = closure(next) }
        }
        reachable EntryOnly { reached = Entry Zero.paths = Entry One.paths = Two + Three }
        reachable IncludesOrigin { reachableFrom(Zero, next) = Position }
        scope steps = 3";
    let model = idealis_model::parse(source).unwrap();
    round_trip("typed endpoints", &model);
    let report = idealis_check::check(
        &model,
        "EntryOnly",
        &idealis_check::Selection::All,
        idealis_check::Budget::default(),
    )
    .unwrap();
    assert_eq!(report.answers.len(), 4);
    assert!(
        report
            .answers
            .iter()
            .all(|answer| answer.verdict == idealis_check::Verdict::Reachable)
    );
    let result = rprob::witness(&model, "EntryOnly", 100_000).unwrap();
    assert_eq!(
        result.outcome,
        idealis_model::reference::Outcome::Reachable { depth: 1 }
    );
    assert!(result.complete);
    assert!(
        idealis_model::parse(&source.replace("reachableFrom(One,", "reachableFrom(Zero,")).is_err()
    );
}

#[test]
fn a_boolean_condition_lowers_to_the_same_ir_as_an_explicit_value_choice() {
    let source = "model Gate
enum Mode = Old | New
var mode: one Mode
var bool: one Bool
derive enabled = bool { mode = New, not bool }
init { mode = Old, not bool }
behavior enter() { update { mode = New } }
behavior mark() { update { bool = enabled } }
invariant SetOnlyInNew { bool implies mode = New }
reachable CanSet { bool }
scope steps = 4";
    let expanded = source.replace(
        "bool { mode = New, not bool }",
        "if { mode = New, not bool } then true else false",
    );
    let actual = idealis_model::parse(source).unwrap();
    let equivalent = idealis_model::parse(&expanded).unwrap();
    assert_eq!(
        idealis_check::ir::to_ir(&actual).unwrap(),
        idealis_check::ir::to_ir(&equivalent).unwrap(),
    );
    let report = idealis_model::reference::analyze(&actual, 100);
    assert_eq!(report.scope, idealis_model::reference::Outcome::Holds);
    assert_eq!(
        report.verdicts[0].outcome,
        idealis_model::reference::Outcome::Holds
    );
    assert_eq!(
        report.verdicts[1].outcome,
        idealis_model::reference::Outcome::Reachable { depth: 2 }
    );
    round_trip("Boolean condition", &actual);
}

#[test]
fn a_record_named_bool_keeps_its_literal_syntax() {
    let model = idealis_model::parse(
        "model Record
record bool { value: Bool }
var flag: one bool
init { flag = bool { value: false } }
behavior mark() { update { flag = bool { value: true } } }
reachable CanSet { flag.value }
scope steps = 3",
    )
    .unwrap();
    round_trip("record named bool", &model);
}

#[test]
fn finite_integer_quantifiers_respect_the_declared_domain_in_both_checkers() {
    use idealis_check::{Budget, Coverage, Selection, Verdict};

    let source = "model Indices
invariant NatIsNonnegative { all i: Nat | i >= 0 }
invariant NegativeBelongsToInt { some i: Int | i < 0 }
invariant AllNatIndicesExist { all i: Nat | i < 2 }
invariant NoNegativeNat { some i: Nat | i < 0 }
scope Int = -2..2, steps = 2";
    let model = idealis_model::parse(source).unwrap();
    round_trip("finite integer quantifiers", &model);
    for (question, expected) in [
        ("NatIsNonnegative", Verdict::Holds),
        ("NegativeBelongsToInt", Verdict::Holds),
        ("AllNatIndicesExist", Verdict::Violated),
        ("NoNegativeNat", Verdict::Violated),
    ] {
        let observed =
            idealis_check::check(&model, question, &Selection::All, Budget::default()).unwrap();
        assert_eq!(observed.answers.len(), 3, "{observed:?}");
        for answer in observed.answers {
            assert_eq!(answer.coverage, Coverage::Complete);
            assert_eq!(answer.verdict, expected, "{question}: {answer:?}");
        }
        assert_eq!(observed.unsupported[0].checker, "sat");
    }

    for source in [
        "model MissingScope invariant Checked { some i: Int | i = 0 }",
        "model EmptyNat invariant Checked { some i: Nat | i = 0 } scope Int = -2..-1",
    ] {
        assert!(
            idealis_model::parse(source)
                .unwrap_err()
                .message
                .contains("scope")
        );
    }
}

#[test]
fn expanded_bindings_keep_their_own_selection_and_quantifier_variables() {
    use idealis_check::{Budget, Selection, Verdict};

    let source = "model Choice
enum Item = A | B
var chosen: lone Item
derive _rmodel_bound_0 = B
derive available = select(item: Item | item = A)
derive admitted = bool { some item: Item | item = A }
derive early = select(later: Item | later = A)
derive later = B
derive matches = select(item: Item |
  A in available and A in early and admitted and item = _rmodel_bound_0)
init { no chosen }
behavior choose(item: Item) {
  guard { item in matches }
  update {
    let selected = select(option: Item | option = item)
    chosen = select(option: Item | option in selected)
  }
}
invariant PicksOnlyB { chosen in B }
reachable CanPick { chosen = B }
scope steps = 3";
    let manual = source
        .replace(
            "select(item: Item | item = A)",
            "select(option: Item | option = A)",
        )
        .replace(
            "some item: Item | item = A",
            "some admittedItem: Item | admittedItem = A",
        )
        .replace(
            "select(later: Item | later = A)",
            "select(earlyItem: Item | earlyItem = A)",
        )
        .replace(
            "derive matches = select(item: Item |",
            "derive matches = select(eligible: Item |",
        )
        .replace("admitted and item =", "admitted and eligible =")
        .replace(
            "select(option: Item | option in selected)",
            "select(result: Item | result in selected)",
        );
    let model = idealis_model::parse(source).unwrap();
    let renamed = idealis_model::parse(&manual).unwrap();
    round_trip("expanded bound variables", &model);
    assert_eq!(verdicts(&model), verdicts(&renamed));
    for (question, expected) in [
        ("PicksOnlyB", Verdict::Holds),
        ("CanPick", Verdict::Reachable),
    ] {
        let report =
            idealis_check::check(&model, question, &Selection::All, Budget::default()).unwrap();
        assert_eq!(report.answers.len(), 3);
        assert!(
            report
                .answers
                .iter()
                .all(|answer| answer.verdict == expected)
        );
    }

    let explicit_shadow = "model M
enum Item = A | B
derive bad = select(item: Item | some item: Item | item = B)
scope steps = 3";
    assert!(
        idealis_model::parse(explicit_shadow)
            .unwrap_err()
            .message
            .contains("shadows another name")
    );
}
