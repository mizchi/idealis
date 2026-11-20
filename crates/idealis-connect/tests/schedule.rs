use idealis_connect::{Engine, ErrorKind, Generator, Schedule, ScheduleActor};
use serde_json::{Value, json};

fn schedule(actors: usize) -> Schedule {
    Schedule {
        id: "increments".into(),
        version: "1".into(),
        actors: (0..actors)
            .map(|actor| ScheduleActor {
                id: format!("actor-{actor}"),
                operations: vec![json!("read"), json!("write")],
            })
            .collect(),
    }
}
fn complete(generator: &Generator, key: &str, prefix: Vec<Value>) -> Vec<Vec<Value>> {
    if generator.states[key].is_empty() {
        return vec![prefix];
    }
    generator.states[key]
        .iter()
        .flat_map(|edge| {
            let mut actions = prefix.clone();
            actions.push(edge.action.clone());
            complete(generator, &edge.target, actions)
        })
        .collect()
}

#[test]
fn preserves_each_actor_order_and_all_complete_interleavings() {
    for (actors, paths) in [(2, 6), (3, 90)] {
        let generator = schedule(actors).generator().unwrap();
        assert_eq!(generator.states.len(), 3_usize.pow(actors as u32));
        assert_eq!(generator.min_length, 0);
        assert_eq!(generator.max_length, actors * 2);
        let orders = complete(&generator, &generator.initial[0], vec![]);
        assert_eq!(orders.len(), paths);
        for order in orders {
            for actor in 0..actors {
                let steps: Vec<_> = order
                    .iter()
                    .filter(|action| action["actor"] == format!("actor-{actor}"))
                    .collect();
                assert_eq!(
                    steps,
                    vec![
                        &json!({"actor":format!("actor-{actor}"),"step":0,"operation":"read"}),
                        &json!({"actor":format!("actor-{actor}"),"step":1,"operation":"write"})
                    ]
                );
            }
        }
    }
}

#[test]
fn validates_identity_payload_and_product_before_expanding() {
    assert_eq!(
        schedule(0).generator().unwrap_err().kind,
        ErrorKind::Protocol
    );
    assert_eq!(
        schedule(9).generator().unwrap_err().kind,
        ErrorKind::Resource
    );
    let mut duplicate = schedule(2);
    duplicate.actors[1].id = duplicate.actors[0].id.clone();
    assert_eq!(duplicate.generator().unwrap_err().kind, ErrorKind::Protocol);
    let mut large = schedule(2);
    for actor in &mut large.actors {
        actor.operations = vec![json!("op"); 32];
    }
    assert_eq!(large.generator().unwrap_err().kind, ErrorKind::Resource);
    let mut payload = schedule(1);
    payload.actors[0].operations = vec![json!(0.5)];
    assert_eq!(payload.generator().unwrap_err().kind, ErrorKind::Protocol);
    let mut empty = schedule(1);
    empty.actors[0].operations.clear();
    let generator = empty.generator().unwrap();
    assert_eq!(generator.states.len(), 1);
    assert_eq!(generator.max_length, 0);
    let mut maximum = schedule(1);
    maximum.actors[0].operations = vec![json!(null); 128];
    assert_eq!(maximum.clone().generator().unwrap().max_length, 128);
    maximum.actors[0].operations.push(json!(null));
    assert_eq!(maximum.generator().unwrap_err().kind, ErrorKind::Resource);
}

#[test]
fn wire_compilation_and_errors_preserve_pending_property_feedback() {
    let input = schedule(2);
    let generator = input.clone().generator().unwrap();
    let mut engine = Engine::default();
    let start = engine.dispatch(&serde_json::to_vec(&json!({"version":1,"request":{"kind":"property","plan":{"generator":generator,"seed":1,"cases":1,"maxAttempts":1,"maxShrinkChecks":16,"maxWork":1000}}})).unwrap());
    assert_eq!(start["effect"]["kind"], "evaluate");
    let compiled = engine.dispatch(
        &serde_json::to_vec(&json!({"version":1,"request":{"kind":"schedule","schedule":input}}))
            .unwrap(),
    );
    assert_eq!(compiled["kind"], "generator");
    assert_eq!(
        compiled["generator"],
        serde_json::to_value(generator).unwrap()
    );
    assert_eq!(engine.dispatch(br#"{"version":1,"request":{"kind":"schedule","schedule":{"id":"x","version":"1","actors":[]}}}"#)["kind"], "error");
    let done = engine.dispatch(&serde_json::to_vec(&json!({"version":1,"request":{"kind":"resume","id":start["effect"]["id"],"verdict":{"status":"matched"}}})).unwrap());
    assert_eq!(done["effect"]["result"]["status"], "passed");
}
