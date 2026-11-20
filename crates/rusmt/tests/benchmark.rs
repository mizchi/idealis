use rusmt::{Problem, Script, Solver};

#[test]
fn criterion_fixtures_exercise_complete_queries_and_restored_contexts() {
    let source = include_str!("../benches/fixtures/repeat-64.smt2");
    let script = Script::parse(source).unwrap();
    let results = script.solve(2_000_000);
    assert_eq!(results.len(), 64);
    for result in results {
        assert_eq!(result.status, "sat");
        assert_eq!(result.values["x31"], "31");
    }
    let prefix = source.split_once("(check-sat)").unwrap().0;
    let mut context = Solver::parse(prefix).unwrap();
    for _ in 0..32 {
        context.push(1).unwrap();
        context.assert("(<= x31 31)").unwrap();
        context.pop(1).unwrap();
    }
    assert_eq!(context.check(10000).values["x31"], "31");
    let results = Script::parse(include_str!("../benches/fixtures/scopes-32.smt2"))
        .unwrap()
        .solve(2_000_000);
    assert_eq!(results.len(), 64);
    for (index, result) in results.iter().enumerate() {
        assert_eq!(result.status, if index % 2 == 0 { "unsat" } else { "sat" });
    }
    let results = Script::parse(include_str!("../benches/fixtures/ite-64.smt2"))
        .unwrap()
        .solve(2_000_000);
    assert_eq!(results.len(), 64);
    for (index, result) in results.iter().enumerate() {
        assert_eq!(result.status, "sat");
        if index % 3 != 2 {
            assert_eq!(result.values["x"], if index % 3 == 0 { "3" } else { "4" });
        }
    }
    assert_eq!(
        Problem::parse(include_str!("../benches/fixtures/integer-ite.smt2"))
            .unwrap()
            .solve(10000)
            .status,
        "sat"
    );
}
