#[test]
fn fairness_and_temporal_requirements_are_separate_from_reachability() {
    let model = idealis_model::parse(
        "model Delivery
         enum Status = Waiting | Delivered
         var status: one Status
         init { status = Waiting }
         behavior deliver() { guard { status = Waiting } update { status = Delivered } }
         reachable CanDeliver { status = Delivered }
         fairness weak deliver
         eventually Delivers { status = Delivered }
         scope steps = 3",
    )
    .unwrap();
    let (source, config) = idealis_model::to_tla(&model, "Delivers").unwrap();
    assert!(source.contains("WF_"));
    assert!(config.contains("PROPERTY c_Delivers"));
}

#[test]
fn a_list_preserves_order_and_a_variant_carries_its_payload() {
    let model = idealis_model::parse(
        "model Log
         enum Entry = A | B
         sequence Journal = Entry, max = 2
         variant Result = Pending | Added(Entry)
         var log: one Journal
         var result: one Result
         init { log = Journal[] result = Pending() }
         behavior add(e: Entry) {
           guard { len(log) < 2 }
           update { log = append(log, e) result = Added(e) }
         }
         reachable Ordered { log = Journal[A, B] result = Added(B) }
         reachable Repeated { log = Journal[A, A] }
         scope Int = 0..2, steps = 3",
    )
    .unwrap();
    let report = idealis_model::reference::analyze(&model, 100_000);
    for verdict in report.verdicts {
        assert_eq!(
            verdict.outcome,
            idealis_model::reference::Outcome::Reachable { depth: 2 },
            "{verdict:?}"
        );
    }
}

#[test]
fn tail_preserves_sequence_type_order_and_duplicates() {
    use idealis_model::reference::Outcome::{Holds, Reachable};
    let source = concat!(
        include_str!("../../../examples/model-patterns/fifo/messages.ideal"),
        "\n",
        include_str!("../../../examples/model-patterns/fifo/mailbox.ideal"),
    );
    let model = idealis_model::parse(source).unwrap();
    let report = idealis_model::reference::analyze(&model, 10_000);
    assert_eq!(report.scope, Holds);
    assert_eq!(
        report
            .verdicts
            .iter()
            .map(|v| (v.check.as_str(), v.outcome))
            .collect::<Vec<_>>(),
        [
            ("EmptyTail", Holds),
            ("DuplicatesRemain", Reachable { depth: 4 }),
            ("DequeueKeepsOrder", Holds)
        ]
    );
    for (check, violates) in [
        ("EmptyTail", false),
        ("DequeueKeepsOrder", false),
        ("DuplicatesRemain", true),
    ] {
        let (source, config) = idealis_model::to_tla(&model, check).unwrap();
        let result = idealis_core::compile_tla_with_options(
            &source,
            &config,
            idealis_core::CompileOptions {
                irVersion: idealis_core::IrVersion::V2,
            },
        )
        .unwrap()
        .check(idealis_core::SafetyBounds {
            max_depth: 20,
            max_states: 10_000,
            max_work: 1_000_000,
            max_temporal_nodes: None,
        })
        .unwrap();
        assert_eq!(
            result.violation.is_some(),
            violates,
            "{check}: {:?}",
            result.reason
        );
        if !violates {
            assert_eq!(result.status, "complete", "{check}");
        }
    }
}

#[test]
fn tail_requires_a_sequence_and_preserves_a_declared_variant_tag() {
    for expr in ["tail()", "tail(A)"] {
        let source = format!(
            "model InvalidTail enum Item = A invariant Checked {{ {expr} = A }} scope steps = 2"
        );
        assert_eq!(
            idealis_model::parse(&source).unwrap_err().kind,
            idealis_model::ErrorKind::Type
        );
    }

    let model = idealis_model::parse(
        "model Constructor
         enum Item = A
         variant Result = tail(Item)
         var result: one Result
         init { result = tail(A) }
         invariant Kept { result = tail(A) }
         scope steps = 2",
    )
    .unwrap();
    assert_eq!(
        idealis_model::reference::evaluate(&model, 100)[0].outcome,
        idealis_model::reference::Outcome::Holds
    );
}

