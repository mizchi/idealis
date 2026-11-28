use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output};

fn invoke(args: &[&str], status: i32) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(status),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn external_worker_lasso_is_replayed_and_checked_against_each_fairness_scope() {
    let root = std::env::temp_dir().join(format!("rcheck-external-lasso-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let model = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/model-patterns/worker-fairness/competitive.ideal");
    let model = model.to_str().unwrap();
    let witness: Value =
        serde_json::from_slice(&invoke(&["witness", model, "InstanceWeak"], 0).stdout).unwrap();
    assert_eq!(witness["verdict"], "violated");
    let request = root.join("request.json");
    let reply = root.join("reply.json");
    let select = |name| {
        let output = invoke(&["checker-request", model, name], 0);
        std::fs::write(&request, &output.stdout).unwrap();
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["requestId"].clone()
    };
    let verify = |body: &Value, status| {
        std::fs::write(&reply, body.to_string()).unwrap();
        invoke(
            &[
                "checker-verify",
                request.to_str().unwrap(),
                reply.to_str().unwrap(),
            ],
            status,
        )
    };
    let depth = witness["trace"]["commands"].as_array().unwrap().len();
    let mut body = json!({
        "version":1,"requestId": select("InstanceWeak"), "verdict":"violated",
        "coverage":{"kind":"bounded","steps":depth},"witness":witness["trace"],
    });
    let report: Value = serde_json::from_slice(&verify(&body, 0).stdout).unwrap();
    assert_eq!(report["status"], "verified");
    assert_eq!(report["loopStart"], body["witness"]["loopStart"]);
    body["coverage"]["steps"] = json!(depth - 1);
    assert!(
        String::from_utf8_lossy(&verify(&body, 2).stderr).contains("outside declared coverage")
    );
    body["coverage"] = json!({"kind":"complete"});
    let mut wrong_loop = body.clone();
    wrong_loop["witness"]["loopStart"] = json!(0);
    assert!(String::from_utf8_lossy(&verify(&wrong_loop, 2).stderr).contains("does not close"));
    body["requestId"] = select("GroupStrong");
    verify(&body, 0);
    body["requestId"] = select("InstanceStrong");
    assert!(String::from_utf8_lossy(&verify(&body, 2).stderr).contains("fairness"));

    let parsed = idealis_model::parse(&std::fs::read_to_string(model).unwrap()).unwrap();
    let tiny = idealis_check::conform::external::request(
        &parsed,
        "InstanceWeak",
        idealis_check::Budget {
            work: 0,
            ..idealis_check::Budget::default()
        },
    )
    .unwrap();
    let tiny = serde_json::to_value(tiny).unwrap();
    std::fs::write(&request, tiny.to_string()).unwrap();
    body["requestId"] = tiny["requestId"].clone();
    let report: Value = serde_json::from_slice(&verify(&body, 3).stdout).unwrap();
    assert_eq!(report["status"], "reported");
    assert!(
        report["verificationReason"]
            .as_str()
            .unwrap()
            .contains("work budget")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn temporal_evidence_checks_the_prefix_and_interprets_fairness_as_state_pairs() {
    let root = std::env::temp_dir().join(format!(
        "rcheck-external-lasso-pairs-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let model = root.join("flags.model");
    std::fs::write(
        &model,
        r#"model Flags
var on: one Bool
init { not on }
behavior flip() { update { on = if { on } then false else true } }
behavior alias() { update { on = if { on } then false else true } }
behavior wait() {}
eventually Never with { weak flip } { false }
eventually On { on }
eventually Stalled with { strong wait } { on }
leads_to Responds { not on } { on }
scope steps = 4
"#,
    )
    .unwrap();
    let request = root.join("request.json");
    let reply = root.join("reply.json");
    let verify = |name, trace, status| {
        let output = invoke(&["checker-request", model.to_str().unwrap(), name], 0);
        std::fs::write(&request, &output.stdout).unwrap();
        let header: Value = serde_json::from_slice(&output.stdout).unwrap();
        std::fs::write(&reply, json!({"version":1,"requestId":header["requestId"],"verdict":"violated","coverage":{"kind":"complete"},"witness":trace}).to_string()).unwrap();
        invoke(
            &[
                "checker-verify",
                request.to_str().unwrap(),
                reply.to_str().unwrap(),
            ],
            status,
        )
    };
    let alias = json!({"config":{},"initial":{"on":false},"commands":[{"kind":"alias"},{"kind":"alias"}],"loopStart":0,"finalState":{"on":false}});
    verify("Never", alias, 0);
    let stutter = json!({"config":{},"initial":{"on":false},"commands":[null],"loopStart":0,"finalState":{"on":false}});
    verify("Stalled", stutter.clone(), 0);
    assert!(String::from_utf8_lossy(&verify("Never", stutter, 2).stderr).contains("fairness"));
    let responded = json!({"config":{},"initial":{"on":false},"commands":[{"kind":"flip"},null],"loopStart":1,"finalState":{"on":true}});
    for name in ["On", "Responds"] {
        assert!(
            String::from_utf8_lossy(&verify(name, responded.clone(), 2).stderr)
                .contains("does not violate")
        );
    }
    let prefix = json!({"config":{},"initial":{"on":false},"commands":[{"kind":"flip"},{"kind":"flip"},null],"loopStart":2,"finalState":{"on":false}});
    assert!(
        String::from_utf8_lossy(&verify("On", prefix.clone(), 2).stderr)
            .contains("does not violate")
    );
    verify("Responds", prefix, 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn external_claims_are_bound_to_the_request_and_concrete_evidence_is_replayed() {
    let directory = std::env::temp_dir().join(format!("rcheck-external-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let model = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/rcheck/ir-compat/pkl-bank-v1.json");
    let request_path = directory.join("request.json");
    let reply_path = directory.join("reply.json");
    let request = invoke(
        &[
            "checker-request",
            model.to_str().unwrap(),
            "Rich",
            "--max-states",
            "100",
        ],
        0,
    );
    std::fs::write(&request_path, &request.stdout).unwrap();
    let request_bytes = request.stdout;
    let request: Value = serde_json::from_slice(&request_bytes).unwrap();
    assert_eq!(request["ir"]["version"], 1);
    assert_eq!(request["budget"]["states"], 100);
    assert_eq!(request["question"]["name"], "Rich");

    let verify = |reply: &Value, status| {
        std::fs::write(&reply_path, reply.to_string()).unwrap();
        invoke(
            &[
                "checker-verify",
                request_path.to_str().unwrap(),
                reply_path.to_str().unwrap(),
            ],
            status,
        )
    };
    let mut reply = json!({
        "version":1, "requestId": request["requestId"],
        "verdict":"reachable", "coverage":{"kind":"bounded","steps":2},
        "witness":{
            "config":{}, "initial":{"balance":{"Account0":0,"Account1":0}},
            "commands":[{"kind":"deposit","a":"Account0","n":3}],
            "finalState":{"balance":{"Account0":3,"Account1":0}}
        }
    });
    let report: Value = serde_json::from_slice(&verify(&reply, 0).stdout).unwrap();
    assert_eq!(report["status"], "verified");
    assert_eq!(report["depth"], 1);
    assert_eq!(report["verdict"], "reachable");
    let mut edited_request = request.clone();
    edited_request["budget"]["states"] = json!(101);
    std::fs::write(&request_path, edited_request.to_string()).unwrap();
    assert!(
        String::from_utf8_lossy(&verify(&reply, 2).stderr)
            .contains("requestId does not match the request content")
    );
    std::fs::write(&request_path, &request_bytes).unwrap();

    let mut alternative = reply.clone();
    alternative["coverage"]["steps"] = json!(1);
    assert!(
        String::from_utf8_lossy(&verify(&alternative, 2).stderr)
            .contains("outside declared coverage")
    );
    alternative = reply.clone();
    alternative["requestId"] = json!("other");
    assert!(String::from_utf8_lossy(&verify(&alternative, 2).stderr).contains("reply requestId"));
    alternative = reply.clone();
    alternative["witness"]["finalState"]["balance"]["Account0"] = json!(4);
    assert!(
        String::from_utf8_lossy(&verify(&alternative, 2).stderr)
            .contains("final state does not match")
    );
    alternative = reply.clone();
    alternative["witness"]["commands"][0]["n"] = json!(0);
    assert!(String::from_utf8_lossy(&verify(&alternative, 2).stderr).contains("guard is false"));

    reply["witness"]["commands"][0]["n"] = json!(1);
    reply["witness"]["finalState"]["balance"]["Account0"] = json!(1);
    assert!(String::from_utf8_lossy(&verify(&reply, 2).stderr).contains("does not establish Rich"));

    for (verdict, fields) in [
        ("unsupported", json!({"reason":"numeric fragment"})),
        (
            "unknown",
            json!({"coverage":{"kind":"complete"},"reason":"state budget"}),
        ),
        ("unreachable", json!({"coverage":{"kind":"complete"}})),
    ] {
        let mut response = json!({"version":1,"requestId":request["requestId"],"verdict":verdict});
        response
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        let report: Value = serde_json::from_slice(&verify(&response, 3).stdout).unwrap();
        assert_eq!(report["status"], "reported");
        assert_eq!(report["verdict"], verdict);
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn finite_violations_are_checked_at_the_final_state_or_named_edge() {
    let directory =
        std::env::temp_dir().join(format!("rcheck-external-edge-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let model = directory.join("flag.model");
    std::fs::write(
        &model,
        r#"model Flag
var flag: one Bool
init { not flag }
behavior setFlag() { update { flag = true } }
behavior clearFlag() { update { flag = false } }
behavior eraseFlag() { update { flag = none } }
invariant AlwaysClear { not flag }
transition NeverAssign on setFlag { unchanged(flag) }
scope steps = 4
"#,
    )
    .unwrap();
    let request_path = directory.join("request.json");
    let reply_path = directory.join("reply.json");
    for name in ["AlwaysClear", "NeverAssign"] {
        let request = invoke(&["checker-request", model.to_str().unwrap(), name], 0);
        std::fs::write(&request_path, &request.stdout).unwrap();
        let request: Value = serde_json::from_slice(&request.stdout).unwrap();
        let mut reply = json!({
            "version":1, "requestId":request["requestId"], "verdict":"violated", "coverage":{"kind":"complete"},
            "witness":{"config":{}, "initial":{"flag":false}, "commands":[{"kind":"setFlag"}], "finalState":{"flag":true}}
        });
        let verify = |reply: &Value, status| {
            std::fs::write(&reply_path, reply.to_string()).unwrap();
            invoke(
                &[
                    "checker-verify",
                    request_path.to_str().unwrap(),
                    reply_path.to_str().unwrap(),
                ],
                status,
            )
        };
        assert_eq!(
            serde_json::from_slice::<Value>(&verify(&reply, 0).stdout).unwrap()["status"],
            "verified"
        );
        reply["witness"]["commands"]
            .as_array_mut()
            .unwrap()
            .push(json!({"kind":"clearFlag"}));
        reply["witness"]["finalState"]["flag"] = json!(false);
        assert!(
            String::from_utf8_lossy(&verify(&reply, 2).stderr)
                .contains(&format!("does not establish {name}"))
        );
    }
    let request = invoke(
        &["checker-request", model.to_str().unwrap(), "Declarations"],
        0,
    );
    std::fs::write(&request_path, &request.stdout).unwrap();
    let request: Value = serde_json::from_slice(&request.stdout).unwrap();
    let erased = json!({
        "version":1, "requestId":request["requestId"], "verdict":"violated", "coverage":{"kind":"complete"},
        "witness":{"config":{}, "initial":{"flag":false}, "commands":[{"kind":"eraseFlag"}], "finalState":{"flag":null}}
    });
    std::fs::write(&reply_path, erased.to_string()).unwrap();
    let checked = invoke(
        &[
            "checker-verify",
            request_path.to_str().unwrap(),
            reply_path.to_str().unwrap(),
        ],
        0,
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&checked.stdout).unwrap()["status"],
        "verified"
    );
    std::fs::remove_dir_all(directory).unwrap();
}
