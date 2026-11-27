use rmodel::reference::Outcome::{self, Empty, Holds, Reachable, Unknown, Unreachable, Violated};

const PERMISSIONS: &str = include_str!("../../../fixtures/rmodel/permissions.model");
const SHIPMENT: &str = include_str!("../../../fixtures/rprob/shipment.model");

fn agree(source: &str, budgets: &[usize]) -> rmodel::reference::Report {
    let model = rmodel::parse(source).unwrap();
    let compiled = rprob::Compiled::new(&model).unwrap();
    let mut full = None;
    for &budget in budgets {
        let expected = rmodel::reference::analyze(&model, budget);
        let actual = compiled.analyze(budget);
        assert_eq!(actual.verdicts, expected.verdicts, "budget {budget}");
        assert_eq!(actual.scope, expected.scope, "budget {budget}");
        assert_eq!(actual.stop, expected.stop, "budget {budget}");
        assert_eq!(
            actual.declarations, expected.declarations,
            "budget {budget}"
        );
        assert_eq!(actual.enabled, expected.enabled, "budget {budget}");
        full = Some(actual);
    }
    full.unwrap()
}

fn outcome(report: &rmodel::reference::Report, name: &str) -> Outcome {
    report
        .verdicts
        .iter()
        .find(|verdict| verdict.check == name)
        .unwrap()
        .outcome
}

#[test]
fn key_removal_agrees_on_dense_and_sparse_layouts() {
    let model = r#"model RemoveKeys
sig Padding
enum Key = Old | Keep | Missing
enum Item = Shared | Extra
var rows: Key -> set Item
var expired: set Key
init { rows = (Old -> (Shared + Extra)) + (Keep -> Shared) no expired }
behavior mark(key: Key) { update { expired = expired + key } }
behavior prune() { update { rows = withoutKeys(rows, expired) } }
transition PruneMatchesPairs on prune { after(rows) = before(rows - (expired -> Item)) }
reachable Kept { rows = (Keep -> Shared) }
"#;
    for padding in [1, 4] {
        let source = format!("{model}\nscope Padding = {padding}, steps = 5");
        let report = agree(&source, &[1, 15, 1_000]);
        assert_eq!(outcome(&report, "PruneMatchesPairs"), Holds);
        assert_eq!(outcome(&report, "Kept"), Reachable { depth: 2 });
    }
}

#[test]
fn finite_relations_match_the_reference() {
    for source in [
        PERMISSIONS,
        include_str!("../../../fixtures/rmodel/permissions-open.model"),
        include_str!("../../../fixtures/rmodel/sessions.model"),
        include_str!("../../../fixtures/rmodel/mutex.model"),
        concat!(
            include_str!("../../../fixtures/rcheck/linked/raft/domain.model"),
            "\n",
            include_str!("../../../fixtures/rcheck/linked/raft/election.model"),
        ),
        include_str!("../../../fixtures/rprob/edges.model"),
        "model Graph
        sig Node
        rel links: Node -> set Node
        var copied: Node -> set Node
        init { no copied }
        behavior close() { update { copied = links.links } }
        invariant Consistent { no copied or all n: Node | n.copied = n.links.links }
        reachable TwoHops { some copied }
        scope Node = 2, steps = 4",
        "model Sum
        sig A
        sig B
        type Party = B + A
        rel input: one Party
        var selected: lone Party
        init { no selected }
        behavior copy() { update { selected = input } }
        reachable FoundA { some selected & A }
        scope A = 1, B = 1, steps = 4",
        "model ExplicitSet
        enum Item = Left | Right
        var pending: set Item
        init { pending = Left }
        behavior replace() {
          guard { Left in pending }
          update { pending = union(difference(pending, Left), Right) }
        }
        reachable Replaced { pending = Right }
        scope steps = 3",
        "model Filtered
        enum Item = Left | Right
        var picked: set Item
        init { picked = Item }
        behavior prune() { update { picked = select(item: Item | item in picked and item != Left) } }
        reachable OnlyRight { picked = Right }
        scope steps = 3",
    ] {
        agree(source, &[0, 1, 15, 97, 1_000_000]);
    }
}

#[test]
fn shipment_separates_reachable_differences_from_absence_and_budget() {
    let budgets = [0, 10, 145, 1_000_000];
    let normal = agree(SHIPMENT, &budgets);
    assert_eq!(outcome(&normal, "KeepsTracking"), Holds);
    assert_eq!(outcome(&normal, "ConfirmCourier"), Reachable { depth: 2 });

    let faulty = SHIPMENT.replace(
        "acknowledged = acknowledged + o }",
        "acknowledged = acknowledged + o tracking = (tracking - o -> Carrier) + o -> Post }",
    );
    let report = agree(&faulty, &budgets);
    assert_eq!(outcome(&report, "KeepsTracking"), Violated { depth: 2 });
    assert_eq!(outcome(&report, "ConfirmCourier"), Unreachable);

    let restricted = SHIPMENT.replace(
        "guard { o in shipped }",
        "guard { o in shipped Post in o.tracking }",
    );
    let report = agree(&restricted, &budgets);
    assert_eq!(outcome(&report, "KeepsTracking"), Holds);
    assert_eq!(outcome(&report, "ConfirmCourier"), Unreachable);
    assert_eq!(outcome(&report, "ConfirmPost"), Reachable { depth: 2 });

    let no_initial = SHIPMENT.replace("no shipped no tracking no acknowledged", "false");
    let report = agree(&no_initial, &budgets);
    assert_eq!(report.scope, Empty);
    assert_eq!(outcome(&report, "KeepsTracking"), Empty);
    let report = agree(SHIPMENT, &[1]);
    assert_eq!(report.scope, Unknown);
    assert_eq!(outcome(&report, "ConfirmCourier"), Unknown);

    let broken = SHIPMENT.replace(
        "tracking = tracking + o -> c",
        "tracking = tracking + o -> Carrier",
    );
    let report = agree(&broken, &budgets);
    assert_eq!(report.declarations, Violated { depth: 1 });
    assert_eq!(report.scope, Holds);
    assert_eq!(outcome(&report, "ConfirmPost"), Unreachable);
}

#[test]
fn unsupported_models_do_not_produce_a_verdict() {
    let sequence_model =
        include_str!("../../../examples/model-patterns/node-event-loop/nested-io-batch.model");
    let temporal_model = include_str!("../../../fixtures/rcheck/structured/retry-delivery.model");
    let oversized =
        include_str!("../../../fixtures/rprob/edges.model").replace("Left = 3", "Left = 6");
    for source in [sequence_model, temporal_model, &oversized] {
        let model = rmodel::parse(source).unwrap();
        assert!(rprob::Compiled::new(&model).is_err());
    }
}

#[test]
fn oversized_sequence_with_closed_writes_agrees_with_the_reference() {
    let source =
        include_str!("../../../examples/model-patterns/node-event-loop/nested-io-batch.model");
    let model = rmodel::parse(source).unwrap();
    assert_eq!(
        rprob::analyze_safety(&model, 10).unwrap().report.verdicts,
        rmodel::reference::analyze_actions(&model, 10).verdicts
    );
    let actual = rprob::witness(&model, "ObservedBatch", 10).unwrap();
    let expected = rmodel::reference::witness(&model, "ObservedBatch", 10).unwrap();
    assert_eq!(actual.outcome, Reachable { depth: 1 });
    assert_eq!(actual.outcome, expected.outcome);
    let actual = actual.witness.unwrap();
    let expected = expected.witness.unwrap();
    let path = |commands: Vec<rmodel::reference::TraceCommand>| {
        commands
            .into_iter()
            .map(|command| (command.action, command.args))
            .collect::<Vec<_>>()
    };
    assert_eq!(path(actual.commands), path(expected.commands));
    assert_eq!(actual.final_state, expected.final_state);

    let with_fork = source
        .replace("behavior drainIoBatch()", "behavior drainIoBatch(fast: Bool)")
        .replace(
            "update { calls = Invocations[Microtask, NestedMicrotask, Immediate, Timer] }",
            "update { calls = if { fast } then Invocations[Microtask, NestedMicrotask, Immediate, Timer] else Invocations[Microtask, Timer] }",
        )
        .replace(
            "scope steps",
            "reachable NeverWritten { calls = Invocations[NestedMicrotask] }\nscope steps",
        );
    let model = rmodel::parse(&with_fork).unwrap();
    for budget in [0, 3, 4, 5, 10] {
        let reference = rmodel::reference::analyze_actions(&model, budget);
        let actual = rprob::analyze_safety(&model, budget).unwrap().report;
        assert_eq!(actual.verdicts, reference.verdicts, "budget {budget}");
        assert_eq!(actual.scope, reference.scope, "budget {budget}");
        assert_eq!(actual.stop, reference.stop, "budget {budget}");
    }
    let result = rprob::analyze_safety(&model, 10).unwrap().report;
    assert_eq!(outcome(&result, "NeverWritten"), Unreachable);
}

#[test]
fn a_free_checkpoint_with_a_large_nested_sequence_preserves_its_shortest_trace() {
    let source =
        include_str!("../../../examples/model-patterns/crash-recovery/restored-queue.model");
    let model = rmodel::parse(source).unwrap();
    for budget in [0, 1, 2728, 2729, 3000, 10_000] {
        let actual = rprob::analyze_safety(&model, budget).unwrap().report;
        let reference = rmodel::reference::analyze_actions(&model, budget);
        assert_eq!(actual.verdicts, reference.verdicts, "budget {budget}");
        assert_eq!(actual.stop, reference.stop, "budget {budget}");
    }
    let actual = rprob::witness(&model, "RestoresFullQueue", 10_000).unwrap();
    let expected = rmodel::reference::witness(&model, "RestoresFullQueue", 10_000).unwrap();
    assert_eq!(actual.outcome, Reachable { depth: 1 });
    assert_eq!(actual.outcome, expected.outcome);
    let actual = actual.witness.unwrap();
    let expected = expected.witness.unwrap();
    assert_eq!(actual.initial, expected.initial);
    assert_eq!(actual.final_state, expected.final_state);
    assert_eq!(actual.commands[0].action, "restart");

    let escaped = source.replace(
        "restored = saved }",
        "restored = Checkpoint { pending: Pending[], retries: 8 } }",
    );
    let model = rmodel::parse(&escaped).unwrap();
    let actual = rprob::witness(&model, "Scope", 10_000).unwrap();
    let expected = rmodel::reference::witness(&model, "Scope", 10_000).unwrap();
    assert_eq!(actual.outcome, Violated { depth: 1 });
    assert_eq!(actual.outcome, expected.outcome);
    assert_eq!(
        actual.witness.unwrap().final_state,
        expected.witness.unwrap().final_state,
    );
}