#[test]
fn take_keeps_the_order_and_clamps_the_length() {
    let model = idealis_model::parse(
        "model Take
enum Entry = A | B
sequence Entries = Entry, max = 3
var sample: one Entries
init { sample = Entries[B, B, A] }
invariant Prefix { take(sample, 2) = Entries[B, B] }
invariant Clamps { take(Entries[A, B], 3) = Entries[A, B] take(Entries[A], -1) = Entries[] }
scope Int = -1..3, steps = 2",
    )
    .unwrap();
    for verdict in idealis_model::reference::evaluate(&model, 10_000) {
        assert_eq!(verdict.outcome, idealis_model::reference::Outcome::Holds);
        let (source, config) = idealis_model::to_tla(&model, &verdict.check).unwrap();
        let result = idealis_core::compile_tla_with_options(
            &source,
            &config,
            idealis_core::CompileOptions {
                irVersion: idealis_core::IrVersion::V2,
            },
        )
        .unwrap()
        .check(idealis_core::SafetyBounds {
            max_depth: 10,
            max_states: 100,
            max_work: 10_000,
            max_temporal_nodes: None,
        })
        .unwrap();
        assert_eq!(result.status, "complete");
        assert!(result.violation.is_none());
    }
}

#[test]
fn history_checks_detect_bad_prefix_and_unguarded_account_display() {
    use idealis_model::reference::Outcome::{Holds, Reachable, Violated};
    let source = include_str!("../../../examples/model-patterns/auth-history/history.ideal");
    let verdict = |source: &str, name: &str| {
        idealis_model::reference::evaluate(&idealis_model::parse(source).unwrap(), 20_000)
            .into_iter()
            .find(|result| result.check == name)
            .unwrap()
            .outcome
    };
    assert_eq!(verdict(source, "AccountRequiresSignIn"), Holds);
    assert_eq!(verdict(source, "ChangedHistoryKeepsPrefix"), Holds);
    assert_eq!(
        verdict(source, "SignedOutAccountInHistory"),
        Reachable { depth: 2 }
    );
    let bad_prefix = source.replacen("take(history, cursor + 1)", "take(history, cursor)", 1);
    assert_eq!(
        verdict(&bad_prefix, "ChangedHistoryKeepsPrefix"),
        Violated { depth: 1 }
    );
    let unguarded_history =
        source.replacen("then SignIn else current", "then current else current", 1);
    assert_ne!(unguarded_history, source);
    assert_eq!(
        verdict(&unguarded_history, "AccountRequiresSignIn"),
        Violated { depth: 1 }
    );
}

#[test]
fn take_checks_its_arguments_without_capturing_a_variant_constructor() {
    let model = idealis_model::parse(
        "model Constructor
enum Item = A
variant Result = take(Item)
var result: one Result
init { result = take(A) }
invariant Kept { result = take(A) }
scope steps = 2",
    )
    .unwrap();
    assert_eq!(
        idealis_model::reference::evaluate(&model, 100)[0].outcome,
        idealis_model::reference::Outcome::Holds
    );
    for expr in ["take(A, 1)", "take(Entries[A], A)"] {
        let source = format!(
            "model InvalidTake enum Item = A sequence Entries = Item, max = 2
             invariant Checked {{ {expr} = Entries[A] }} scope Int = 0..2, steps = 2"
        );
        assert_eq!(
            idealis_model::parse(&source).unwrap_err().kind,
            idealis_model::ErrorKind::Type
        );
    }
}

