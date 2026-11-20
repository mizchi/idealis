use rusmt::Problem;

fn solve(source: &str) -> serde_json::Value {
    serde_json::to_value(Problem::parse(source).unwrap().solve(1_000_000)).unwrap()
}

#[test]
fn unbounded_difference_constraints_have_exact_witnesses() {
    let result = solve(
        "(declare-const x Int)(declare-const y Int)(assert (= x 10000000000000000000000000000000000000000))(assert (= y (+ x 7)))",
    );
    assert_eq!(result["status"], "sat");
    assert_eq!(
        result["values"]["y"],
        "10000000000000000000000000000000000000007"
    );
    assert_eq!(
        solve("(declare-const x Int)(declare-const y Int)(assert (< x y))(assert (<= y x))")["status"],
        "unsat"
    );
}

#[test]
fn existing_integer_input_generation_is_supported() {
    let result = solve(
        "(set-logic QF_LIA)(declare-const amount Int)(declare-const other Int)(assert (and (> amount 0)(< amount 10)(= other 3)(= (+ amount other) 10)))",
    );
    assert_eq!(result["status"], "sat");
    assert_eq!(result["values"]["amount"], "7");
}

#[test]
fn boolean_polarity_and_let_scope_are_preserved() {
    assert_eq!(
        solve(
            "(declare-const b Bool)(declare-const x Int)(assert (and (or b (= x 0))(not b)(not (= x 0))))"
        )["status"],
        "unsat"
    );
    assert_eq!(
        solve("(declare-const x Int)(assert (let ((x 2)(y x)) (and (= x 2)(= y 7))))")["values"]["x"],
        "7"
    );
    assert_eq!(
        solve("(declare-const b Bool)(assert (xor b (not b)))")["status"],
        "sat"
    );
}

#[test]
fn budget_cuts_and_unsupported_theories_are_not_unsat() {
    let problem = Problem::parse("(declare-const x Int)(assert (> x 0))").unwrap();
    assert_eq!(
        serde_json::to_value(problem.solve(0)).unwrap()["status"],
        "unknown"
    );
    for input in [
        "(declare-const x Int)(assert (= (* x x) 2))",
        "(declare-const x Real)",
        "(assert (= true 0))",
        "(assert true)(check-sat)(assert false)",
        "(push 1)",
        "(assert unknown)",
        "(assert (forall ((x Int)) (= x x)))",
    ] {
        assert!(Problem::parse(input).is_err(), "{input}");
    }
}

#[test]
fn empty_boolean_applications_are_syntax_errors() {
    for source in ["(assert (and))", "(assert (or))"] {
        assert!(Problem::parse(source).is_err(), "{source}");
    }
    assert_eq!(solve("(assert (and true))")["status"], "sat");
    assert_eq!(solve("(assert (or false))")["status"], "unsat");
}