#[test]
fn a_nested_checkpoint_projects_queue_and_retry_budget() {
    let source =
        include_str!("../../../examples/model-patterns/crash-recovery/restart-budget.model");
    let model = rmodel::parse(source).unwrap();
    let actual = rprob::analyze_safety(&model, 10_000).unwrap().report;
    let reference = rmodel::reference::analyze_actions(&model, 10_000);
    assert_eq!(actual.verdicts, reference.verdicts);
    assert_eq!(
        outcome(&actual, "RestartsFullQueue"),
        Reachable { depth: 1 }
    );
    assert_eq!(outcome(&actual, "RestartConsumesRetry"), Holds);
    assert_eq!(outcome(&actual, "RestartKeepsOrder"), Holds);
    assert_eq!(outcome(&actual, "RestartsWithoutBudget"), Unreachable);

    let actual = rprob::witness(&model, "RestartsFullQueue", 10_000).unwrap();
    let reference = rmodel::reference::witness(&model, "RestartsFullQueue", 10_000).unwrap();
    assert_eq!(actual.outcome, reference.outcome);
    assert_eq!(
        actual.witness.unwrap().final_state,
        reference.witness.unwrap().final_state
    );

    let fault = source.replace("no restored, saved.retries > 0", "no restored");
    let model = rmodel::parse(&fault).unwrap();
    let actual = rprob::witness(&model, "Scope", 10_000).unwrap();
    let reference = rmodel::reference::witness(&model, "Scope", 10_000).unwrap();
    assert_eq!(actual.outcome, Violated { depth: 1 });
    assert_eq!(actual.outcome, reference.outcome);
    assert_eq!(
        actual.witness.unwrap().final_state,
        reference.witness.unwrap().final_state
    );
}

#[test]
fn a_single_free_sequence_slot_keeps_distinct_ordinals() {
    let source =
        include_str!("../../../examples/model-patterns/node-event-loop/nested-io-batch.model")
            .replace("init { calls = Invocations[] }", "init { true }");
    let model = rmodel::parse(&source).unwrap();
    for budget in [341, 342, 683, 684] {
        let actual = rprob::analyze_safety(&model, budget).unwrap().report;
        let reference = rmodel::reference::analyze_actions(&model, budget);
        assert_eq!(actual.verdicts, reference.verdicts, "budget {budget}");
        assert_eq!(actual.stop, reference.stop, "budget {budget}");
    }
    let witness = rprob::witness(&model, "ObservedBatch", 684).unwrap();
    assert_eq!(witness.outcome, Reachable { depth: 0 });
}

#[test]
fn batch_journal_recognizes_sets_of_structured_values() {
    let source =
        include_str!("../../../examples/model-patterns/crash-recovery/deduplicate-batch.model");
    let model = rmodel::parse(source).unwrap();
    let actual = rprob::analyze_safety(&model, 10).unwrap().report;
    let expected = rmodel::reference::analyze_actions(&model, 10);
    assert_eq!(actual.verdicts, expected.verdicts);
    assert_eq!(outcome(&actual, "AppliedBoth"), Reachable { depth: 1 });
    assert_eq!(outcome(&actual, "KnownBatchStaysOnce"), Holds);
    assert_eq!(outcome(&actual, "NoDuplicateBatches"), Holds);

    let edited = source.replace(
        "scope Int",
        "invariant SetAlgebra { one (elements(applied) & Batch[First, Second]), no (elements(applied) - (Batch[First, Second] + Batch[First, Third])), Batch[Second, First] not in elements(applied) }\nscope Int",
    );
    let model = rmodel::parse(&edited).unwrap();
    let actual = rprob::analyze_safety(&model, 10).unwrap().report;
    let expected = rmodel::reference::analyze_actions(&model, 10);
    assert_eq!(actual.verdicts, expected.verdicts);
    assert_eq!(outcome(&actual, "SetAlgebra"), Holds);

    let fault = source.replace("batch not in elements(applied) and ", "");
    let model = rmodel::parse(&fault).unwrap();
    let actual = rprob::witness(&model, "NoDuplicateBatches", 10).unwrap();
    let expected = rmodel::reference::witness(&model, "NoDuplicateBatches", 10).unwrap();
    assert_eq!(actual.outcome, Violated { depth: 1 });
    assert_eq!(actual.outcome, expected.outcome);
    assert_eq!(
        actual.witness.unwrap().final_state,
        expected.witness.unwrap().final_state
    );
}

#[test]
fn worker_checkpoints_preserve_other_large_rows() {
    let source =
        include_str!("../../../examples/model-patterns/crash-recovery/worker-checkpoints.ideal");
    let model = rmodel::parse(source).unwrap();
    let actual = rprob::analyze_safety(&model, 100).unwrap().report;
    let expected = rmodel::reference::analyze_actions(&model, 100);
    assert_eq!(actual.verdicts, expected.verdicts);
    assert_eq!(outcome(&actual, "BothSaved"), Reachable { depth: 2 });
    assert_eq!(outcome(&actual, "CheckpointKeepsOtherWorker"), Holds);
    assert_eq!(outcome(&actual, "ExpireKeepsOtherWorker"), Holds);

    let faulty = source.replace("withoutKeys(saved, worker)", "withoutKeys(saved, Worker)");
    let model = rmodel::parse(&faulty).unwrap();
    let actual = rprob::witness(&model, "ExpireKeepsOtherWorker", 100).unwrap();
    let expected = rmodel::reference::witness(&model, "ExpireKeepsOtherWorker", 100).unwrap();
    assert_eq!(actual.outcome, Violated { depth: 3 });
    assert_eq!(actual.outcome, expected.outcome);
    assert_eq!(
        actual.witness.unwrap().final_state,
        expected.witness.unwrap().final_state
    );

    let required = source
        .replace("Worker -> lone Checkpoint", "Worker -> one Checkpoint")
        .replace(
            "init { no saved }",
            "init { saved = Worker -> Checkpoint { pending: Pending[], retries: 0 } }",
        );
    let model = rmodel::parse(&required).unwrap();
    let actual = rprob::witness(&model, "Declarations", 100).unwrap();
    let expected = rmodel::reference::witness(&model, "Declarations", 100).unwrap();
    assert_eq!(actual.outcome, Violated { depth: 1 });
    assert_eq!(actual.outcome, expected.outcome);
    assert_eq!(
        actual.witness.unwrap().final_state,
        expected.witness.unwrap().final_state
    );
}

#[test]
fn indexed_relations_decline_unrestricted_union() {
    let source =
        include_str!("../../../examples/model-patterns/node-event-loop/nested-io-batch.model")
            .replace(
                "scope steps",
        "rel other: Callback -> lone Invocations\nfact { other = (Microtask -> Invocations[]) + (Microtask -> Invocations[Timer]) }\nscope steps",
            );
    let model = rmodel::parse(&source).unwrap();
    assert!(rprob::analyze_safety(&model, 100).is_err());
}

#[test]
fn indexed_queue_rotation_agrees_on_order_and_the_first_scope_exit() {
    let source = include_str!("../../../examples/model-patterns/crash-recovery/rotate-queue.model");
    let model = rmodel::parse(source).unwrap();
    let actual = rprob::witness(&model, "Retried", 10).unwrap();
    let expected = rmodel::reference::witness(&model, "Retried", 10).unwrap();
    assert_eq!(actual.outcome, Reachable { depth: 1 });
    assert_eq!(actual.outcome, expected.outcome);
    assert_eq!(
        actual.witness.unwrap().final_state,
        expected.witness.unwrap().final_state
    );

    let fault = source.replace("append(tail(pending),", "append(pending,");
    let model = rmodel::parse(&fault).unwrap();
    let actual = rprob::witness(&model, "Scope", 10).unwrap();
    let expected = rmodel::reference::witness(&model, "Scope", 10).unwrap();
    assert_eq!(actual.outcome, Violated { depth: 1 });
    assert_eq!(actual.outcome, expected.outcome);
    assert_eq!(
        actual.witness.unwrap().final_state,
        expected.witness.unwrap().final_state
    );
}

#[test]
fn indexed_sequence_edits_preserve_duplicates_and_compare_temporary_values() {
    let source = include_str!("../../../examples/model-patterns/crash-recovery/rotate-queue.model");
    for (edit, result) in [
        ("tail(pending)", "Second, Third, Second"),
        ("take(pending, 2)", "First, Second"),
        ("removeAt(pending, 1)", "First, Third, Second"),
        ("removeAll(pending, Second)", "First, Third"),
    ] {
        let source = source
            .replace("append(tail(pending), at(pending, 0))", edit)
            .replace("Second, Third, Second, First", result)
            .replace(
                "scope Int",
                "invariant TemporaryListsCompare { append(pending, First) = append(pending, First), append(pending, First) != append(pending, Second), Second in elements(append(pending, Second)) }\nscope Int",
            );
        let model = rmodel::parse(&source).unwrap();
        let actual = rprob::analyze_safety(&model, 100).unwrap().report;
        let reference = rmodel::reference::analyze_actions(&model, 100);
        assert_eq!(actual.verdicts, reference.verdicts, "{edit}");
        assert_eq!(outcome(&actual, "Retried"), Reachable { depth: 1 });
        assert_eq!(outcome(&actual, "TemporaryListsCompare"), Holds);
        assert_eq!(actual.scope, Holds);
    }
}

#[test]
fn indexed_sequence_elements_use_value_equality_for_removal_and_indexing() {
    let source = include_str!("../../../examples/model-patterns/crash-recovery/refill-queue.model")
        .replace(
            "History[Batch[Third], Batch[Fourth]]",
            "History[Batch[First], Batch[Second]]",
        )
        .replace(
            "update { pending = history }",
            "update { pending = removeAll(history, elements(history) - Batch[First]) }",
        )
        .replace(
            "after(pending) = history",
            "after(pending) = removeAll(history, elements(history) - Batch[First])",
        )
        .replace(
            "reachable Refilled { pending = History[Batch[First], Batch[Second]] }",
            "reachable Refilled { pending = History[Batch[First]] }",
        )
        .replace(
            "scope steps",
            "transition KeepsFirst on refill { at(history, 0) = Batch[First] }\nscope Int = 0..2, steps",
        );
    let model = rmodel::parse(&source).unwrap();
    let initial = rprob::analyze_safety(&model, 4).unwrap().report;
    let reference = rmodel::reference::analyze_actions(&model, 4);
    assert_eq!(initial.verdicts, reference.verdicts);
    assert_eq!(outcome(&initial, "Refilled"), Reachable { depth: 1 });
    assert_eq!(outcome(&initial, "RefillKeepsOrder"), Holds);
    assert_eq!(outcome(&initial, "KeepsFirst"), Holds);
}

#[test]
fn refill_enumerates_large_arguments_in_reference_order() {
    let source = include_str!("../../../examples/model-patterns/crash-recovery/refill-queue.model");
    let model = rmodel::parse(source).unwrap();
    let actual = rprob::witness(&model, "Refilled", 4).unwrap();
    let expected = rmodel::reference::witness(&model, "Refilled", 4).unwrap();
    assert_eq!(actual.outcome, Reachable { depth: 1 });
    assert_eq!(actual.outcome, expected.outcome);
    assert!(actual.complete);
    let actual = actual.witness.unwrap();
    let expected = expected.witness.unwrap();
    assert_eq!(actual.commands[0].args, expected.commands[0].args);
    assert_eq!(actual.final_state, expected.final_state);

    let broken = source.replace("after(pending) = history", "after(pending) != history");
    let model = rmodel::parse(&broken).unwrap();
    for (arguments, outcome) in [
        (1369, Unknown),
        (1370, Violated { depth: 1 }),
        (116_623, Violated { depth: 1 }),
    ] {
        let budget = rmodel::reference::SearchBudget {
            states: 4,
            arguments,
        };
        let question = "RefillKeepsOrder";
        let actual = rprob::witness_with_budget(&model, question, budget).unwrap();
        let expected = rmodel::reference::witness_with_budget(&model, question, budget).unwrap();
        assert_eq!(actual.outcome, outcome, "arguments {arguments}");
        assert_eq!(actual.outcome, expected.outcome);
        assert_eq!(actual.stop, expected.stop);
        assert_eq!(actual.complete, arguments == 116_623);
        assert_eq!(
            actual.witness.map(|witness| witness.final_state),
            expected.witness.map(|witness| witness.final_state)
        );
    }
}

