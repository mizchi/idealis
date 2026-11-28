//! Plan 0103: integer, bool and record values. The reference evaluator and
//! rtlc on the TLA+ lowering give the verdicts written before running them;
//! a model whose integers leave the scope is unknown, never a pass.
use idealis_model::reference::Outcome;

fn word(outcome: Outcome) -> (&'static str, Option<usize>) {
    match outcome {
        Outcome::Empty => ("empty", None),
        Outcome::Holds => ("holds", None),
        Outcome::Violated { depth } => ("violated", Some(depth)),
        Outcome::Reachable { depth } => ("reachable", Some(depth)),
        Outcome::Unreachable => ("unreachable", None),
        Outcome::Unknown => ("unknown", None),
    }
}

/// rtlc on one check; a negative answer becomes unknown when a state
/// leaves the integer scope (the built-in Scope check).
fn explicit(model: &idealis_model::Model, name: &str) -> (&'static str, Option<usize>) {
    let run = |name: &str| {
        let (source, config) = idealis_model::to_tla(model, name).unwrap();
        let program = idealis_core::compile_tla_with_options(
            &source,
            &config,
            idealis_core::CompileOptions {
                irVersion: idealis_core::IrVersion::V2,
            },
        )
        .unwrap_or_else(|error| panic!("{name}: {error:?}\n{source}"));
        program
            .check(idealis_core::SafetyBounds {
                max_depth: 1_000,
                max_states: 2_000_000,
                max_work: 500_000_000,
                max_temporal_nodes: None,
            })
            .unwrap_or_else(|error| panic!("{name}: {error:?}\n{source}"))
    };
    let result = run(name);
    let reachable = model
        .checks
        .iter()
        .any(|check| check.name == name && check.kind == idealis_model::CheckKind::Reachable);
    if result.violation.is_some() {
        let verdict = if reachable { "reachable" } else { "violated" };
        return (verdict, Some(result.depth as usize));
    }
    match result.status.as_str() {
        "empty" => ("empty", None),
        "complete" => {
            if run(idealis_model::SCOPE).violation.is_some() {
                ("unknown", None)
            } else if reachable {
                ("unreachable", None)
            } else {
                ("holds", None)
            }
        }
        other => panic!("{name}: {other}"),
    }
}

fn expect(source: &str, expected: &[(&str, &str, Option<usize>)]) {
    let model = idealis_model::parse(source).unwrap_or_else(|error| panic!("{error}"));
    let report = idealis_model::reference::analyze(&model, 2_000_000);
    for &(name, verdict, depth) in expected {
        let reference = match name {
            "Declarations" => report.declarations,
            _ => {
                report
                    .verdicts
                    .iter()
                    .find(|v| v.check == name)
                    .unwrap()
                    .outcome
            }
        };
        assert_eq!(word(reference), (verdict, depth), "{name} reference");
        assert_eq!(explicit(&model, name), (verdict, depth), "{name} explicit");
    }
}

const BANK: &str = "model Bank
sig Account
var balance: Account -> one Int
init { all a: Account | a.balance = 0 }
action deposit(a: Account, n: Int) {
  n > 0
  a.balance + n <= 4
  balance' = balance - a -> a.balance + a -> (a.balance + n)
}
action withdraw(a: Account, n: Int) {
  n > 0
  a.balance >= n
  balance' = balance - a -> a.balance + a -> (a.balance - n)
}
invariant NonNegative { all a: Account | a.balance >= 0 }
reachable Rich { some a: Account | a.balance >= 3 }
scope Account = 2, Int = -4..4, steps = 4";

#[test]
fn integers_in_an_account() {
    expect(
        BANK,
        &[
            ("NonNegative", "holds", None),
            ("Rich", "reachable", Some(1)),
            ("Declarations", "holds", None),
        ],
    );
}

#[test]
fn an_unchecked_withdrawal_goes_negative() {
    expect(
        &BANK.replace("  a.balance >= n\n", ""),
        &[
            ("NonNegative", "violated", Some(1)),
            ("Rich", "reachable", Some(1)),
        ],
    );
}