#[test]
fn a_model_without_relations_can_check_constant_values_in_tla() {
    use idealis_model::reference::Outcome::{Holds, Reachable, Violated};
    for action in ["", "behavior idle() {}"] {
        let model = idealis_model::parse(&format!(
            "model ConstantValues
enum Entry = A | B
sequence Entries = Entry, max = 2
{action}
invariant TruePrefix {{ take(Entries[A, B], 1) = Entries[A] }}
invariant FalsePrefix {{ take(Entries[A, B], 1) = Entries[B] }}
reachable Exists {{ take(Entries[A], 0) = Entries[] }}
scope Int = 0..2, steps = 2"
        ))
        .unwrap();
        let results = idealis_model::reference::evaluate(&model, 100);
        for (name, expected) in [
            ("TruePrefix", Holds),
            ("FalsePrefix", Violated { depth: 0 }),
            ("Exists", Reachable { depth: 0 }),
        ] {
            assert_eq!(
                results.iter().find(|v| v.check == name).unwrap().outcome,
                expected
            );
            let (source, config) = idealis_model::to_tla(&model, name).unwrap();
            let result = idealis_core::compile_tla_with_options(
                &source,
                &config,
                idealis_core::CompileOptions {
                    irVersion: idealis_core::IrVersion::V2,
                },
            )
            .unwrap()
            .check(idealis_core::SafetyBounds {
                max_depth: 4,
                max_states: 100,
                max_work: 10_000,
                max_temporal_nodes: None,
            })
            .unwrap();
            if expected == Holds {
                assert_eq!(result.status, "complete");
                assert!(result.violation.is_none());
            } else {
                assert!(result.violation.is_some(), "{name}: {:?}", result.reason);
            }
        }
    }
}

#[test]
fn remove_at_keeps_other_elements_in_order() {
    let model = idealis_model::parse(
        "model Remove
enum Item = A | B
sequence Items = Item, max = 3
var sample: one Items
init { sample = Items[A, B, A] }
invariant Middle { removeAt(sample, 1) = Items[A, A] }
invariant Ends { removeAt(sample, 0) = Items[B, A] removeAt(sample, 2) = Items[A, B] }
invariant Outside { removeAt(sample, -1) = sample removeAt(sample, 3) = sample removeAt(Items[], 0) = Items[] }
scope Int = -1..3, steps = 2",
    )
    .unwrap();
    for answer in idealis_model::reference::evaluate(&model, 1000) {
        assert_eq!(answer.outcome, idealis_model::reference::Outcome::Holds);
        let (source, cfg) = idealis_model::to_tla(&model, &answer.check).unwrap();
        let result = idealis_core::compile_tla_with_options(
            &source,
            &cfg,
            idealis_core::CompileOptions {
                irVersion: idealis_core::IrVersion::V2,
            },
        )
        .unwrap()
        .check(idealis_core::SafetyBounds {
            max_depth: 4,
            max_states: 1000,
            max_work: 100_000,
            max_temporal_nodes: None,
        })
        .unwrap();
        assert_eq!(
            result.status, "complete",
            "{}: {:?}",
            answer.check, result.reason
        );
    }
}

#[test]
fn remove_at_checks_indices_and_keeps_one_argument_variant_names() {
    let template = "model Remove
enum Item = A
sequence Items = Item, max = 2
variant Wrapper = removeAt(Item)
var value: one Wrapper
init { value = removeAt(A) }
invariant Kept { value = removeAt(A) }
scope Int = 0..2, steps = 2";
    assert!(idealis_model::parse(template).is_ok());
    for expression in ["removeAt(A, 0)", "removeAt(Items[A], A)"] {
        let source = format!("{template}\ninvariant Invalid {{ {expression} = Items[] }}");
        assert_eq!(
            idealis_model::parse(&source).unwrap_err().kind,
            idealis_model::ErrorKind::Type
        );
    }
}