#[test]
fn budgeted_search_preserves_numeric_and_indexed_arguments_in_one_call() {
    let source = include_str!("../../../examples/model-patterns/crash-recovery/refill-queue.model")
        .replace(
            "refill(history: History)",
            "refill(lane: Nat, history: History, keep: Bool)",
        )
        .replace(
            "guard { no pending history",
            "guard { no pending lane + 1 = 1 keep history",
        )
        .replace("after(pending) = history", "after(pending) != history")
        .replace("scope steps", "scope Int = 0..1, steps");
    let model = rmodel::parse(&source).unwrap();
    let budget = rmodel::reference::SearchBudget {
        states: 4,
        arguments: 2740,
    };
    let question = "RefillKeepsOrder";
    let actual = rprob::witness_with_budget(&model, question, budget).unwrap();
    let expected = rmodel::reference::witness_with_budget(&model, question, budget).unwrap();
    assert_eq!(actual.outcome, Violated { depth: 1 });
    assert_eq!(actual.outcome, expected.outcome);
    assert_eq!(actual.stop, expected.stop);
    let actual = actual.witness.unwrap();
    let expected = expected.witness.unwrap();
    assert_eq!(actual.commands[0].args, expected.commands[0].args);
    assert_eq!(actual.final_state, expected.final_state);
}

#[test]
fn sparse_relations_with_unrelated_atoms_match_the_reference() {
    let ring = include_str!("../../../examples/model-patterns/worker-deque/ring-copy.model");
    let early = ring.replace(
        "update { copy = copy + (field -> field.body) }",
        "update { copy = copy + (field -> field.body) if { field = Header } then { slot = Free } }",
    );
    let normal = agree(ring, &[0, 1, 5, 12, 10_000]);
    assert_eq!(outcome(&normal, "ReaderOwnsSlotWhileCopying"), Holds);
    let faulty = agree(&early, &[0, 1, 5, 12, 10_000]);
    assert_eq!(
        outcome(&faulty, "CopiedWordsBelongToOldTicket"),
        Violated { depth: 4 }
    );
    assert_eq!(
        outcome(&faulty, "ReturnedCopyIsComplete"),
        Violated { depth: 5 }
    );

    let cutover = include_str!("../../../examples/model-patterns/lease/store-cutover.model");
    assert_eq!(agree(cutover, &[0, 30, 10_000]).scope, Holds);

    // Force the sparse representation even for the existing fixtures with
    // joins in both directions, composition, and unbound initial candidates.
    for source in [
        PERMISSIONS,
        include_str!("../../../fixtures/rmodel/permissions-open.model"),
        include_str!("../../../fixtures/rprob/edges.model"),
        "model Graph
        sig Node
        rel links: Node -> set Node
        var copied: Node -> set Node
        init { no copied }
        behavior close() { update { copied = links.links } }
        invariant Consistent { no copied or all n: Node | n.copied = n.links.links }
        reachable TwoHops { some copied }
        scope Node = 2, steps = 4",
    ] {
        let (declaration, rest) = source.split_once('\n').unwrap();
        let padding = "enum Padding = Pad0 | Pad1 | Pad2 | Pad3 | Pad4 | Pad5 | Pad6 | Pad7";
        agree(
            &format!("{declaration}\n{padding}\n{rest}"),
            &[0, 1, 15, 97, 10_000],
        );
    }
}

#[test]
fn sparse_relations_keep_candidate_order_and_enforce_pair_capacity() {
    let lazy = "model Rows
        enum Row = R0 | R1 | R2 | R3 | R4 | R5 | R6 | R7 | R8
        enum Value = Tag
        rel assigned: Row -> lone Value
        var selected: lone Value
        init { no selected }
        behavior select(row: Row) { update { selected = row.assigned } }
        reachable CanSelect { some selected }
        scope steps = 3";
    let report = agree(lazy, &[1, 23, 255, 511, 512, 1000, 2_000]);
    assert_eq!(outcome(&report, "CanSelect"), Reachable { depth: 1 });

    let exact = "model Capacity
        enum Left = L0 | L1 | L2 | L3 | L4 | L5 | L6 | L7
        enum Right = R0 | R1 | R2 | R3 | R4 | R5 | R6 | R7
        rel assigned: Left -> one Right
        scope steps = 1";
    assert!(rprob::Compiled::new(&rmodel::parse(exact).unwrap()).is_ok());
    let overflow = exact.replace("L6 | L7", "L6 | L7 | L8");
    let model = rmodel::parse(&overflow).unwrap();
    let error = rprob::Compiled::new(&model)
        .err()
        .expect("72 ordered pairs");
    assert!(
        error
            .to_string()
            .contains("more than 64 distinct ordered pairs")
    );
}