#[test]
fn leaving_the_scope_is_unknown() {
    expect(
        &BANK.replace("  a.balance + n <= 4\n", ""),
        &[
            ("NonNegative", "unknown", None),
            ("Rich", "reachable", Some(1)),
            ("Declarations", "unknown", None),
        ],
    );
}

#[test]
fn bools_on_doors() {
    let source = "model Doors
sig Door
var open: Door -> one Bool
init { all d: Door | not d.open }
action push(d: Door) { not d.open  open' = open - d -> false + d -> true }
action pull(d: Door) { d.open  open' = open - d -> true + d -> false }
invariant AllClosed { all d: Door | not d.open }
reachable BothOpen { all d: Door | d.open }
scope Door = 2, steps = 4";
    expect(
        source,
        &[
            ("AllClosed", "violated", Some(1)),
            ("BothOpen", "reachable", Some(2)),
            ("Declarations", "holds", None),
        ],
    );
    // Bool is an enum: the Alloy lowering supports it.
    let model = idealis_model::parse(source).unwrap();
    assert_eq!(idealis_model::alloy_supports(&model), Ok(()));
    ralloy::Model::parse(&idealis_model::to_alloy(&model)).unwrap();
}

#[test]
fn records_in_a_purse() {
    let source = "model Purse
enum Currency = JPY | USD
record Money { amount: Int, currency: Currency }
sig Wallet
var cash: Wallet -> one Money
init { all w: Wallet | w.cash = Money { amount: 0, currency: JPY } }
action add(w: Wallet, m: Money) {
  m.currency = w.cash.currency
  m.amount > 0
  w.cash.amount + m.amount <= 3
  cash' = cash - w -> w.cash + w -> Money { currency: w.cash.currency, amount: w.cash.amount + m.amount }
}
invariant OnlyYen { all w: Wallet | w.cash.currency = JPY }
reachable Three { some w: Wallet | w.cash.amount = 3 }
reachable Dollars { some w: Wallet | w.cash.currency = USD }
scope Wallet = 1, Int = 0..3, steps = 4";
    expect(
        source,
        &[
            ("OnlyYen", "holds", None),
            ("Three", "reachable", Some(1)),
            ("Dollars", "unreachable", None),
            ("Declarations", "holds", None),
        ],
    );
    let model = idealis_model::parse(source).unwrap();
    assert!(idealis_model::alloy_supports(&model).is_err());
}

#[test]
fn record_projection_uses_the_receiver_even_when_an_action_argument_has_the_field_name() {
    expect(
        "model Versions
         sig Replica
         record Snapshot { revision: Nat }
         var copies: Replica -> one Snapshot
         init { all r: Replica | r.copies = Snapshot { revision: 0 } }
         behavior store(replica: Replica, revision: Nat) {
           guard { revision > replica.copies.revision }
           update { copies = copies ++ replica -> Snapshot { revision: revision } }
         }
         transition StoresNewerRevision on store {
           before(replica.copies.revision) < revision
           after(replica.copies.revision) = revision
         }
         invariant ProjectionPreservesKeys {
           all r: Replica | r.(copies.revision) = r.copies.revision
         }
         reachable Stored { some r: Replica | r.copies.revision = 2 }
         scope Replica = 1, Int = 0..2, steps = 3",
        &[
            ("StoresNewerRevision", "holds", None),
            ("ProjectionPreservesKeys", "holds", None),
            ("Stored", "reachable", Some(1)),
        ],
    );
}

#[test]
fn presence_refines_optional_records_in_order_and_in_the_same_phase() {
    let base = "
model Presence
record Snapshot { revision: Nat }
var accepted: lone Snapshot
init { no accepted }
behavior publish() {
  guard { no accepted }
  update { accepted = Snapshot { revision: 1 } }
}
behavior clear() {
  guard { some accepted }
  update { accepted = none }
}
scope Int = 0..2, steps = 3
";
    let checks = "
invariant RevisionFloor { some accepted implies accepted.revision >= 1 }
invariant FieldFloor { some accepted.revision implies accepted.revision >= 1 }
reachable Published { some accepted and accepted.revision >= 1 }
transition PublishedRevision on publish {
  some after(accepted) implies after(accepted.revision) >= 1
}
transition ClearedRevision on clear {
  some before(accepted) implies before(accepted.revision) >= 1
}
";
    expect(
        &format!("{base}{checks}"),
        &[
            ("RevisionFloor", "holds", None),
            ("FieldFloor", "holds", None),
            ("Published", "reachable", Some(1)),
            ("PublishedRevision", "holds", None),
            ("ClearedRevision", "holds", None),
        ],
    );

    for formula in [
        "accepted.revision >= 1",
        "accepted.revision >= 1 and some accepted",
        "some accepted or accepted.revision >= 1",
        "(some accepted implies true) and accepted.revision >= 1",
        "some before(accepted) implies after(accepted.revision) >= 1",
        "some after(accepted) implies before(accepted.revision) >= 1",
    ] {
        let source = format!("{base}\ntransition Unsafe {{ {formula} }}");
        let error = idealis_model::parse(&source).unwrap_err();
        assert!(
            error.message.contains("exactly one integer"),
            "{formula}: {error}"
        );
    }
}

#[test]
fn field_names_are_scoped_to_the_record_type() {
    expect(
        "model LocalFields
         sig Order
         enum Channel = Web | Store
         record Amount { units: Nat }
         record Receipt { via: Channel, units: Nat }
         var units: Order -> one Amount
         var posted: Order -> one Receipt
         init {
           units = Order -> Amount { units: 1 }
           posted = Order -> Receipt { via: Web, units: 0 }
         }
         behavior capture(order: Order) {
           guard { order.posted.units = 0 }
           update { posted = posted ++ order -> Receipt { via: Store, units: order.units.units } }
         }
         transition CopiesAmount on capture {
           after(order.posted.units) = before(order.units.units)
         }
         reachable Captured { some order: Order | order.posted.units = 1 }
         scope Order = 1, Int = 0..1, steps = 2",
        &[
            ("CopiesAmount", "holds", None),
            ("Captured", "reachable", Some(1)),
        ],
    );
    let err = idealis_model::parse(
        "model Duplicate record Amount { units: Nat, units: Nat } scope Int = 0..1, steps = 2",
    )
    .unwrap_err();
    assert!(err.message.contains("Amount") && err.message.contains("duplicate field units"));
}

#[test]
fn update_bindings_share_values_in_the_prestate() {
    let source = "
model Local
var level: one Nat
var old: one Nat
var receipt: one Nat
init { level = 1 old = 0 receipt = 0 }
behavior capture() {
  guard { level = 1 }
  update {
    let previous = level
    let next = previous + 1
    level = next
    old = previous
    receipt = next
  }
}
transition SameValue on capture { after(old) = before(level) after(level) = after(receipt) }
reachable Captured { level = 2 old = 1 receipt = 2 }
scope Int = 0..2, steps = 2
";
    expect(
        source,
        &[
            ("SameValue", "holds", None),
            ("Captured", "reachable", Some(1)),
        ],
    );
    for invalid in [
        source.replace("let previous = level", "let previous = next"),
        source.replace("let previous = level", "let level = 1"),
        source.replace("old = 1 receipt", "old = previous receipt"),
    ] {
        assert!(idealis_model::parse(&invalid).is_err(), "{invalid}");
    }
    let expanded = (1..16)
        .map(|index| format!("let local{index} = local{} + local{}", index - 1, index - 1))
        .collect::<Vec<_>>()
        .join("\n");
    let oversized = source.replace(
        "let previous = level",
        &format!("let local0 = 0\n{expanded}\nlet previous = level"),
    );
    let error = idealis_model::parse(&oversized).unwrap_err();
    assert!(error.message.contains("expanded update"), "{error}");
}

#[test]
fn ill_typed_values_are_rejected() {
    let base = "model M\nsig A\nenum C = X | Y\nrecord R { n: Int, c: C }\nvar b: A -> one Int\nvar l: A -> lone Int\n";
    let scope = "\nscope A = 2, Int = 0..3, steps = 2";
    for (body, needle) in [
        (
            "var xs: A -> set Int\ninvariant I { some xs }",
            "one or lone value",
        ),
        ("invariant I { all a: A | a.l + 1 > 0 }", "may be empty"),
        ("invariant I { A.b + 1 > 0 }", "may be empty"),
        ("invariant I { some a: A | a.b + a = a }", "different sorts"),
        (
            "var r: A -> one R\ninvariant I { all a: A | a.r = R { n: 1, k: X } }",
            "unexpected field",
        ),
        (
            "var r: A -> one R\ninvariant I { all a: A | a.r = R { n: 1 } }",
            "missing field",
        ),
        (
            "var r: A -> one R\ninvariant I { all a: A | a.r = R { n: X, c: X } }",
            "needs one value of Int",
        ),
        (
            "invariant I { all r: R | some r }",
            "only atoms are quantified",
        ),
        ("invariant I { some a: A | a.b }", "expression of Bool"),
    ] {
        let error = idealis_model::parse(&format!("{base}{body}{scope}")).unwrap_err();
        assert!(error.message.contains(needle), "{body}: {error}");
    }
    let error = idealis_model::parse(
        "model M\nsig A\nvar b: A -> one Int\ninvariant I { some b }\nscope A = 1, steps = 1",
    )
    .unwrap_err();
    assert!(error.message.contains("scope Int"), "{error}");
}

/// Found by the 0103 generator: a parenthesized integer before `<` was
/// read as a Bool formula.
#[test]
fn parenthesized_integers_start_comparisons() {
    let model = idealis_model::parse(
        "model P
sig A
var b: A -> one Int
init { all a: A | a.b = 0 }
action up(a: A, n: Int) { (n + 0) < 2  (a.b * 2) >= 0  b' = b - a -> a.b + a -> (n) }
invariant Small { all a: A | (a.b + 0) <= 2 }
scope A = 1, Int = 0..2, steps = 2",
    );
    assert!(model.is_ok(), "{:?}", model.err());
}

#[test]
fn naturals_use_the_nonnegative_part_of_the_integer_scope() {
    let model = idealis_model::parse(
        "model NaturalCounter
         var n: one Nat
         init { n = 0 }
         behavior take(x: Nat) { guard { x = n + 1 } update { n = x } }
         invariant NonNegative { n >= 0 }
         reachable Two { n = 2 }
         scope Int = -2..2, steps = 3",
    )
    .unwrap();
    let report = idealis_model::reference::analyze(&model, 1_000);
    assert_eq!(report.scope, idealis_model::reference::Outcome::Holds);
    assert_eq!(
        report.verdicts[1].outcome,
        idealis_model::reference::Outcome::Reachable { depth: 2 }
    );
}

#[test]
fn arithmetic_overflow_is_rejected_before_a_checker_can_report_holds() {
    let source = "model MaxInt
        var n: one Int
        init { n = 9223372036854775807 }
        behavior increment() { update { n = n + 1 } }
        invariant AtMax { n = 9223372036854775807 }
        scope Int = 9223372036854775806..9223372036854775807, steps = 2";
    assert!(
        idealis_model::parse(source)
            .unwrap_err()
            .message
            .contains("arithmetic")
    );
}

#[test]
fn a_wide_exact_model_can_be_checked_by_the_reference_without_promising_tlc_support() {
    let model = idealis_model::parse(
        "model Wide
         var n: one Int
         init { n = 2147483647 }
         behavior increment() { update { n = n + 1 } }
         invariant InRange { n <= 2147483647 }
         scope Int = 2147483646..2147483647, steps = 2",
    )
    .unwrap();
    let report = idealis_model::reference::analyze(&model, 100);
    assert_eq!(
        report.scope,
        idealis_model::reference::Outcome::Violated { depth: 1 }
    );
    let error = idealis_model::to_tla(&model, "InRange").unwrap_err();
    assert!(error.message.contains("arithmetic"), "{error}");
}

#[test]
fn an_overflow_through_a_record_literal_projection_is_rejected() {
    let error = idealis_model::parse(
        "model Project
         record Amount { units: Int }
         var n: one Int
         init { n = 0 }
         behavior explode() { update { n = (Amount {units: 9223372036854775807}).units + 1 } }
         scope Int = 0..1, steps = 2",
    )
    .unwrap_err();
    assert!(error.message.contains("arithmetic"), "{error}");
}

#[test]
fn the_reference_accepts_the_lowest_signed_literal_without_wrap() {
    let model = idealis_model::parse(
        "model Low
         var n: one Int
         init { n = -9223372036854775808 }
         invariant AtMin { n = -9223372036854775808 }
         scope Int = -9223372036854775808..-9223372036854775807, steps = 2",
    )
    .unwrap();
    let result = idealis_model::reference::evaluate(&model, 100);
    assert_eq!(result[0].outcome, idealis_model::reference::Outcome::Holds);
    assert!(idealis_model::tla_supports(&model).is_err());
}

#[test]
fn arithmetic_in_updates_accounts_for_large_count_domains() {
    // This model has no integer comparisons, so the Alloy-specific limit
    // on the universe size does not apply to the update.
    let members = (0..401)
        .map(|i| format!("Member{i}"))
        .collect::<Vec<_>>()
        .join(" | ");
    let source = format!(
        "model Many
         enum Members = {members}
         var n: one Int
         init {{ n = 0 }}
         behavior count() {{ update {{ n = #Members + 9223372036854775407 }} }}
         scope Int = 0..0, steps = 2"
    );
    assert!(
        idealis_model::parse(&source)
            .unwrap_err()
            .message
            .contains("arithmetic")
    );
}

#[test]
fn relation_keys_project_distinct_left_values_in_both_states() {
    let model = idealis_model::parse(
        "model ConnectionKeys
         enum Ticket = First | Second
         enum Slot = Primary | Hedge
         record Connection { slot: Slot }
         var pending: Ticket -> lone Connection
         init { no pending }
         behavior connect() {
           update { pending = First -> Connection { slot: Primary } }
         }
         behavior clear() { guard { some pending } update { pending = none } }
         reachable Connected { keys(pending) = First #keys(pending + First -> Connection { slot: Hedge }) = 1 }
         invariant MatchesExistence {
           { all ticket: Ticket | ticket in keys(pending) implies some ticket.pending }
           { all ticket: Ticket | some ticket.pending implies ticket in keys(pending) }
         }
         transition ClearsKeys on clear { some before(keys(pending)) no after(keys(pending)) }
         scope steps = 3",
    )
    .unwrap();
    for (name, expected) in [
        ("Connected", "reachable"),
        ("MatchesExistence", "holds"),
        ("ClearsKeys", "holds"),
    ] {
        let verdicts = idealis_model::reference::evaluate(&model, 20_000);
        assert_eq!(
            word(verdicts.iter().find(|v| v.check == name).unwrap().outcome).0,
            expected
        );
        assert_eq!(explicit(&model, name).0, expected);
    }
}

#[test]
fn keys_requires_a_binary_relation_and_existing_variant_tags_keep_their_meaning() {
    for expression in ["keys(Key)", "keys(none)", "keys()"] {
        let error = idealis_model::parse(&format!(
            "model Invalid enum Key = A invariant Bad {{ no {expression} }} scope steps = 1"
        ))
        .unwrap_err();
        assert!(
            error.message.contains("keys needs one binary relation"),
            "{error}"
        );
    }
    let model = idealis_model::parse(
        "model Existing enum Key = A variant Event = keys(Key)
         var value: one Event init { value = keys(A) }
         invariant TagStays { is(value, keys) }
         scope steps = 1",
    )
    .unwrap();
    assert_eq!(
        idealis_model::reference::evaluate(&model, 100)[0].outcome,
        Outcome::Holds
    );
}

#[test]
fn binary_cardinality_does_not_extend_the_numeric_storage_scope() {
    let model = idealis_model::parse(
        "model CountScope enum From = A | B | C enum To = D
         var stored: one Nat init { stored = 0 }
         behavior save() { update { stored = #(From -> To) } }
         scope Int = 0..2, steps = 2",
    )
    .unwrap();
    let witness = idealis_model::reference::witness(&model, idealis_model::SCOPE, 100).unwrap();
    assert_eq!(
        witness.outcome,
        idealis_model::reference::Outcome::Violated { depth: 1 }
    );
}

#[test]
fn a_fixed_sequence_uses_the_state_budget_without_enumerating_the_value_domain() {
    use idealis_model::reference::{Outcome, StopReason, analyze, witness};

    let source =
        include_str!("../../../examples/model-patterns/node-event-loop/nested-io-batch.ideal");
    let model = idealis_model::parse(source).unwrap();
    let answer = witness(&model, "ObservedBatch", 4).unwrap();
    assert!(answer.complete);
    assert_eq!(answer.outcome, Outcome::Reachable { depth: 1 });
    assert_eq!(analyze(&model, 4).verdicts[0].outcome, answer.outcome);

    let free = idealis_model::parse(&source.replace("init { calls = Invocations[] }", "")).unwrap();
    let partial = witness(&free, "ObservedBatch", 4).unwrap();
    assert!(!partial.complete);
    assert_eq!(partial.outcome, Outcome::Unknown);
    assert_eq!(partial.stop, Some(StopReason::Budget));
    let complete = witness(&free, "ObservedBatch", 700).unwrap();
    assert!(complete.complete);
    assert_eq!(complete.outcome, Outcome::Reachable { depth: 0 });

    // Two batches have over a hundred thousand possible values. The only
    // reachable values in this model are the ones constructed in the source.
    let nested_source = "model NestedBatches
         enum Callback = Microtask | NestedMicrotask | Immediate | Timer
         sequence Batch = Callback, max = 4
         sequence History = Batch, max = 2
         var history: one History
         init { history = History[] }
         behavior store() {
           guard { history = History[] }
           update { history = History[Batch[Microtask], Batch[Timer]] }
         }
         reachable Stored { history = History[Batch[Microtask], Batch[Timer]] }
         scope steps = 2";
    let nested = idealis_model::parse(nested_source).unwrap();
    let answer = witness(&nested, "Stored", 4).unwrap();
    assert!(answer.complete);
    assert_eq!(answer.outcome, Outcome::Reachable { depth: 1 });
    assert!(idealis_model::reference::Machine::new(&nested).is_ok());
    assert!(
        idealis_model::tla_supports(&nested)
            .unwrap_err()
            .contains("History")
    );

    let nested_free =
        idealis_model::parse(&nested_source.replace("init { history = History[] }", "")).unwrap();
    let answer = witness(&nested_free, "Stored", 4).unwrap();
    assert!(!answer.complete);
    assert_eq!(answer.outcome, Outcome::Unknown);
    assert_eq!(answer.stop, Some(StopReason::Budget));

    assert!(idealis_model::reference::enumerable(&nested_free).is_ok());

    let argument =
        idealis_model::parse(&nested_source.replace("store()", "store(value: History)")).unwrap();
    let budget = idealis_model::reference::SearchBudget {
        states: 4,
        arguments: 1,
    };
    let answer =
        idealis_model::reference::witness_with_budget(&argument, "Stored", budget).unwrap();
    assert_eq!(answer.outcome, Outcome::Unknown);
    assert_eq!(answer.stop, Some(StopReason::ArgumentBudget));
}

#[test]
fn indexed_initial_values_check_and_encode_a_witness_beyond_the_materialization_limit() {
    use idealis_model::reference::{Outcome, StopReason, witness};

    let source =
        include_str!("../../../examples/model-patterns/crash-recovery/restored-queue.ideal");
    let model = idealis_model::parse(source).unwrap();
    let small = witness(&model, "RestoresFullQueue", 4).unwrap();
    assert_eq!(small.stop, Some(StopReason::Budget));
    assert!(!small.complete);

    let enough = witness(&model, "RestoresFullQueue", 9000).unwrap();
    assert!(enough.complete);
    assert_eq!(enough.outcome, Outcome::Reachable { depth: 1 });
    let trace = enough.witness.unwrap();
    assert_eq!(trace.commands[0].action, "restart");
    assert_eq!(trace.initial[0].1, trace.final_state[1].1);

    let fault = source.replace(
        "restored = saved }",
        "restored = Checkpoint { pending: Pending[], retries: saved.retries } }",
    );
    assert_ne!(source, fault);
    let fault = idealis_model::parse(&fault).unwrap();
    let answer = witness(&fault, "RestoresCheckpoint", 9000).unwrap();
    assert!(answer.complete);
    assert_eq!(answer.outcome, Outcome::Violated { depth: 1 });
}

#[test]
fn discovers_one_enabled_call_in_a_large_argument_domain() {
    use idealis_model::reference::{
        Outcome, SearchBudget, StopReason, transition_witness_with_budget, witness_with_budget,
    };
    let model = idealis_model::parse(include_str!(
        "../../../examples/model-patterns/crash-recovery/refill-queue.ideal"
    ))
    .unwrap();
    let budget = |arguments| SearchBudget {
        states: 4,
        arguments,
    };
    for (arguments, outcome) in [
        (1369, Outcome::Unknown),
        (1370, Outcome::Reachable { depth: 1 }),
    ] {
        let answer = transition_witness_with_budget(
            &model,
            "refill",
            "some after(pending)",
            budget(arguments),
        )
        .unwrap();
        assert_eq!(answer.outcome, outcome);
        assert_eq!(answer.witness.is_some(), arguments == 1370);
        assert!(!answer.complete);
        assert_eq!(answer.stop, Some(StopReason::ArgumentBudget));
    }
    let answer = witness_with_budget(&model, "Refilled", budget(116_623)).unwrap();
    assert!(answer.complete);
    assert_eq!(answer.outcome, Outcome::Reachable { depth: 1 });
    let path = answer.witness.unwrap();
    assert_eq!(path.commands[0].action, "refill");
    assert_eq!(path.commands[0].args[0].0, "history");
}

#[test]
fn guard_prefix_prunes_bindings_and_keeps_argument_dependent_presence_order() {
    use idealis_model::reference::{Outcome, SearchBudget, analyze_instances_with_budget, witness};
    let source = "model GuardOrder
        enum Choice = Run | Skip
        record Amount { units: Nat }
        var saved: lone Amount
        var accepted: one Bool
        init { no saved not accepted }
        behavior run(choice: Choice) {
          guard { some saved, (choice = Run and saved.units > 0) }
          update { accepted = true }
        }
        reachable Accepted { accepted }
        scope Int = 0..1, steps = 3";
    let model = idealis_model::parse(source).unwrap();
    let (report, calls) = analyze_instances_with_budget(
        &model,
        "run",
        SearchBudget {
            states: 4,
            arguments: 0,
        },
        2,
    )
    .unwrap();
    assert_eq!(report.scope, Outcome::Holds);
    assert_eq!(report.verdicts[0].outcome, Outcome::Unreachable);
    assert_eq!(
        calls.iter().map(|call| call.enabled).collect::<Vec<_>>(),
        [Some(false), Some(false)]
    );

    let guarded = source
        .replace("some saved, (choice = Run and saved.units > 0)", "(choice = Run and some saved) and saved.units > 0")
        .replace("reachable Accepted", "behavior store() { guard { no saved } update { saved = Amount { units: 1 } } } reachable Accepted");
    let guarded = idealis_model::parse(&guarded).unwrap();
    let witness = witness(&guarded, "Accepted", 100).unwrap();
    assert!(witness.complete);
    let commands = witness.witness.unwrap().commands;
    assert_eq!(
        commands
            .iter()
            .map(|command| command.action.as_str())
            .collect::<Vec<_>>(),
        ["store", "run"]
    );
}

#[test]
fn one_refines_a_derived_row_to_an_integer_index() {
    let source =
        include_str!("../../../examples/model-patterns/sqlite-transactions/savepoint-named.ideal");
    let model = idealis_model::parse(source).unwrap();
    let report = idealis_model::reference::analyze_actions(&model, 20_000);
    assert_eq!(
        report.declarations,
        idealis_model::reference::Outcome::Holds
    );
    let name = "RewindKeepsTheMostRecentMatchingMark";
    assert_eq!(
        report
            .verdicts
            .iter()
            .find(|verdict| verdict.check == name)
            .unwrap()
            .outcome,
        idealis_model::reference::Outcome::Holds
    );
    for guard in ["some label.latest", "not one label.latest"] {
        let without_proof = source.replace("one label.latest", guard);
        assert!(
            idealis_model::parse(&without_proof)
                .unwrap_err()
                .message
                .contains("one integer index")
        );
    }
}
