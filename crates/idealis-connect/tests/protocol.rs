use idealis_connect::{Engine, ErrorKind, Generator, Plan, Session};
use serde_json::{Value, json};

fn request(engine: &mut Engine, request: Value) -> Value {
    engine.dispatch(&serde_json::to_vec(&json!({"version":1,"request":request})).unwrap())
}

#[test]
fn active_sessions_are_not_replaced_and_cancel_releases_them() {
    let plan = json!({"generator":{"id":"g","version":"1","initial":["s"],"states":{"s":[]},"minLength":0,"maxLength":0},"seed":1,"cases":1,"maxAttempts":1,"maxShrinkChecks":2,"maxWork":10});
    let mut engine = Engine::default();
    let pending = request(&mut engine, json!({"kind":"property","plan":plan}));
    assert_eq!(pending["effect"]["kind"], "evaluate");
    assert_eq!(
        request(&mut engine, json!({"kind":"property","plan":plan}))["kind"],
        "error"
    );
    assert_eq!(
        request(
            &mut engine,
            json!({"kind":"resume","id":2,"verdict":{"status":"matched"}})
        )["kind"],
        "error"
    );
    assert_eq!(
        request(
            &mut engine,
            json!({"kind":"resume","id":1,"verdict":{"status":"matched","extra":true}})
        )["kind"],
        "error"
    );
    let done = request(
        &mut engine,
        json!({"kind":"resume","id":1,"verdict":{"status":"matched"}}),
    );
    assert_eq!(done["effect"]["result"]["status"], "passed");
    assert_eq!(
        request(&mut engine, json!({"kind":"cancel"}))["kind"],
        "cancelled"
    );
    assert_eq!(
        request(&mut engine, json!({"kind":"property","plan":plan})),
        pending
    );
}

#[test]
fn malformed_generators_and_cases_fail_before_executing_the_sut() {
    let generator: Generator = serde_json::from_value(json!({"id":"g","version":"1","initial":["missing"],"states":{"s":[]},"minLength":0,"maxLength":0})).unwrap();
    assert_eq!(
        Session::property(Plan {
            generator,
            seed: 1,
            cases: 1,
            max_attempts: 1,
            max_shrink_checks: 2,
            max_work: 10
        })
        .err()
        .unwrap()
        .kind,
        ErrorKind::Protocol
    );
    let mut engine = Engine::default();
    for input in [
        json!({"version":2,"request":{"kind":"cancel"}}),
        json!({"version":1,"request":{"kind":"cancel","extra":true}}),
    ] {
        assert_eq!(
            engine.dispatch(&serde_json::to_vec(&input).unwrap())["error"]["kind"],
            "protocol"
        );
    }
}