#[test]
fn cli_accepts_a_model_and_refuses_an_unsupported_one() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let binary = env!("CARGO_BIN_EXE_rprob");
    let output = std::process::Command::new(binary)
        .arg(directory.join("fixtures/rprob/shipment.model"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("KeepsTracking: Holds")
            && text.contains("ConfirmCourier: Reachable { depth: 2 }")
    );

    let output = std::process::Command::new(binary)
        .arg(directory.join("examples/model-patterns/node-event-loop/nested-io-batch.model"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("ObservedBatch: Reachable { depth: 1 }")
    );

    let output = std::process::Command::new(binary)
        .arg(directory.join("fixtures/rcheck/structured/retry-delivery.model"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("unsupported")
    );
}

#[test]
fn versioned_adoption_rejects_old_success_and_accepts_current_evidence() {
    let source = include_str!("../../../spec/models/evidence-adoption.model");
    let report = agree(source, &[0, 75, 100_000]);
    for property in [
        "AdoptedIsCurrent",
        "AdoptionNeedsCurrentReview",
        "AdoptionNeedsCurrentAgreement",
        "AdoptionNeedsCurrentImprovement",
    ] {
        assert_eq!(
            outcome(&report, property),
            Holds,
            "{report_property}",
            report_property = property
        );
    }
    assert_eq!(
        outcome(&report, "OldAgreementArrives"),
        Reachable { depth: 2 }
    );
    assert_eq!(
        outcome(&report, "CurrentEvidenceCanAdopt"),
        Reachable { depth: 5 }
    );
    let mutant = source.replace(
        "current in equivalent current in improved",
        "some equivalent current in improved",
    );
    assert_ne!(mutant, source);
    let report = agree(&mutant, &[100_000]);
    assert_eq!(
        outcome(&report, "AdoptionNeedsCurrentAgreement"),
        Violated { depth: 5 }
    );
}

#[test]
fn adoption_lifetime_depends_on_revision_changes() {
    let source = include_str!("../../../spec/models/evidence-adoption.model");
    let report = agree(source, &[0, 75, 100_000]);
    assert_eq!(outcome(&report, "AdoptionSurvivesSameRevision"), Holds);
    assert_eq!(outcome(&report, "RevisionClearsAdoption"), Holds);

    let dropped = source.replace("lateSemantic = r }", "lateSemantic = r adopted = none }");
    assert_ne!(dropped, source);
    let report = agree(&dropped, &[100_000]);
    assert_eq!(outcome(&report, "AdoptedIsCurrent"), Holds);
    assert_eq!(
        outcome(&report, "AdoptionSurvivesSameRevision"),
        Violated { depth: 6 }
    );

    let retained = source.replace("current = Second adopted = none }", "current = Second }");
    assert_ne!(retained, source);
    assert_eq!(
        outcome(&agree(&retained, &[100_000]), "RevisionClearsAdoption"),
        Violated { depth: 5 }
    );
}

#[test]
fn comparison_requires_complete_agreement_on_current_results() {
    let source = include_str!("../../../spec/models/checker-comparison.model");
    let report = agree(source, &[0, 300_000]);
    assert_eq!(outcome(&report, "PublishedNeedsCurrentResults"), Holds);
    assert_eq!(outcome(&report, "PublishedNeedsAgreement"), Holds);
    assert_eq!(
        outcome(&report, "OneCompleteResult"),
        Reachable { depth: 2 }
    );
    assert_eq!(outcome(&report, "OldResultsOnly"), Reachable { depth: 3 });
    assert_eq!(
        outcome(&report, "CurrentAgreementPublished"),
        Reachable { depth: 4 }
    );
    assert_eq!(outcome(&report, "ReceivedResultStays"), Holds);
    let overwritten = source
        .replace(
            "behavior receiveB(r: Revision, c: Checker) {\n  guard { (r = current or r = First) and c not in r.received }",
            "behavior receiveB(r: Revision, c: Checker) {\n  guard { (r = current or r = First) and (c not in r.received or (no published and c in r.valueA)) }",
        )
        .replace(
            "completed = completed + r -> c }",
            "completed = completed + r -> c valueA = valueA - r -> c }",
        );
    assert_ne!(overwritten, source);
    let overwritten_report = agree(&overwritten, &[300_000]);
    assert_eq!(
        outcome(&overwritten_report, "PublishedNeedsAgreement"),
        Holds
    );
    assert_eq!(
        outcome(&overwritten_report, "ReceivedResultStays"),
        Violated { depth: 2 }
    );
    for forgotten in ["received", "completed"] {
        let discarded = source.replace(
            "current = Second published = none }",
            &format!("current = Second published = none {forgotten} = none }}"),
        );
        assert_ne!(discarded, source);
        assert_eq!(
            outcome(&agree(&discarded, &[300_000]), "ReceivedResultStays"),
            Violated { depth: 2 },
            "discarding {forgotten}"
        );
    }
    let ready = "guard { no published current.completed = Checker and (current.valueA = Checker or no current.valueA) }";
    assert_eq!(source.matches(ready).count(), 1);
    for (weakened, property, depth) in [
        (
            "guard { no published some current.completed and (current.valueA = Checker or no current.valueA) }",
            "PublishedNeedsCurrentResults",
            2,
        ),
        (
            "guard { no published and (current.completed = Checker or First.completed = Checker) and (current.valueA = Checker or no current.valueA) }",
            "PublishedNeedsCurrentResults",
            4,
        ),
        (
            "guard { no published current.completed = Checker }",
            "PublishedNeedsAgreement",
            3,
        ),
    ] {
        let mutant = source.replace(ready, weakened);
        assert_eq!(
            outcome(&agree(&mutant, &[300_000]), property),
            Violated { depth }
        );
    }
}

#[test]
fn goal_improvement_requires_the_pinned_run_stable_samples_and_threshold() {
    let source = include_str!("../../../spec/models/measurement-gate.model");
    let report = agree(source, &[0, 100_000]);
    let policy = "guard { no published confirmed = Prerequisite }";
    assert_eq!(source.matches(policy).count(), 1);
    assert_eq!(
        outcome(&report, "ImprovementPublished"),
        Reachable { depth: 4 }
    );
    assert_eq!(
        outcome(&report, "UnsettledMeasurement"),
        Reachable { depth: 1 }
    );
    assert_eq!(outcome(&report, "PublicationKeepsPrerequisites"), Holds);
    let manufactured = source
        .replace(
            policy,
            "guard { no published and (Prerequisite - ThresholdMet) in confirmed }",
        )
        .replace(
            "update { published = Improvement }",
            "update { published = Improvement confirmed = Prerequisite }",
        );
    assert_ne!(manufactured, source);
    let manufactured_report = agree(&manufactured, &[100_000]);
    for check in [
        "ImprovementNeedsPinnedRun",
        "ImprovementNeedsStableSamples",
        "ImprovementNeedsThreshold",
    ] {
        assert_eq!(outcome(&manufactured_report, check), Holds);
    }
    assert_eq!(
        outcome(&manufactured_report, "PublicationKeepsPrerequisites"),
        Violated { depth: 3 }
    );
    for (omitted, check) in [
        ("PinnedRun", "ImprovementNeedsPinnedRun"),
        ("StableSamples", "ImprovementNeedsStableSamples"),
        ("ThresholdMet", "ImprovementNeedsThreshold"),
    ] {
        assert_eq!(outcome(&report, check), Holds);
        let changed = source.replace(
            policy,
            &format!("guard {{ no published and (Prerequisite - {omitted}) in confirmed }}"),
        );
        assert_eq!(
            outcome(&agree(&changed, &[100_000]), check),
            Violated { depth: 3 }
        );
    }
}

#[test]
fn a_wide_total_function_uses_legal_rows_and_keeps_the_budget_boundary() {
    let source = include_str!("../../../fixtures/rprob/wide-function.model");
    let model = rmodel::parse(source).unwrap();
    let compiled = rprob::Compiled::new(&model).expect("six legal choices per row");
    let candidates = 6usize.pow(6);
    let incomplete = compiled.analyze(candidates + 1);
    assert_eq!(incomplete.scope, Unknown);
    assert_eq!(outcome(&incomplete, "Initialized"), Unknown);
    let complete = compiled.analyze(candidates + 2);
    assert_eq!(complete.scope, Holds);
    assert_eq!(outcome(&complete, "Initialized"), Reachable { depth: 0 });
}

#[test]
fn compact_frontier_keeps_distinct_edges_and_checks_known_destinations() {
    let report = agree(
        "model CompactFrontier
         sig Left
         sig Right
         var links: Left -> set Right
         init { no links }
         behavior add(l: Left, r: Right) {
           guard { r not in l.links }
           update { links = links + l -> r }
         }
         behavior repeat() { guard { some links } update { links = links } }
         transition StrictChange { not unchanged(links) }
         reachable Full { links = Left -> Right }
         scope Left = 2, Right = 3, steps = 8",
        &[0, 1, 65, 66, 68, 128, 129],
    );
    assert_eq!(outcome(&report, "StrictChange"), Violated { depth: 2 });
    assert_eq!(outcome(&report, "Full"), Reachable { depth: 6 });
}

#[test]
fn frontier_modes_match_at_the_budget_boundary() {
    let function = include_str!("../../../fixtures/rprob/function-choice.model");
    assert_eq!(
        outcome(&agree(function, &[257, 258, 1024, 1025]), "Initialized"),
        Reachable { depth: 0 },
    );
    let split =
        include_str!("../../../fixtures/rprob/split-sets.model").replace("Item = 7", "Item = 3");
    assert_eq!(
        outcome(&agree(&split, &[65, 66, 128, 129]), "Full"),
        Reachable { depth: 6 },
    );
}

#[test]
fn cached_guards_require_a_reached_state_to_count_as_enabled() {
    let source = "model Guard
        enum Flag = Only
        rel setting: one Flag
        var selected: lone Flag
        init { selected = none }
        behavior turnOn() { guard { Only in setting } update { selected = Flag } }
        behavior disabled() { guard { setting = none } update { selected = none } }
        reachable On { selected = Flag }
        scope steps = 3";
    let limited = agree(source, &[3]);
    assert_eq!(limited.scope, Unknown);
    assert_eq!(
        limited.enabled,
        [("turnOn".into(), None), ("disabled".into(), None)]
    );
    let complete = agree(source, &[4, 5]);
    assert_eq!(
        complete.enabled,
        [
            ("turnOn".into(), Some(true)),
            ("disabled".into(), Some(false))
        ]
    );
    assert_eq!(outcome(&complete, "On"), Reachable { depth: 1 });
}

#[test]
fn full_updates_read_the_old_state_and_partial_updates_preserve_other_slots() {
    let report = agree(
        "model ParallelUpdates
         enum Token = A | B
         var left: one Token
         var right: one Token
         init { left = A right = B }
         behavior swap() {
           update { left = right right = left }
         }
         behavior restoreLeft() {
           guard { left = B and right = A }
           update { left = A }
         }
         reachable Swapped { left = B and right = A }
         reachable LeftRestored { left = A and right = A }
         invariant NoBothB { not (left = B and right = B) }
         scope steps = 4",
        &[5, 6, 7, 8],
    );
    assert_eq!(outcome(&report, "Swapped"), Reachable { depth: 1 });
    assert_eq!(outcome(&report, "LeftRestored"), Reachable { depth: 2 });
    assert_eq!(outcome(&report, "NoBothB"), Holds);
}

#[test]
fn prepaid_initial_candidates_preserve_empty_and_partial_reports() {
    let source = "model InitialBudget
        enum Token = A | B
        rel setting: lone Token
        var left: set Token
        var right: one Token
        fact { some setting }
        init { left = none right = B setting = A }
        reachable Initialized { left = none and right = B }
        scope steps = 3";
    let budgets = [0, 1, 2, 9, 10, 11, 12, 19, 20, 22, 23, 24];
    let report = agree(source, &budgets);
    assert_eq!(report.scope, Holds);
    assert_eq!(outcome(&report, "Initialized"), Reachable { depth: 0 });
    assert_eq!(
        agree(
            &source.replace("right = B setting", "no right setting"),
            &budgets
        )
        .scope,
        Empty
    );
    assert_eq!(
        agree(
            &source.replace("left = none right", "left in Token right"),
            &budgets
        )
        .scope,
        Holds
    );
    let no_state = "model NoState
        enum Token = A
        reachable Initialized { some Token }
        scope steps = 2";
    assert_eq!(
        outcome(&agree(no_state, &[0, 1, 2, 3]), "Initialized"),
        Reachable { depth: 0 }
    );
}

#[test]
fn safety_charges_only_the_fixed_initial_error_graph() {
    let source = include_str!(
        "../../../examples/model-patterns/node-event-loop/suppressed-error-graph.model"
    )
    .replace("var root: lone Wrapper", "var root: lone ErrorNode")
    .replace("Wrapper -> lone ErrorNode", "ErrorNode -> lone ErrorNode");
    for (label, source, end) in [
        ("fixed", source.clone(), Holds),
        (
            "unbound flag",
            source
                .replace("init {", "var flag: one Bool\ninit {")
                .replace("scope steps", "reachable EnabledFlag { flag }\nscope steps"),
            Holds,
        ),
        (
            "inconsistent",
            source.replace("init {", "init { false,"),
            Empty,
        ),
    ] {
        let model = rmodel::parse(&source).unwrap();
        for budget in [0, 1, 2, 3, 4, 5, 10] {
            let expected = rmodel::reference::analyze_actions(&model, budget);
            let actual = rprob::analyze_safety(&model, budget).unwrap().report;
            assert_eq!(
                actual.verdicts, expected.verdicts,
                "{label}, budget {budget}"
            );
            assert_eq!(actual.scope, expected.scope, "{label}, budget {budget}");
            assert_eq!(actual.stop, expected.stop, "{label}, budget {budget}");
            assert_eq!(
                actual.declarations, expected.declarations,
                "{label}, budget {budget}"
            );
        }
        let report = rprob::analyze_safety(&model, 10).unwrap().report;
        assert_eq!(outcome(&report, "ReusedErrorRemainsOneObject"), end);
        let witness = rprob::witness(&model, "InitialErrorScope", 10).unwrap();
        let expected = rmodel::reference::witness(&model, "InitialErrorScope", 10).unwrap();
        assert_eq!(witness.outcome, expected.outcome, "{label}");
        let trace_state = |result: rmodel::reference::WitnessResult| {
            result.witness.map(|trace| {
                assert!(trace.commands.is_empty());
                (trace.initial, trace.final_state)
            })
        };
        assert_eq!(trace_state(witness), trace_state(expected), "{label}");
    }
}

#[test]
fn safety_charges_only_configurations_allowed_by_constant_bindings() {
    let source = include_str!(
        "../../../examples/model-patterns/node-event-loop/suppressed-error-graph.model"
    )
    .replace(
        "var root:",
        "rel labels: ErrorNode -> lone ErrorNode\nrel peers: ErrorNode -> lone ErrorNode\nvar root:",
    )
    .replace(
        "init {",
        "fact { labels = (Original + Clone) -> Recent\nno peers }\ninit {",
    )
    .replace(
        "(Original -> Recent) + (thrown -> Recent)",
        "(labels & ((Original + thrown) -> ErrorNode))",
    );
    for (label, source, expected) in [
        ("fixed", source.clone(), Holds),
        (
            "unbound flag",
            source.replace("var root:", "rel feature: one Bool\nvar root:"),
            Holds,
        ),
        (
            "inconsistent",
            source.replace("no peers }", "no peers, peers = Wrapped -> Earlier }"),
            Empty,
        ),
    ] {
        let model = rmodel::parse(&source).unwrap();
        for budget in [0, 1, 2, 3, 4, 5, 10] {
            let reference = rmodel::reference::analyze_actions(&model, budget);
            let actual = rprob::analyze_safety(&model, budget).unwrap().report;
            assert_eq!(
                actual.verdicts, reference.verdicts,
                "{label}, budget {budget}"
            );
            assert_eq!(actual.scope, reference.scope, "{label}, budget {budget}");
            assert_eq!(actual.stop, reference.stop, "{label}, budget {budget}");
        }
        let report = rprob::analyze_safety(&model, 10).unwrap().report;
        assert_eq!(outcome(&report, "ReusedErrorRemainsOneObject"), expected);
        let witness = rprob::witness(&model, "InitialErrorScope", 10).unwrap();
        let reference = rmodel::reference::witness(&model, "InitialErrorScope", 10).unwrap();
        assert_eq!(witness.outcome, reference.outcome, "{label}");
        let configuration = |result: rmodel::reference::WitnessResult| {
            result.witness.map(|trace| {
                trace
                    .config
                    .into_iter()
                    .map(|(name, mut tuples)| {
                        tuples.sort();
                        (name, tuples)
                    })
                    .collect::<std::collections::BTreeMap<_, _>>()
            })
        };
        assert_eq!(configuration(witness), configuration(reference), "{label}");
    }
}

#[test]
fn an_overflowing_candidate_product_only_blocks_accepted_configurations() {
    let source = include_str!("../../../fixtures/rprob/candidates-large.model");
    let run = |source: &str| {
        rprob::Compiled::new(&rmodel::parse(source).unwrap())
            .unwrap()
            .analyze(usize::MAX)
    };
    assert_eq!(run(source).scope, Unknown);
    assert_eq!(
        run(&source.replace("init {", "fact { false }\ninit {")).scope,
        Empty
    );
    let model = rmodel::parse(source).unwrap();
    let report = rprob::analyze_safety(&model, 3).unwrap().report;
    assert_eq!(report.scope, Holds);
    assert_eq!(outcome(&report, "Initialized"), Reachable { depth: 0 });
}

#[test]
fn grouped_bit_updates_keep_the_original_action_order() {
    let source = "model ToggleOrder
        sig Row
        enum A = X | Y
        enum B = Z
        type Column = A + B
        var links: Row -> set Column
        init { no links }
        behavior add(r: Row, c: Column) {
          guard { c not in r.links }
          update { links = links + r -> c }
        }
        behavior remove(r: Row, c: Column) {
          guard { c in r.links }
          update { links = links - r -> c }
        }
        transition KeepsX { Row -> X in before(links) implies Row -> X in after(links) }
        reachable HasX { Row -> X in links }
        reachable HasZ { Row -> Z in links }
        scope Row = 1, steps = 4";
    for model in [source.to_owned(), source.replace("A + B", "B + A")] {
        let report = agree(&model, &[0, 8, 9, 10, 11, 12, 13, 17]);
        assert_eq!(report.scope, Holds);
        assert_eq!(outcome(&report, "KeepsX"), Violated { depth: 2 });
    }
}

#[test]
fn an_empty_product_can_be_used_at_either_arity() {
    let report = agree(
        "model EmptyProduct
         sig A
         invariant Kept { A in ((none -> A) + A) }
         invariant RightEmpty { A in ((A -> none) + A) }
         scope A = 1, steps = 2",
        &[3],
    );
    assert_eq!(outcome(&report, "Kept"), Holds);
    assert_eq!(outcome(&report, "RightEmpty"), Holds);
}

#[test]
fn cardinality_can_select_the_before_and_after_phase() {
    let report = agree(
        "model PhaseCounts
         sig Item
         var selected: set Item
         init { no selected }
         behavior choose() { guard { no selected } update { selected = Item } }
         transition Growth { after(#selected) > before(#selected) }
         transition GrowthOfSet { #after(selected) > #before(selected) }
         scope Item = 1, steps = 3",
        &[5, 6],
    );
    assert_eq!(outcome(&report, "Growth"), Holds);
    assert_eq!(outcome(&report, "GrowthOfSet"), Holds);
}

#[test]
fn action_parameter_depth_has_a_checked_limit() {
    let source = |count| {
        let args = (0..count)
            .map(|index| format!("p{index}: Item"))
            .collect::<Vec<_>>();
        format!(
            "model ManyParameters\n sig Item\n action pick({}) {{ true }}\n scope Item = 1, steps = 2",
            args.join(", ")
        )
    };
    let accepted = rmodel::parse(&source(128)).unwrap();
    assert!(rprob::Compiled::new(&accepted).is_ok());
    let oversized = rmodel::parse(&source(129)).unwrap();
    let Err(error) = rprob::Compiled::new(&oversized) else {
        panic!("expected an unsupported parameter list");
    };
    assert!(error.to_string().contains("128 parameters"));
}

#[test]
fn unchanged_cardinality_compares_counts_instead_of_set_members() {
    let source = "model KeepCount
        enum Item = A | B
        var members: set Item
        init { members = A }
        behavior swap() {
          guard { members = A }
          update { members = B }
        }
        behavior add() {
          guard { members = B }
          update { members = Item }
        }
        transition SwapKeepsCount {
          before(members) = A implies unchanged(#members)
        }
        transition NeverGrows { unchanged(#members) }
        transition NeverGrowsExplicit { before(#members) = after(#members) }
        transition NeverGrowsByInequality { not (before(#members) != after(#members)) }
        transition NeverGrowsByMembership { before(#members) in after(#members) }
        transition NeverGrowsByNonmembership { not (before(#members) not in after(#members)) }
        scope steps = 3";
    let report = agree(source, &[0, 5, 6, 8]);
    assert_eq!(outcome(&report, "SwapKeepsCount"), Holds);
    for check in [
        "NeverGrows",
        "NeverGrowsExplicit",
        "NeverGrowsByInequality",
        "NeverGrowsByMembership",
        "NeverGrowsByNonmembership",
    ] {
        assert_eq!(outcome(&report, check), Violated { depth: 2 });
    }
}

#[test]
fn conditional_actions_compare_both_terminal_races() {
    for (source, check, expected_depth, chosen, wrong) in [
        (
            include_str!("../../../examples/model-patterns/latest-request/search.model"),
            "CurrentResultStays",
            4,
            "if { request = latest } then request else shown",
            "request",
        ),
        (
            include_str!("../../../examples/model-patterns/cancellation/request.model"),
            "TerminalOutcomeStays",
            2,
            "if { no outcome } then Done else outcome",
            "Done",
        ),
    ] {
        assert_eq!(outcome(&agree(source, &[0, 10_000]), check), Holds);
        assert_eq!(source.matches(chosen).count(), 1);
        let faulty = source.replace(chosen, wrong);
        assert_eq!(
            outcome(&agree(&faulty, &[10_000]), check),
            Violated {
                depth: expected_depth
            }
        );
    }
}

#[test]
fn conditionals_in_after_use_poststate_counts() {
    let source = "model ConditionalCount
        enum Item = A | B
        var selected: set Item
        init { no selected }
        behavior add() { guard { no selected } update { selected = A } }
        transition UsesAfter { after(if { #selected > 0 } then A else B) = A }
        scope steps = 3";
    assert_eq!(outcome(&agree(source, &[100]), "UsesAfter"), Holds);
}

#[test]
fn ordered_and_reversed_locks_agree_with_the_reference() {
    use rmodel::reference::Outcome::{Reachable, Unreachable};
    let model = include_str!("../../../examples/model-patterns/worker-locks/locks.model");
    for (order, expected) in [
        (
            include_str!("../../../examples/model-patterns/worker-locks/ordered.model"),
            Unreachable,
        ),
        (
            include_str!("../../../examples/model-patterns/worker-locks/reversed.model"),
            Reachable { depth: 2 },
        ),
    ] {
        let report = agree(&format!("{model}\n{order}"), &[0, 20_000]);
        assert_eq!(outcome(&report, "Deadlocked"), expected);
        assert_eq!(outcome(&report, "BothFinished"), Reachable { depth: 6 });
    }
}

#[test]
fn scoped_transition_uses_the_action_instance_including_noop_retries() {
    use rmodel::reference::Outcome::{Holds, Violated};
    let source = include_str!("../../../examples/model-patterns/retry/delivery.model");
    let report = agree(source, &[0, 20_000]);
    assert_eq!(outcome(&report, "RetryPreservesEffect"), Holds);
    assert_eq!(outcome(&report, "DeliveryReturnsReceipt"), Holds);
    let fault = source.replace(
        "then effects else",
        "then (effects - request -> Effect) + request -> Duplicate else",
    );
    let report = agree(&fault, &[20_000]);
    assert_eq!(
        outcome(&report, "RetryPreservesEffect"),
        Violated { depth: 2 }
    );
    // An intentionally bad assertion on a legal idempotent, already-replied call.
    let noop = source.replace(
        "request in before(receipts) implies unchanged(request.effects)",
        "request in before(replies) implies not unchanged(replies)",
    );
    assert_eq!(
        outcome(&agree(&noop, &[20_000]), "RetryPreservesEffect"),
        Violated { depth: 2 }
    );
}

#[test]
fn scoped_transition_retains_arguments_on_the_single_word_path() {
    let model = "model EnableFlag
enum Flag = A | B
var active: set Flag
init { no active }
behavior enable(flag: Flag) {
  guard { flag not in active }
  update { active = active + flag }
}
transition OnlyA on enable { flag = A }
scope steps = 3";
    let report = agree(model, &[0, 20_000]);
    assert_eq!(
        outcome(&report, "OnlyA"),
        rmodel::reference::Outcome::Violated { depth: 1 }
    );
}

#[test]
fn safety_mode_keeps_the_crash_model_and_names_the_omitted_temporal_checks() {
    use rmodel::reference::Outcome::{Holds, Reachable, Violated};
    let source = include_str!("../../../examples/model-patterns/crash-recovery/transaction.model");
    let fault = source.replace("phase = Prepared }", "phase = Prepared ledger = Posted }");
    for (source, expected) in [(source, Holds), (fault.as_str(), Violated { depth: 4 })] {
        let model = rmodel::parse(source).unwrap();
        assert!(rprob::Compiled::new(&model).is_err());
        let analysis = rprob::analyze_safety(&model, 20_000).unwrap();
        assert_eq!(
            analysis.omitted_temporal,
            ["RecoversWhenScheduled", "RecoveryWithoutScheduling"]
        );
        let reference = rmodel::reference::analyze(&model, 20_000);
        assert_eq!(analysis.report.verdicts, reference.verdicts);
        assert_eq!(analysis.report.scope, reference.scope);
        assert_eq!(analysis.report.declarations, reference.declarations);
        assert_eq!(analysis.report.enabled, reference.enabled);
        assert_eq!(outcome(&analysis.report, "RestartIsAtomic"), expected);
        assert_eq!(
            outcome(&analysis.report, "StoppedAfterCommit"),
            Reachable { depth: 4 }
        );

        let budget_exhausted = rprob::analyze_safety(&model, 0).unwrap();
        assert_eq!(
            budget_exhausted.report.scope,
            rmodel::reference::Outcome::Unknown
        );
        assert_eq!(budget_exhausted.omitted_temporal, analysis.omitted_temporal);
    }
}

#[test]
fn cli_safety_mode_reports_which_questions_are_outside_its_scope() {
    use std::process::Command;
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/model-patterns/crash-recovery/transaction.model",
    );
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_rprob"))
            .args(args)
            .output()
            .unwrap()
    };
    assert_eq!(run(&[path]).status.code(), Some(2));
    let output = run(&["--safety", path, "--max-states", "20000"]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("RestartIsAtomic: Holds"));
    assert!(stdout.contains("RecoversWhenScheduled: unsupported (temporal)"));
    assert!(stdout.contains("RecoveryWithoutScheduling: unsupported (temporal)"));
}

#[test]
fn outbox_faults_distinguish_lost_and_repeated_effects() {
    let source = include_str!("../../../examples/model-patterns/outbox/delivery.model");
    for (source, recoverable, unique) in [
        (source.to_owned(), Holds, Holds),
        (
            source.replace(" pending = pending + request", ""),
            Violated { depth: 1 },
            Holds,
        ),
        (
            source.replace(
                "delivered = delivered + request sent = sent + request",
                "delivered = delivered + request sent = sent + request
                 duplicated = if { request in delivered } then duplicated + request else duplicated",
            ),
            Holds,
            Violated { depth: 5 },
        ),
    ] {
        let model = rmodel::parse(&source).unwrap();
        let safety = rprob::analyze_safety(&model, 20_000).unwrap();
        let reference = rmodel::reference::analyze(&model, 20_000);
        assert_eq!(safety.report.verdicts, reference.verdicts);
        assert_eq!(outcome(&safety.report, "CommittedIsRecoverable"), recoverable);
        assert_eq!(outcome(&safety.report, "AtMostOneEffect"), unique);
        assert_eq!(safety.omitted_temporal, ["QueuedEventuallyDelivered"]);
    }
}

#[test]
fn relational_override_keeps_other_keys_and_multi_valued_rows() {
    let source = include_str!("../../../fixtures/rmodel/relations/override.model");
    let report = agree(source, &[0, 20_000]);
    assert_eq!(outcome(&report, "OtherTabKept"), Holds);
    assert_eq!(outcome(&report, "LeftReplaced"), Reachable { depth: 1 });
    assert_eq!(outcome(&report, "EmptyPatch"), Holds);
}

#[test]
fn cache_coalesces_loads_and_rejects_an_obsolete_completion() {
    let source = include_str!("../../../examples/model-patterns/cache-refresh/cache.model");
    let report = agree(source, &[0, 250_000]);
    for check in ["OneLoadPerGeneration", "OldCompletionKeepsCurrent"] {
        assert_eq!(outcome(&report, check), Holds);
    }
    assert_eq!(outcome(&report, "SharedLoad"), Reachable { depth: 2 });
    assert_eq!(
        outcome(&report, "CurrentReadyWhileOldWaits"),
        Reachable { depth: 4 }
    );
    let separate = source.replace(
        "then starts ++ generation -> Once else starts",
        "then starts ++ generation -> Once else
           if { generation in inflight } then starts ++ generation -> Repeated else starts",
    );
    let late = source.replace(
        "cache = if { completed = generation } then completed else cache",
        "cache = completed",
    );
    assert_eq!(
        outcome(&agree(&separate, &[250_000]), "OneLoadPerGeneration"),
        Violated { depth: 2 }
    );
    assert_eq!(
        outcome(&agree(&late, &[250_000]), "OldCompletionKeepsCurrent"),
        Violated { depth: 3 }
    );
}

#[test]
fn notification_dispatch_preserves_registration_identity() {
    let source = include_str!("../../../examples/model-patterns/subscriptions/notification.model");
    let report = agree(source, &[0, 60_000]);
    assert_eq!(outcome(&report, "DispatchUsesSameRegistration"), Holds);
    assert_eq!(
        outcome(&report, "ReplacedWhileQueued"),
        Reachable { depth: 5 }
    );
    assert_eq!(
        outcome(&report, "RightJoinsAfterAnnouncement"),
        Reachable { depth: 3 }
    );

    let reroute = source.replace("ticket & Subscriber.active", "ticket.owners.active");
    let broaden = source.replace(
        "used = used + ticket }",
        "used = used + ticket pending = if { started } then pending + ticket else pending }",
    );
    assert_eq!(
        outcome(&agree(&reroute, &[60_000]), "DispatchUsesSameRegistration"),
        Violated { depth: 5 }
    );
    assert_eq!(
        outcome(
            &agree(&broaden, &[60_000]),
            "NewRegistrationDoesNotJoinOldAnnouncement"
        ),
        Violated { depth: 3 }
    );
}

#[test]
fn an_upload_keeps_publication_and_delivery_independently_current() {
    let source = include_str!("../../../examples/model-patterns/upload/upload.model");
    let report = agree(source, &[0, 60_000]);
    assert_eq!(outcome(&report, "NeverPublishAnOlderUpload"), Holds);
    assert_eq!(outcome(&report, "NeverShowAnOlderUpload"), Holds);
    assert_eq!(outcome(&report, "ServerKnowsNewestAccepted"), Holds);
    assert_eq!(
        outcome(&report, "NewVisibleWhileOldInTransit"),
        Reachable { depth: 5 }
    );
    assert_eq!(
        outcome(&report, "NewVisibleWhileOldWriting"),
        Reachable { depth: 6 }
    );
    assert_eq!(
        outcome(&report, "NewVisibleWhileOldReplyPending"),
        Reachable { depth: 7 }
    );

    let stale_publish = source.replace(
        "published = if { request = serverLatest } then request else published",
        "published = request",
    );
    let stale_reply = source.replace(
        "shown = if { request = latest } then request else shown",
        "shown = request",
    );
    let published = agree(&stale_publish, &[60_000]);
    let replied = agree(&stale_reply, &[60_000]);
    assert_eq!(
        outcome(&published, "NeverPublishAnOlderUpload"),
        Violated { depth: 5 }
    );
    assert_eq!(outcome(&published, "NeverShowAnOlderUpload"), Holds);
    assert_eq!(outcome(&replied, "NeverPublishAnOlderUpload"), Holds);
    assert_eq!(
        outcome(&replied, "NeverShowAnOlderUpload"),
        Violated { depth: 5 }
    );
    let arrival_order = source.replace(
        "serverLatest = if { no (request.newer & serverLatest) } then request else serverLatest",
        "serverLatest = request",
    );
    let arrived = agree(&arrival_order, &[60_000]);
    assert_eq!(
        outcome(&arrived, "ServerKnowsNewestAccepted"),
        Violated { depth: 4 }
    );
    assert_eq!(
        outcome(&arrived, "NeverPublishAnOlderUpload"),
        Violated { depth: 5 }
    );
    assert_eq!(outcome(&arrived, "NeverShowAnOlderUpload"), Holds);
}

#[test]
fn refund_safety_distinguishes_duplicate_effects_and_premature_confirmation() {
    let source = include_str!("../../../examples/model-patterns/compensation/saga.model");
    let duplicate = source.replace("then Refunded else effect", "then Refunded else Repeated");
    let early = source.replace(
        "update { replyPending = false }",
        "update { stage = Cancelled replyPending = false }",
    );
    for (source, duplicate, lost) in [
        (source, Holds, Holds),
        (&duplicate, Violated { depth: 5 }, Holds),
        (&early, Holds, Violated { depth: 4 }),
    ] {
        let model = rmodel::parse(source).unwrap();
        let safety = rprob::analyze_safety(&model, 20_000).unwrap();
        let reference = rmodel::reference::analyze(&model, 20_000);
        assert_eq!(safety.report.verdicts, reference.verdicts);
        assert_eq!(safety.report.scope, reference.scope);
        assert_eq!(safety.report.enabled, reference.enabled);
        assert_eq!(outcome(&safety.report, "NoDoubleRefund"), duplicate);
        assert_eq!(outcome(&safety.report, "RetryPreservesRefund"), duplicate);
        assert_eq!(outcome(&safety.report, "LostReplyDoesNotConfirm"), lost);
        assert_eq!(safety.omitted_temporal, ["RefundEventuallyConfirmed"]);
    }
}

#[test]
fn reissued_lease_rejects_a_stale_release_and_a_stale_write() {
    let source = include_str!("../../../examples/model-patterns/lease/fence.model");
    let report = agree(source, &[0, 20_000]);
    for check in [
        "StaleReleaseKeepsLease",
        "StaleWriteKeepsData",
        "CurrentWritePublishes",
    ] {
        assert_eq!(outcome(&report, check), Holds);
    }
    assert_eq!(
        outcome(&report, "ReissuedBeforeFirstWrite"),
        Reachable { depth: 3 }
    );
    let owner_only = source.replace("if { generation = active } then", "if { true } then");
    let lazy = source.replace("    fence = generation\n  }\n}", "  }\n}");
    assert_eq!(
        outcome(&agree(&owner_only, &[20_000]), "StaleReleaseKeepsLease"),
        Violated { depth: 4 }
    );
    assert_eq!(
        outcome(&agree(&lazy, &[20_000]), "StaleWriteKeepsData"),
        Violated { depth: 4 }
    );
}

#[test]
fn split_lease_recovery_activates_only_after_the_sink_fence() {
    let source = include_str!("../../../examples/model-patterns/lease/split-fence.model");
    let normal = agree(source, &[0, 20_000]);
    for check in [
        "ExposedLeaseHasFence",
        "ConfirmedFenceIsDurable",
        "OldWriteCannotChangeNewLease",
        "RetryInstallKeepsData",
    ] {
        assert_eq!(outcome(&normal, check), Holds);
    }
    assert_eq!(
        outcome(&normal, "StoppedBeforeInstall"),
        Reachable { depth: 2 }
    );
    assert_eq!(
        outcome(&normal, "StoppedAfterInstall"),
        Reachable { depth: 3 }
    );
    let fault = source.replace(
        "update { online = true }",
        "update { online = true active = prepared prepared = none }",
    );
    let report = agree(&fault, &[20_000]);
    assert_eq!(
        outcome(&report, "ExposedLeaseHasFence"),
        Violated { depth: 3 }
    );
    assert_eq!(
        outcome(&report, "OldWriteCannotChangeNewLease"),
        Violated { depth: 4 }
    );
}

#[test]
fn reachable_from_matches_the_reference_on_dense_and_sparse_edges() {
    let body = "enum Node = Root | Middle | Last | Isolated
var edges: Node -> lone Node
init { edges = (Root -> Middle) + (Middle -> Last) }
behavior closeCycle() { update { edges = edges + (Last -> Root) } }
invariant Walkable { reachableFrom(Root, edges) = Root + Middle + Last }
invariant EmptyRoots { no reachableFrom(none, edges) }
invariant EmptyEdges { reachableFrom(Root + Isolated, none) = Root + Isolated }
transition CycleKeepsReachability on closeCycle { unchanged(reachableFrom(Root, edges)) }
scope steps = 3";
    for padding in ["", "enum Padding = P1 | P2 | P3 | P4 | P5\n"] {
        let source = format!("model Reach\n{padding}{body}");
        let report = agree(&source, &[0, 1, 625, 10_000]);
        assert_eq!(report.scope, Holds);
        assert!(report.verdicts.iter().all(|check| check.outcome == Holds));
    }
}

#[test]
fn relation_keys_project_used_rows_in_dense_and_sparse_layouts() {
    let source = include_str!("../../../fixtures/rmodel/relations/keys.model");
    let sparse = source.replace("scope steps = 3", "sig Spare scope Spare = 6, steps = 3");
    for model in [source, sparse.as_str()] {
        let report = agree(model, &[0, 90, 1_000]);
        assert_eq!(outcome(&report, "ProjectsDistinctKeys"), Holds);
        assert_eq!(outcome(&report, "TypedEmptyRelation"), Holds);
        assert_eq!(outcome(&report, "FirstIsActive"), Reachable { depth: 1 });
        assert_eq!(outcome(&report, "SnapshotProjectsKeys"), Holds);
    }
}

#[test]
fn transitive_closure_matches_reference_on_dense_and_sparse_edges() {
    let source = include_str!("../../../fixtures/rmodel/relations/closure.model");
    let sparse = source.replacen(
        "enum Node",
        "enum Padding = P1 | P2 | P3 | P4 | P5\nenum Node",
        1,
    );
    for model in [source, sparse.as_str()] {
        let report = agree(model, &[1, 100_000]);
        assert_eq!(outcome(&report, "EveryRowAgrees"), Holds);
        assert_eq!(outcome(&report, "EmptyStaysEmpty"), Holds);
        assert_eq!(outcome(&report, "LongPath"), Reachable { depth: 0 });
        assert_eq!(
            outcome(&report, "CycleReturnsToStart"),
            Reachable { depth: 1 }
        );
    }
}

#[test]
fn counts_ordered_pairs_in_dense_and_sparse_relations() {
    let source = include_str!("../../../fixtures/rmodel/relations/count-pairs.model");
    let sparse = source.replace("scope steps = 3", "sig Spare scope Spare = 5, steps = 3");
    for source in [source, &sparse] {
        let report = agree(source, &[1, 10_000]);
        assert_eq!(outcome(&report, "NamedPairs"), Holds);
        assert_eq!(outcome(&report, "ProductCountsPairs"), Holds);
        assert_eq!(outcome(&report, "IntersectedPairs"), Holds);
        assert_eq!(outcome(&report, "FourthPair"), Reachable { depth: 1 });
        assert_eq!(outcome(&report, "CopyAddsOne"), Holds);
    }
}

#[test]
fn finite_integers_check_compacted_feed_and_keep_scope_exits_distinct() {
    let feed = include_str!("../../../examples/model-patterns/lease/feed-compaction.model");
    let signed = "model Signed
var integer: one Int
var natural: one Nat
init { integer = -1, natural = 0 }
behavior within(value: Int) {
  guard { integer = -1, value = -2 }
  update { integer = integer * 2 }
}
behavior escape() {
  guard { integer = -2 }
  update { natural = integer }
}
behavior exceed() {
  guard { integer = -2 }
  update { integer = integer - 3 }
}
invariant Nonnegative { natural >= 0 }
invariant IntermediateOutsideScope { all x: Int | x + 4 > x }
reachable ReachedNegative { integer = -2 }
scope Int = -2..2, steps = 5";
    let broken = feed.replace(
        "replicaTotal = snapshotTotal pending",
        "replicaTotal = snapshotTotal - 1 pending",
    );
    assert_ne!(broken, feed);
    for (source, budget) in [(feed, 20_000), (broken.as_str(), 20_000), (signed, 100)] {
        let model = rmodel::parse(source).unwrap();
        let actual = rprob::Compiled::new(&model).unwrap().analyze(budget);
        let expected = rmodel::reference::analyze_actions(&model, budget);
        assert_eq!(actual.verdicts, expected.verdicts);
        assert_eq!(actual.scope, expected.scope);
        assert_eq!(actual.declarations, expected.declarations);
        assert_eq!(actual.enabled, expected.enabled);
        assert_eq!(actual.stop, expected.stop);
    }
    for (source, question, outcome) in [
        (feed, "NeedsInitialSnapshot", Reachable { depth: 3 }),
        (
            broken.as_str(),
            "ReplicaValueMatchesPrefix",
            Violated { depth: 5 },
        ),
        (signed, "ReachedNegative", Reachable { depth: 1 }),
    ] {
        let model = rmodel::parse(source).unwrap();
        let expected = rmodel::reference::witness(&model, question, 20_000).unwrap();
        let actual = rprob::witness(&model, question, 20_000).unwrap();
        assert_eq!(actual.outcome, outcome);
        assert_eq!(actual.complete, expected.complete);
        let actual = actual.witness.unwrap();
        let expected = expected.witness.unwrap();
        assert_eq!(actual.final_state, expected.final_state);
        assert_eq!(actual.initial, expected.initial);
        assert_eq!(actual.config, expected.config);
        assert_eq!(actual.commands.len(), expected.commands.len());
        for (actual, expected) in actual.commands.iter().zip(expected.commands) {
            assert_eq!(
                (&actual.action, &actual.args),
                (&expected.action, &expected.args)
            );
        }
    }
}

#[test]
fn integer_rows_preserve_subscription_cursors_and_distinguish_invalid_updates() {
    let retention =
        include_str!("../../../examples/model-patterns/stream-terminal/retention.model");
    let broken = retention.replace(
        "all client: Client | client in protected implies client.cursors >= through",
        "some client: Client | client in protected and client.cursors >= through",
    );
    assert_ne!(broken, retention);
    let signed = "model Rows
enum Key = First | Second
var cursors: Key -> one Nat
var pending: Key -> lone Int
init { cursors = Key -> 0, no pending }
behavior capture(key: Key) {
  guard { no key.pending, key = First }
  update { pending = pending ++ key -> (key.cursors - 1) }
}
behavior advance(key: Key) {
  guard { some key.pending }
  update { cursors = cursors ++ key -> (key.pending + 2) }
}
behavior clear(key: Key) {
  guard { some key.pending }
  update { cursors = withoutKeys(cursors, key) }
}
behavior escape(key: Key) {
  guard { some key.pending }
  update { cursors = cursors ++ key -> (key.pending - 2) }
}
invariant OptionalMissingIsNotZero { Second.pending != First.cursors - First.cursors }
invariant DistinctRowsProjectToDistinctValues {
  First.cursors != Second.cursors implies Key.cursors != (Second.cursors + 1)
}
reachable Advanced { First.cursors = 1 and Second.cursors = 0 }
scope Int = -1..1, steps = 5";
    let sparse = signed
        .replace("model Rows", "model Rows\nsig Padding")
        .replace("scope Int", "scope Padding = 4, Int");
    let negative = "model NegativeColumn
enum Key = First | Second
var cursors: Key -> one Nat
init { cursors = Key -> 0 }
behavior decrement(key: Key) {
  update { cursors = cursors ++ key -> (key.cursors - 1) }
}
scope Int = -1..1, steps = 3";
    for source in [
        retention,
        broken.as_str(),
        signed,
        sparse.as_str(),
        negative,
    ] {
        let model = rmodel::parse(source).unwrap();
        let reference = rmodel::reference::analyze_actions(&model, 20_000);
        let actual = rprob::Compiled::new(&model).unwrap().analyze(20_000);
        assert_eq!(actual.verdicts, reference.verdicts);
        assert_eq!(actual.scope, reference.scope);
        assert_eq!(actual.declarations, reference.declarations);
        assert_eq!(actual.stop, reference.stop);
        assert_eq!(actual.enabled, reference.enabled);
        if source == negative {
            assert_eq!(actual.scope, Violated { depth: 1 });
            assert_eq!(actual.declarations, Unknown);
        }
    }
    let model = rmodel::parse(&broken).unwrap();
    let question = "ProtectedClientsKeepTheirPrefix";
    let expected = rmodel::reference::witness(&model, question, 20_000).unwrap();
    let actual = rprob::witness(&model, question, 20_000).unwrap();
    assert!(actual.complete);
    assert_eq!(actual.outcome, Violated { depth: 2 });
    assert_eq!(actual.outcome, expected.outcome);
    let actual = actual.witness.unwrap();
    let expected = expected.witness.unwrap();
    assert_eq!(actual.final_state, expected.final_state);
    assert_eq!(actual.commands.len(), expected.commands.len());
    for (actual, expected) in actual.commands.iter().zip(expected.commands) {
        assert_eq!(
            (&actual.action, &actual.args),
            (&expected.action, &expected.args)
        );
    }
}

#[test]
fn integer_sets_and_optional_integers_keep_transient_values_until_storage() {
    let navigation =
        include_str!("../../../examples/model-patterns/browser/navigation-activation.model");
    let autosave = include_str!("../../../examples/model-patterns/autosave/autosave.model");
    let route_only = navigation.replace("if { visitId = visit }", "if { current = Detail }");
    let stale_save = autosave.replace("if { tab.pending = revision }", "if { some tab.pending }");
    assert_ne!(route_only, navigation);
    assert_ne!(stale_save, autosave);
    let set = "model IntegerSets
var pending: set Nat
var saved: lone Nat
var required: some Int
init { no pending, no saved, required = -1 }
behavior stage() { update { pending = union(pending, 1) } }
behavior display() {
  update { saved = if { 1 in pending } then #pending + 1 else none }
}
behavior transient() {
  update { pending = difference(union(pending, 4), 4) }
}
behavior clear() { update { required = difference(required, -1) } }
invariant TransientHasOneNewValue { #(difference(union(pending, 4), pending)) = 1 }
invariant TransientSetIsNonempty { some union(pending, 4) }
invariant TransientIntersectionDeduplicates { one (union(4, 5) & union(4, 0)) }
invariant SavedReadsAsInteger { some saved implies saved + 2 > saved }
transition TransientKeepsPending on transient { unchanged(pending) }
reachable Displayed { saved = 2 }
scope Int = -1..3, steps = 5";
    let escaped = set.replace(
        "difference(union(pending, 4), 4)",
        "difference(union(pending, 4), 3)",
    );
    assert_ne!(escaped, set);
    for source in [
        navigation,
        route_only.as_str(),
        autosave,
        stale_save.as_str(),
        set,
        escaped.as_str(),
    ] {
        let model = rmodel::parse(source).unwrap();
        let reference = rmodel::reference::analyze_actions(&model, 20_000);
        let actual = rprob::Compiled::new(&model).unwrap().analyze(20_000);
        assert_eq!(actual.verdicts, reference.verdicts);
        assert_eq!(actual.scope, reference.scope);
        assert_eq!(actual.declarations, reference.declarations);
        assert_eq!(actual.stop, reference.stop);
        assert_eq!(actual.enabled, reference.enabled);
        if source == escaped {
            assert_eq!(actual.scope, Violated { depth: 1 });
        } else {
            assert_eq!(actual.scope, Holds);
        }
    }
}

#[test]
fn finite_records_preserve_cas_evidence_and_nested_field_scope() {
    use rmodel::reference::Data;

    let register = include_str!("../../../examples/model-patterns/aba/register.model");
    let broken = register.replace("if { saved = current }", "if { saved.slot = current.slot }");
    assert_ne!(broken, register);
    let nested = "model Records
enum Owner = First | Second
record Stamp { tick: Nat }
record Message { stamp: Stamp }
var message: one Message
var inbox: Owner -> lone Message
init { message = Message { stamp: Stamp { tick: 0 } }, no inbox }
behavior send(owner: Owner) {
  guard { no owner.inbox }
  update { inbox = inbox ++ owner -> message }
}
behavior bump() {
  update { message = Message { stamp: Stamp { tick: message.stamp.tick + 1 } } }
}
invariant WholeRelationProjectionPreservesKeys { keys(inbox.stamp.tick) = keys(inbox) }
reachable SentAtOne { some First.inbox and First.inbox.stamp.tick = 1 }
scope Int = 0..1, steps = 5";
    let sparse = nested
        .replace("model Records", "model Records\nsig Padding")
        .replace("scope Int", "scope Padding = 4, Int");
    for source in [register, broken.as_str(), nested, sparse.as_str()] {
        let model = rmodel::parse(source).unwrap();
        let expected = rmodel::reference::analyze_actions(&model, 20_000);
        let actual = rprob::Compiled::new(&model).unwrap().analyze(20_000);
        assert_eq!(actual.verdicts, expected.verdicts);
        assert_eq!(actual.scope, expected.scope);
        assert_eq!(actual.declarations, expected.declarations);
        assert_eq!(actual.enabled, expected.enabled);
    }
    for source in [nested, sparse.as_str()] {
        let model = rmodel::parse(source).unwrap();
        let scope = rprob::witness(&model, "Scope", 20_000).unwrap();
        let reference = rmodel::reference::witness(&model, "Scope", 20_000).unwrap();
        assert!(!scope.complete);
        assert_eq!(scope.outcome, reference.outcome);
        assert_eq!(
            scope.witness.unwrap().final_state,
            reference.witness.unwrap().final_state
        );
    }
    let model = rmodel::parse(&broken).unwrap();
    let result = rprob::witness(&model, "NoStaleSuccess", 20_000).unwrap();
    assert!(result.complete);
    assert_eq!(result.outcome, Violated { depth: 4 });
    let path = result.witness.unwrap();
    let reference = rmodel::reference::witness(&model, "NoStaleSuccess", 20_000)
        .unwrap()
        .witness
        .unwrap();
    assert_eq!(path.final_state, reference.final_state);
    assert_eq!(path.commands.len(), 4);
    assert!(path.final_state.iter().any(|(name, value)| name == "current"
        && matches!(value.as_slice(), [row] if matches!(row.as_slice(), [Data::Record(name, _)] if name == "Word"))));
}

#[test]
fn computed_record_rows_preserve_replayed_offline_edits() {
    let source = include_str!("../../../examples/model-patterns/offline-merge/retry.model");
    let faulty = source.replace(
        "then request.receipts",
        "then Reply { outcome: Saved, atRevision: revision }",
    );
    assert_ne!(source, faulty);
    let check = "ExactRetryUsesOriginalReply";
    for (source, expected_outcome) in [(source, Holds), (&faulty, Violated { depth: 3 })] {
        let model = rmodel::parse(source).unwrap();
        let compiled = rprob::Compiled::new(&model).unwrap();
        for budget in [1, 20_000] {
            let expected = rmodel::reference::analyze_actions(&model, budget);
            let actual = compiled.analyze(budget);
            assert_eq!(actual.verdicts, expected.verdicts);
            assert_eq!(actual.scope, expected.scope);
            assert_eq!(actual.declarations, expected.declarations);
            assert_eq!(actual.stop, expected.stop);
            assert_eq!(actual.enabled, expected.enabled);
        }
        assert_eq!(outcome(&compiled.analyze(20_000), check), expected_outcome);
    }
    let model = rmodel::parse(&faulty).unwrap();
    let fast = rprob::witness(&model, check, 20_000).unwrap();
    let reference = rmodel::reference::witness(&model, check, 20_000).unwrap();
    assert_eq!(fast.outcome, reference.outcome);
    assert_eq!(
        fast.witness.unwrap().final_state,
        reference.witness.unwrap().final_state
    );
}

#[test]
fn computed_record_rows_respect_keys_and_field_bounds() {
    let source = "model ReceiptRows
sig Padding
enum Key = Left | Right
record Receipt { revision: Nat }
var receipts: Key -> lone Receipt
init { receipts = (Left -> Receipt { revision: 0 }) + (Right -> Receipt { revision: 0 }) }
behavior discard() {
  update {
    let unused = Key - Key
    receipts = receipts ++ unused -> Receipt { revision: 2 }
  }
}
behavior advance() {
  guard { some Left.receipts }
  update { receipts = receipts ++ Left -> Receipt { revision: Left.receipts.revision + 1 } }
}
transition DiscardLeavesBothRows on discard { unchanged(receipts) }
invariant RightRowIsUntouched { Right.receipts.revision = 0 }
reachable LeftAtOne { Left.receipts.revision = 1 }
";
    for padding in [1, 4] {
        let source = format!("{source}\nscope Padding = {padding}, Int = 0..1, steps = 4");
        let report = agree(&source, &[1, 1_000]);
        assert_eq!(outcome(&report, "LeftAtOne"), Reachable { depth: 1 });
        let model = rmodel::parse(&source).unwrap();
        let fast = rprob::witness(&model, "Scope", 1_000).unwrap();
        let reference = rmodel::reference::witness(&model, "Scope", 1_000).unwrap();
        assert_eq!(fast.outcome, Violated { depth: 2 });
        assert!(!fast.complete);
        assert_eq!(fast.outcome, reference.outcome);
        assert_eq!(
            fast.witness.unwrap().final_state,
            reference.witness.unwrap().final_state
        );
    }
}

#[test]
fn variants_keep_the_original_receipt_after_takeover() {
    let shipment = include_str!("../../../fixtures/rcheck/structured/shipped-transition.model");
    let shipment = agree(shipment, &[1_000]);
    assert_eq!(outcome(&shipment, "WasShipped"), Reachable { depth: 1 });
    let source =
        include_str!("../../../examples/model-patterns/crash-recovery/recovery-receipt.model");
    let broken = source.replace(
        "if { some known } then",
        "if { some known and (owner = Original or writer = owner) } then",
    );
    assert_ne!(broken, source);
    let check = "MatchingReplayReturnsStoredReceipt";
    for (source, expected) in [(source, Holds), (&broken, Violated { depth: 4 })] {
        let model = rmodel::parse(source).unwrap();
        let actual = rprob::Compiled::new(&model).unwrap().analyze(20_000);
        let reference = rmodel::reference::analyze_actions(&model, 20_000);
        assert_eq!(actual.verdicts, reference.verdicts);
        assert_eq!(actual.declarations, reference.declarations);
        assert_eq!(actual.scope, reference.scope);
        assert_eq!(outcome(&actual, check), expected);
    }
    let model = rmodel::parse(&broken).unwrap();
    let fast = rprob::witness(&model, check, 20_000)
        .unwrap()
        .witness
        .unwrap();
    let reference = rmodel::reference::witness(&model, check, 20_000)
        .unwrap()
        .witness
        .unwrap();
    assert_eq!(fast.final_state, reference.final_state);
    assert_eq!(fast.commands.len(), 4);
}

#[test]
fn nested_variant_payloads_preserve_tags_defaults_and_scope() {
    let source = "model NestedReplies
sig Padding
variant Status = Missing | Version(Nat)
record Receipt { body: Status }
variant Reply = Waiting | Stored(Receipt)
var reply: one Reply
var tick: one Nat
init { reply = Waiting() tick = 0 }
behavior publish() {
  update {
    reply = Stored(Receipt { body: Version(tick + 1) })
    tick = 1
  }
}
reachable WaitingHasNoStoredReceipt { is(reply, Waiting) and no payload(reply, Stored) }
reachable Sent { payload(reply, Stored).body = Version(1) }
scope Padding = 2, Int = 0..1, steps = 4";
    let model = rmodel::parse(source).unwrap();
    let actual = rprob::Compiled::new(&model).unwrap().analyze(1_000);
    let reference = rmodel::reference::analyze_actions(&model, 1_000);
    assert_eq!(actual.verdicts, reference.verdicts);
    assert_eq!(
        outcome(&actual, "WaitingHasNoStoredReceipt"),
        Reachable { depth: 0 }
    );
    let actual = rprob::witness(&model, "Scope", 1_000).unwrap();
    let reference = rmodel::reference::witness(&model, "Scope", 1_000).unwrap();
    assert_eq!(actual.outcome, Violated { depth: 2 });
    assert!(!actual.complete);
    assert_eq!(
        actual.witness.unwrap().final_state,
        reference.witness.unwrap().final_state
    );
    let actual = rprob::witness(&model, "Sent", 1_000).unwrap();
    let reference = rmodel::reference::witness(&model, "Sent", 1_000).unwrap();
    assert_eq!(actual.outcome, Reachable { depth: 1 });
    assert_eq!(
        actual.witness.unwrap().final_state,
        reference.witness.unwrap().final_state
    );
}

#[test]
fn delivery_mailbox_preserves_order_and_detects_early_release() {
    let source =
        include_str!("../../../examples/model-patterns/subscriptions/delivery-ticket-drain.model");
    let broken = source.replace(
        "sender = none queued = append(queued, Seal())",
        "sender = none queued = append(queued, Seal()) phase = Available",
    );
    assert_ne!(source, broken);
    let check = "PostingTheSealDoesNotReleaseTheTicket";
    for (source, want) in [(source, Holds), (&broken, Violated { depth: 3 })] {
        let model = rmodel::parse(source).unwrap();
        let fast = rprob::analyze_safety(&model, 20_000).unwrap();
        let reference = rmodel::reference::analyze_actions(&model, 20_000);
        assert_eq!(fast.report.verdicts, reference.verdicts);
        assert_eq!(fast.report.scope, reference.scope);
        assert_eq!(fast.report.declarations, reference.declarations);
        assert_eq!(fast.omitted_temporal, ["SealedEventuallyReleases"]);
        assert_eq!(outcome(&fast.report, check), want);
    }
    let model = rmodel::parse(&broken).unwrap();
    let actual = rprob::witness(&model, check, 20_000)
        .unwrap()
        .witness
        .unwrap();
    let expected = rmodel::reference::witness(&model, check, 20_000)
        .unwrap()
        .witness
        .unwrap();
    assert_eq!(actual.final_state, expected.final_state);
}

#[test]
fn mailbox_bounds_apply_on_storage_and_preserve_order() {
    let without_integers = "model Capacity
enum Message = A | B
sequence Queue = Message, max = 1
var queue: one Queue
init { queue = Queue[A] }
behavior grow() { update { queue = append(queue, B) } }
scope steps = 2";
    let small = rmodel::parse(without_integers).unwrap();
    let scope = rprob::witness(&small, "Scope", 100).unwrap();
    assert_eq!(scope.outcome, Violated { depth: 1 });
    assert!(!scope.complete);

    let source = "model MailboxBounds
enum Message = A | B
sequence Queue = Message, max = 2
var queue: one Queue
init { queue = Queue[A, A] }
behavior shift() { update { queue = tail(append(queue, B)) } }
behavior grow() { update { queue = append(queue, B) } }
behavior trim() {
  update { queue = removeAll(removeAt(take(queue, 2), -1), B) }
}
reachable InitiallyRepeated {
  at(queue, 0) = A at(queue, 1) = A #elements(queue) = 1
  append(queue, B) not in queue
  append(queue, A) != append(queue, B)
  len(append(queue, B)) = 3
}
reachable Shifted { queue = Queue[A, B] }
reachable Trimmed { queue = Queue[A] }
scope Int = -1..2, steps = 4";
    let model = rmodel::parse(source).unwrap();
    let fast = rprob::Compiled::new(&model).unwrap().analyze(1_000);
    let reference = rmodel::reference::analyze_actions(&model, 1_000);
    assert_eq!(fast.verdicts, reference.verdicts);
    assert_eq!(outcome(&fast, "InitiallyRepeated"), Reachable { depth: 0 });
    assert_eq!(outcome(&fast, "Shifted"), Reachable { depth: 1 });
    assert_eq!(outcome(&fast, "Trimmed"), Reachable { depth: 2 });
    let fast = rprob::witness(&model, "Scope", 1_000).unwrap();
    let reference = rmodel::reference::witness(&model, "Scope", 1_000).unwrap();
    assert_eq!(fast.outcome, Violated { depth: 1 });
    assert!(!fast.complete);
    assert_eq!(
        fast.witness.unwrap().final_state,
        reference.witness.unwrap().final_state
    );
}

#[test]
fn append_in_a_transition_is_compared_as_an_ordered_value() {
    let source = include_str!("../../../examples/model-patterns/stream-terminal/stream.model");
    let model = rmodel::parse(source).unwrap();
    let fast = rprob::Compiled::new(&model).unwrap().analyze(20_000);
    let reference = rmodel::reference::analyze_actions(&model, 20_000);
    assert_eq!(fast.verdicts, reference.verdicts);
    assert_eq!(outcome(&fast, "ReadFollowsQueue"), Holds);
    assert_eq!(outcome(&fast, "EndAfterTwoWrites"), Reachable { depth: 6 });
}

#[test]
fn derived_integer_indices_can_select_and_truncate_savepoints() {
    let source =
        include_str!("../../../examples/model-patterns/sqlite-transactions/savepoint-named.model");
    let report = agree(source, &[20_000]);
    assert_eq!(report.declarations, Holds);
    assert_eq!(
        outcome(&report, "RewindKeepsTheMostRecentMatchingMark"),
        Holds
    );
    assert_eq!(
        outcome(&report, "ReleaseRemovesThroughTheMostRecentMatch"),
        Holds
    );
}
