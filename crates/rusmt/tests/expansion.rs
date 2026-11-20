use rusmt::{Assumption, Engine, Problem, Solver};
use serde_json::{Value, json};

fn solve(source: &str) -> Value {
    serde_json::to_value(Problem::parse(source).unwrap().solve(1_000_000)).unwrap()
}
#[test]
fn scaled_constraints_use_integer_floor_for_both_signs() {
    for (assertions, status) in [
        ("(assert (<= (* 2 x) 7))(assert (> x 3))", "unsat"),
        ("(assert (<= (* 2 x) (- 3)))(assert (>= x (- 1)))", "unsat"),
        ("(assert (<= (* 2 x) (- 3)))(assert (>= x (- 2)))", "sat"),
        ("(assert (< (* (- 3) x) 4))(assert (<= x (- 2)))", "unsat"),
        ("(assert (= (* 2 x) 7))", "unsat"),
        ("(assert (= (* 0 x) 0))", "sat"),
        (
            "(assert (<= (* 3 (- x y)) (- 4)))(assert (>= (- x y) (- 1)))",
            "unsat",
        ),
        (
            "(assert (>= (* 10000000000000000000000000000000000000000 x) 1))(assert (<= x 0))",
            "unsat",
        ),
    ] {
        assert_eq!(
            solve(&format!(
                "(declare-const x Int)(declare-const y Int){assertions}"
            ))["status"],
            status,
            "{assertions}"
        );
    }
}
#[test]
fn integer_ite_composes_with_let_arithmetic_and_boolean_polarity() {
    for (condition, expected) in [("b", "2"), ("(not b)", "4")] {
        let result = solve(&format!(
            "(declare-const b Bool)(declare-const x Int)(assert {condition})(assert (let ((v (ite b (+ x 1) (- x 1)))) (= (* 2 v) 6)))"
        ));
        assert_eq!(result["status"], "sat");
        assert_eq!(result["values"]["x"], expected);
    }
    assert_eq!(
        solve(
            "(declare-const a Bool)(declare-const b Bool)(declare-const x Int)(assert a)(assert (not b))(assert (= x (ite a (ite b 1 2) (ite b 3 4))))"
        )["values"]["x"],
        "2"
    );
    assert_eq!(
        solve("(declare-const b Bool)(assert (not (> (ite b 5 6) 3)))")["status"],
        "unsat"
    );
}
#[test]
fn unsupported_nonlinear_and_excessive_piecewise_terms_remain_errors() {
    for source in [
        "(declare-const x Int)(declare-const y Int)(assert (= (* x y) 2))",
        "(declare-const x Int)(declare-const y Int)(assert (<= (+ (* 2 x) (* 3 y)) 7))",
        "(assert (= (ite true 1 false) 1))",
    ] {
        assert!(Problem::parse(source).is_err(), "{source}");
    }
    let declarations = (0..9)
        .map(|index| format!("(declare-const b{index} Bool)"))
        .collect::<String>();
    let terms = (0..9)
        .map(|index| format!("(ite b{index} 1 0)"))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(
        Problem::parse(&format!("{declarations}(assert (<= (+ {terms}) 5))"))
            .err()
            .unwrap()
            .code,
        "resource"
    );
}
#[test]
fn scopes_and_temporary_assumptions_never_leak() {
    let mut solver =
        Solver::parse("(declare-const x Int)(declare-const b Bool)(assert (>= x 0))(check-sat)")
            .unwrap();
    solver.push(1).unwrap();
    solver.assert("(= x 1)").unwrap();
    assert_eq!(solver.check(10000).values["x"], "1");
    solver.push(1).unwrap();
    solver.assert("(= x 2)").unwrap();
    assert_eq!(solver.check(10000).status, "unsat");
    assert!(solver.assert("true)(assert false").is_err());
    assert_eq!(solver.check(10000).status, "unsat");
    solver.pop(1).unwrap();
    assert_eq!(solver.check(10000).values["x"], "1");
    assert!(solver.pop(2).is_err());
    assert_eq!(solver.check(10000).values["x"], "1");
    let assumptions = [
        Assumption {
            symbol: "b".into(),
            positive: true,
        },
        Assumption {
            symbol: "b".into(),
            positive: false,
        },
    ];
    assert_eq!(
        solver.check_assuming(&assumptions, 10000).unwrap().status,
        "unsat"
    );
    assert_eq!(solver.check(10000).status, "sat");
    assert_eq!(
        solver.check_assuming(&assumptions[..1], 0).unwrap().status,
        "unknown"
    );
    assert!(
        solver
            .check_assuming(
                &[Assumption {
                    symbol: "x".into(),
                    positive: true
                }],
                10000
            )
            .is_err()
    );
    solver.pop(1).unwrap();
    solver.assert("(<= x 0)").unwrap();
    assert_eq!(solver.check(10000).values["x"], "0");
    solver.push(256).unwrap();
    assert!(solver.push(1).is_err());
    solver.pop(256).unwrap();
    assert_eq!(solver.check(10000).status, "sat");
}
#[test]
fn json_handles_support_atomic_context_edits_and_assumptions() {
    let mut engine = Engine::default();
    let mut call = |request: Value| {
        engine.dispatch(&serde_json::to_vec(&json!({"version":1,"request":request})).unwrap())
    };
    let handle = call(
        json!({"kind":"prepare","source":"(declare-const x Int)(declare-const b Bool)"}),
    )["handle"]
        .clone();
    assert_eq!(
        call(json!({"kind":"push","handle":handle,"count":1}))["kind"],
        "updated"
    );
    assert_eq!(
        call(json!({"kind":"assert","handle":handle,"expression":"(= x (ite b 1 2))"}))["kind"],
        "updated"
    );
    let result = call(
        json!({"kind":"check","handle":handle,"maxWork":10000,"assumptions":[{"symbol":"b","positive":true}]}),
    );
    assert_eq!(result["result"]["values"]["x"], "1");
    assert_eq!(
        call(json!({"kind":"pop","handle":handle,"count":2}))["kind"],
        "error"
    );
    assert_eq!(
        call(
            json!({"kind":"check","handle":handle,"maxWork":10000,"assumptions":[{"symbol":"b","positive":false}]})
        )["result"]["values"]["x"],
        "2"
    );
    assert_eq!(
        call(json!({"kind":"pop","handle":handle,"count":1}))["kind"],
        "updated"
    );
    assert_eq!(
        call(json!({"kind":"check","handle":handle,"maxWork":10000}))["result"]["status"],
        "sat"
    );
}

