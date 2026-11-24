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