#[test]
fn elements_projects_the_items_of_a_sequence_as_a_set() {
    let model = idealis_model::parse(
        "model Elements
enum Item = A | B
sequence Items = Item, max = 3
var sample: one Items
init { sample = Items[A, B, A] }
invariant DistinctMembers { elements(sample) = A + B #elements(sample) = 2 }
invariant Empty { no elements(Items[]) }
scope Int = 0..3, steps = 2",
    )
    .unwrap();
    for answer in idealis_model::reference::evaluate(&model, 1000) {
        assert_eq!(answer.outcome, idealis_model::reference::Outcome::Holds);
        let (source, cfg) = idealis_model::to_tla(&model, &answer.check).unwrap();
        let result = idealis_core::compile_tla_with_options(
            &source,
            &cfg,
            idealis_core::CompileOptions {
                irVersion: idealis_core::IrVersion::V2,
            },
        )
        .unwrap()
        .check(idealis_core::SafetyBounds {
            max_depth: 4,
            max_states: 1000,
            max_work: 100_000,
            max_temporal_nodes: None,
        })
        .unwrap();
        assert_eq!(
            result.status, "complete",
            "{}: {:?}",
            answer.check, result.reason
        );
        assert!(result.violation.is_none());
    }
}

#[test]
fn elements_requires_a_sequence_unless_it_is_a_variant_constructor() {
    assert!(
        idealis_model::parse(
            "model Elements enum Item = A variant Wrapped = elements(Item)
         var value: one Wrapped init { value = elements(A) }
         invariant Kept { value = elements(A) } scope steps = 2",
        )
        .is_ok()
    );
    assert_eq!(
        idealis_model::parse(
            "model BadElements enum Item = A invariant Invalid { elements(A) = A }
             scope steps = 2"
        )
        .unwrap_err()
        .kind,
        idealis_model::ErrorKind::Type,
    );
}

#[test]
fn remove_all_uses_a_set_without_reordering_the_sequence() {
    let model = idealis_model::parse(
        "model RemoveAll
enum Item = A | B | C
sequence Items = Item, max = 3
var sample: one Items
var excluded: set Item
init { sample = Items[C, B, C] excluded = B }
invariant Survivors { removeAll(sample, excluded) = Items[C, C] }
invariant Many { removeAll(Items[C, B, A], A + C) = Items[B] }
invariant Boundaries {
  removeAll(sample, none) = sample
  removeAll(Items[A, A, B], A) = Items[B]
  removeAll(Items[], Item) = Items[]
}
scope steps = 2",
    )
    .unwrap();
    for answer in idealis_model::reference::evaluate(&model, 1000) {
        assert_eq!(answer.outcome, idealis_model::reference::Outcome::Holds);
        let (source, cfg) = idealis_model::to_tla(&model, &answer.check).unwrap();
        let result = idealis_core::compile_tla_with_options(
            &source,
            &cfg,
            idealis_core::CompileOptions {
                irVersion: idealis_core::IrVersion::V2,
            },
        )
        .unwrap()
        .check(idealis_core::SafetyBounds {
            max_depth: 4,
            max_states: 1000,
            max_work: 100_000,
            max_temporal_nodes: None,
        })
        .unwrap();
        assert_eq!(
            result.status, "complete",
            "{}: {:?}",
            answer.check, result.reason
        );
        assert!(result.violation.is_none());
    }
}

#[test]
fn remove_all_checks_the_element_sort_and_preserves_unary_variant_calls() {
    let source = "model Remove
enum Item = A
enum Other = B
sequence Items = Item, max = 2
variant Wrapped = removeAll(Item)
var value: one Wrapped
init { value = removeAll(A) }
invariant Constructor { value = removeAll(A) }
scope steps = 2";
    assert!(idealis_model::parse(source).is_ok());
    for expr in ["removeAll(Items[A], B)", "removeAll(Items[A], A -> A)"] {
        let source = format!("{source}\ninvariant Bad {{ {expr} = Items[] }}");
        assert_eq!(
            idealis_model::parse(&source).unwrap_err().kind,
            idealis_model::ErrorKind::Type
        );
    }
}
