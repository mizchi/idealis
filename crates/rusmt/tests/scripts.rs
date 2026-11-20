use rusmt::{Engine, Script};
use serde_json::{Value, json};

fn solve(source: &str, budget: u32) -> Value {
    serde_json::to_value(Script::parse(source).unwrap().solve(budget)).unwrap()
}

#[test]
fn scripts_restore_scopes_and_temporary_assumptions_in_order() {
    let results = solve(
        "(declare-const x Int)(declare-const b Bool)
         (assert (= x (ite b 3 4)))(check-sat-assuming (b))
         (push 1)(assert (= x 3))(check-sat-assuming ((not b)))
         (push 1)(assert (= x 4))(check-sat)(pop 2)
         (check-sat-assuming ((not b)))(check-sat-assuming (b (not b)))
         (check-sat)(push 0)(pop 0)(check-sat-assuming ())",
        100_000,
    );
    assert_eq!(results[0]["values"]["x"], "3");
    assert_eq!(results[1]["status"], "unsat");
    assert_eq!(results[2]["status"], "unsat");
    assert_eq!(results[3]["values"]["x"], "4");
    assert_eq!(results[4]["status"], "unsat");
    assert_eq!(results[5]["status"], "sat");
    assert_eq!(results[6]["status"], "sat");
}

#[test]
fn work_is_shared_by_all_checks_and_repeated_solves_start_fresh() {
    let script =
        Script::parse("(declare-const x Int)(assert (= x 7))(check-sat)(check-sat)(check-sat)")
            .unwrap();
    let full = serde_json::to_value(script.solve(10000)).unwrap();
    let first = u32::try_from(full[0]["stats"]["work"].as_u64().unwrap()).unwrap();
    assert!(first > 0);
    let bounded = serde_json::to_value(script.solve(first)).unwrap();
    assert_eq!(bounded[0]["status"], "sat");
    for index in [1, 2] {
        assert_eq!(bounded[index]["status"], "unknown");
        assert_eq!(bounded[index]["reason"], "work");
        assert_eq!(bounded[index]["values"], json!({}));
        assert_eq!(bounded[index]["stats"]["work"], 0);
    }
    assert_eq!(serde_json::to_value(script.solve(10000)).unwrap(), full);
    assert_eq!(solve("", 0), json!([]));
    assert_eq!(solve("(declare-const x Int)", 10), json!([]));
}

#[test]
fn invalid_later_commands_and_resource_overflows_are_errors() {
    for source in [
        "(check-sat)(pop 1)",
        "(assert true)(declare-const x Int)",
        "(push 1)(declare-const x Int)",
        "(check-sat)(set-option :produce-models true)",
        "(push -1)",
        "(push 01)",
        "(push 4294967296)",
        "(push |1|)",
        "(push 257)",
        "(push 256)(push 1)",
        "(check-sat 1)",
        "(check-sat-assuming)",
        "(check-sat-assuming (true))",
        "(declare-const x Int)(check-sat-assuming (x))",
        "(declare-const b Bool)(check-sat-assuming ((not (not b))))",
        "(declare-const b Bool)(check-sat-assuming ((and b b)))",
        "(assert (= (* x x) 2))(pop 0)",
        "(get-model)",
        "(exit)",
    ] {
        assert!(Script::parse(source).is_err(), "{source}");
    }
    assert_eq!(
        Script::parse(&"(check-sat)".repeat(257))
            .err()
            .unwrap()
            .code,
        "resource"
    );
    let source = format!(
        "(declare-const b Bool)(check-sat-assuming ({}))",
        "b ".repeat(257)
    );
    assert_eq!(Script::parse(&source).err().unwrap().code, "resource");
    assert_eq!(
        solve("(push 256)(check-sat)(pop 256)(check-sat)", 10000)[1]["status"],
        "sat"
    );
    assert_eq!(
        solve(&"(check-sat)".repeat(256), 10000)
            .as_array()
            .unwrap()
            .len(),
        256
    );
}

#[test]
fn quoted_symbols_and_json_batch_errors_preserve_existing_handles() {
    let results = solve(
        "; comment\n(declare-const |a b| Bool)(declare-const |x y| Int)(assert (= |x y| (ite |a b| 5 6)))(check-sat-assuming (|a b|))(check-sat-assuming ((not |a b|)))",
        10000,
    );
    assert_eq!(results[0]["values"]["x y"], "5");
    assert_eq!(results[1]["values"]["x y"], "6");
    let mut engine = Engine::default();
    let mut call = |request: Value| {
        engine.dispatch(&serde_json::to_vec(&json!({"version":1,"request":request})).unwrap())
    };
    let handle =
        call(json!({"kind":"prepare","source":"(declare-const x Int)(assert (= x 9))"}))["handle"]
            .clone();
    for budget in [0, 10000] {
        assert_eq!(
            call(json!({"kind":"run-script","source":"(check-sat)(pop 1)","maxWork":budget}))["kind"],
            "error"
        );
        assert_eq!(
            call(json!({"kind":"check","handle":handle,"maxWork":10000}))["result"]["values"]["x"],
            "9"
        );
    }
    let reply = call(
        json!({"kind":"run-script","source":"(check-sat)(assert false)(check-sat)","maxWork":10000}),
    );
    assert_eq!(reply["kind"], "script-results");
    assert_eq!(reply["results"][0]["status"], "sat");
    assert_eq!(reply["results"][1]["status"], "unsat");
}
