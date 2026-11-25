//! Plan 0103: integer, bool and record values. The reference evaluator and
//! rtlc on the TLA+ lowering give the verdicts written before running them;
//! a model whose integers leave the scope is unknown, never a pass.
use rmodel::reference::Outcome;

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
fn explicit(model: &rmodel::Model, name: &str) -> (&'static str, Option<usize>) {
    let run = |name: &str| {
        let (source, config) = rmodel::to_tla(model, name).unwrap();
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
        .any(|check| check.name == name && check.kind == rmodel::CheckKind::Reachable);
    if result.violation.is_some() {
        let verdict = if reachable { "reachable" } else { "violated" };
        return (verdict, Some(result.depth as usize));
    }
    match result.status.as_str() {
        "empty" => ("empty", None),
        "complete" => {
            if run(rmodel::SCOPE).violation.is_some() {
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
    let model = rmodel::parse(source).unwrap_or_else(|error| panic!("{error}"));
    let report = rmodel::reference::analyze(&model, 2_000_000);
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
    let model = rmodel::parse(source).unwrap();
    assert_eq!(rmodel::alloy_supports(&model), Ok(()));
    ralloy::Model::parse(&rmodel::to_alloy(&model)).unwrap();
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
    let model = rmodel::parse(source).unwrap();
    assert!(rmodel::alloy_supports(&model).is_err());
}

#[test]
fn ill_typed_values_are_rejected() {
    let base = "model M\nsig A\nenum C = X | Y\nrecord R { n: Int, c: C }\nvar b: A -> one Int\nvar l: A -> lone Int\n";
    let scope = "\nscope A = 2, Int = 0..3, steps = 2";
    for (body, needle) in [
        (
            "var xs: set Int\ninvariant I { some xs }",
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
            "invariant I { all n: Int | n >= 0 }",
            "only atoms are quantified",
        ),
        ("invariant I { some a: A | a.b }", "expression of Bool"),
    ] {
        let error = rmodel::parse(&format!("{base}{body}{scope}")).unwrap_err();
        assert!(error.message.contains(needle), "{body}: {error}");
    }
    let error = rmodel::parse(
        "model M\nsig A\nvar b: A -> one Int\ninvariant I { some b }\nscope A = 1, steps = 1",
    )
    .unwrap_err();
    assert!(error.message.contains("scope Int"), "{error}");
}

/// Found by the 0103 generator: a parenthesized integer before `<` was
/// read as a Bool formula.
#[test]
fn parenthesized_integers_start_comparisons() {
    let model = rmodel::parse(
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
    let model = rmodel::parse(
        "model NaturalCounter
         var n: one Nat
         init { n = 0 }
         behavior take(x: Nat) { guard { x = n + 1 } update { n = x } }
         invariant NonNegative { n >= 0 }
         reachable Two { n = 2 }
         scope Int = -2..2, steps = 3",
    )
    .unwrap();
    let report = rmodel::reference::analyze(&model, 1_000);
    assert_eq!(report.scope, rmodel::reference::Outcome::Holds);
    assert_eq!(
        report.verdicts[1].outcome,
        rmodel::reference::Outcome::Reachable { depth: 2 }
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
        rmodel::parse(source)
            .unwrap_err()
            .message
            .contains("arithmetic")
    );
}

#[test]
fn a_wide_exact_model_can_be_checked_by_the_reference_without_promising_tlc_support() {
    let model = rmodel::parse(
        "model Wide
         var n: one Int
         init { n = 2147483647 }
         behavior increment() { update { n = n + 1 } }
         invariant InRange { n <= 2147483647 }
         scope Int = 2147483646..2147483647, steps = 2",
    )
    .unwrap();
    let report = rmodel::reference::analyze(&model, 100);
    assert_eq!(
        report.scope,
        rmodel::reference::Outcome::Violated { depth: 1 }
    );
    let error = rmodel::to_tla(&model, "InRange").unwrap_err();
    assert!(error.message.contains("arithmetic"), "{error}");
}

#[test]
fn an_overflow_through_a_record_literal_projection_is_rejected() {
    let error = rmodel::parse(
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
    let model = rmodel::parse(
        "model Low
         var n: one Int
         init { n = -9223372036854775808 }
         invariant AtMin { n = -9223372036854775808 }
         scope Int = -9223372036854775808..-9223372036854775807, steps = 2",
    )
    .unwrap();
    let result = rmodel::reference::evaluate(&model, 100);
    assert_eq!(result[0].outcome, rmodel::reference::Outcome::Holds);
    assert!(rmodel::tla_supports(&model).is_err());
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
        rmodel::parse(&source)
            .unwrap_err()
            .message
            .contains("arithmetic")
    );
}
