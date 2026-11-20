use idealis_schedule_eval::dispatch;
use serde_json::json;

#[test]
fn gated_real_threads_reproduce_lost_update_and_fixed_control() {
    let actions = json!([
        {"actor":"actor-0","step":0,"operation":"read"},
        {"actor":"actor-1","step":0,"operation":"read"},
        {"actor":"actor-0","step":1,"operation":"write"},
        {"actor":"actor-1","step":1,"operation":"write"}
    ]);
    for (broken, expected, status) in [(true, 1, "mismatch"), (false, 2, "matched")] {
        let response = dispatch(
            &serde_json::to_vec(&json!({"actors":2,"actions":actions,"broken":broken})).unwrap(),
        );
        assert_eq!(response["actual"], expected);
        assert_eq!(response["expected"], 2);
        assert_eq!(response["verdict"]["status"], status);
        assert_eq!(response["observations"], json!([0, 0, 1, expected]));
    }
}

#[test]
fn empty_prefix_and_invalid_actor_order_do_not_deadlock() {
    assert_eq!(
        dispatch(br#"{"actors":2,"actions":[],"broken":true}"#)["verdict"]["status"],
        "matched"
    );
    for action in [
        json!({"actor":"actor-0","step":1,"operation":"write"}),
        json!({"actor":"actor-9","step":0,"operation":"read"}),
    ] {
        assert_eq!(
            dispatch(
                &serde_json::to_vec(&json!({"actors":2,"actions":[action],"broken":true})).unwrap()
            )["verdict"]["status"],
            "invalid"
        );
    }
}
