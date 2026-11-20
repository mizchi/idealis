use idealis_core::Engine;
use serde_json::{Value, json};

fn request(engine: &mut Engine, request: Value) -> Value {
    serde_json::to_value(
        engine.dispatch(&serde_json::to_vec(&json!({"version":1,"request":request})).unwrap()),
    )
    .unwrap()
}

#[test]
fn prepared_handles_and_batch_preserve_each_one_shot_result() {
    let mut engine = Engine::default();
    let source = "---- MODULE Prepared ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nSafe == x < 3\n====";
    for version in [1, 2] {
        let compiled = request(
            &mut engine,
            json!({"kind":"compile-tla","source":source,"config":"INIT Init\nNEXT Next\nINVARIANT Safe","irVersion":version}),
        );
        assert_eq!(compiled["kind"], "compiled");
        let program = &compiled["program"];
        let prepared = request(
            &mut engine,
            json!({"kind":"prepare-safety","program":program}),
        );
        assert_eq!(prepared["kind"], "safety-prepared", "{prepared}");
        let handle = &prepared["handle"];
        let jobs: Vec<_> = [1, 10000, 0, 2, 10000]
            .into_iter()
            .map(|max_work| json!({"bounds":{"maxDepth":10,"maxStates":100,"maxWork":max_work}}))
            .collect();
        let expected: Vec<_> = jobs
            .iter()
            .map(|job| {
                request(
                    &mut engine,
                    json!({"kind":"check-safety","program":program,"bounds":job["bounds"]}),
                )
            })
            .collect();
        for (job, expected) in jobs.iter().zip(&expected) {
            assert_eq!(
                request(
                    &mut engine,
                    json!({"kind":"check-prepared-safety","handle":handle,"bounds":job["bounds"]})
                ),
                *expected
            );
        }
        let batch = request(
            &mut engine,
            json!({"kind":"check-prepared-safety-batch","handle":handle,"jobs":jobs}),
        );
        assert_eq!(batch, json!({"kind":"safety-batch","results":expected}));
        for jobs in [json!([]), json!(vec![jobs[0].clone(); 257])] {
            assert_eq!(
                request(
                    &mut engine,
                    json!({"kind":"check-prepared-safety-batch","handle":handle,"jobs":jobs})
                )["kind"],
                "error"
            );
        }
        let mut other = Engine::default();
        assert_eq!(
            request(
                &mut other,
                json!({"kind":"check-prepared-safety","handle":handle,"bounds":jobs[0]["bounds"]})
            )["kind"],
            "error"
        );
        assert_eq!(
            request(
                &mut engine,
                json!({"kind":"release-safety","handle":handle})
            ),
            json!({"kind":"released"})
        );
        for kind in ["check-prepared-safety", "release-safety"] {
            let mut command = json!({"kind":kind,"handle":handle});
            if kind == "check-prepared-safety" {
                command["bounds"] = jobs[0]["bounds"].clone();
            }
            assert_eq!(request(&mut engine, command)["kind"], "error");
        }
    }
}