#[test]
fn malformed_assumptions_do_not_mutate_quoted_symbol_contexts() {
    let mut engine = Engine::default();
    let mut call = |request: Value| {
        engine.dispatch(&serde_json::to_vec(&json!({"version":1,"request":request})).unwrap())
    };
    let handle = call(json!({"kind":"prepare","source":"; comment\n(declare-const |has space| Bool)(declare-const |x y| Int)(check-sat)"}))["handle"].clone();
    assert_eq!(
        call(
            json!({"kind":"assert","handle":handle,"expression":"(= |x y| (ite |has space| 3 4)) ; trailing comment"})
        )["kind"],
        "updated"
    );
    for assumptions in [
        json!(null),
        json!([{"symbol":"has space"}]),
        json!([{"symbol":"has space","positive":"true"}]),
        json!([{"symbol":"has space","positive":true,"extra":false}]),
        json!(vec![json!({"symbol":"has space","positive":true}); 257]),
    ] {
        assert_eq!(
            call(json!({"kind":"check","handle":handle,"maxWork":10000,"assumptions":assumptions}))
                ["kind"],
            "error"
        );
        assert_eq!(
            call(
                json!({"kind":"check","handle":handle,"maxWork":10000,"assumptions":[{"symbol":"has space","positive":false}]})
            )["result"]["values"]["x y"],
            "4"
        );
    }
}
