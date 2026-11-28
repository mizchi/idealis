use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/model-patterns/offline-merge/two-stores/model.project.json")
}

fn run(path: &Path, before: &[&str], after: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .args(before)
        .arg(path)
        .args(after)
        .output()
        .unwrap()
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn public_bindings_filter_commands_and_the_full_model_still_replays() {
    let model = project();
    for (target, exported, internal) in [
        ("rust", "fn east_commit(", "fn eastDelivery_send("),
        (
            "typescript",
            "kind: \"east_commit\"",
            "kind: \"eastDelivery_send\"",
        ),
        (
            "go",
            "East_commit(request Request) bool",
            "EastDelivery_send(request Request) bool",
        ),
        (
            "moonbit",
            "fn east_commit(Self",
            "fn eastDelivery_send(Self",
        ),
    ] {
        let public = success(run(&model, &["generate"], &[target, "--public"]));
        assert!(public.contains(exported), "{target}");
        assert!(!public.contains(internal), "{target}");
        assert!(!public.contains("eastDelivery_send"), "{target}");
        if target == "rust" {
            assert!(!public.contains("idealis_check::conform"));
            assert!(!public.contains("pub const MODEL"));
        }
        let full = success(run(&model, &["generate"], &[target]));
        assert!(full.contains(internal), "{target}");
    }

    let schema: Value = serde_json::from_str(&success(run(
        &model,
        &["generate"],
        &["schema", "--public"],
    )))
    .unwrap();
    let full_schema: Value =
        serde_json::from_str(&success(run(&model, &["generate"], &["schema"]))).unwrap();
    for snapshot in ["Config", "State"] {
        assert_eq!(schema["$defs"][snapshot], full_schema["$defs"][snapshot]);
    }
    let commands = &schema["$defs"]["Command"]["oneOf"];
    assert_eq!(commands.as_array().unwrap().len(), 2);
    assert_eq!(commands[0]["properties"]["kind"]["const"], "east_commit");
    assert_eq!(commands[1]["properties"]["kind"]["const"], "west_commit");
    assert!(
        idealis_check::schema::validate(
            &schema,
            "Command",
            &json!({"kind":"east_commit","request":"First"}),
        )
        .is_ok()
    );
    assert!(
        idealis_check::schema::validate(
            &schema,
            "Command",
            &json!({"kind":"eastDelivery_send","request":"First"}),
        )
        .is_err()
    );

    let witness: Value = serde_json::from_str(&success(run(
        &model,
        &["witness"],
        &["IndependentReceipts"],
    )))
    .unwrap();
    assert_eq!(witness["verdict"], "reachable");
    assert_eq!(witness["trace"]["commands"].as_array().unwrap().len(), 5);
    assert!(
        witness["trace"]["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["kind"] == "eastDelivery_send")
    );

    for target in ["ir", "pbt-go", "pbt-typescript"] {
        let unsupported = run(&model, &["generate"], &[target, "--public"]);
        assert_eq!(unsupported.status.code(), Some(2));
        assert!(unsupported.stdout.is_empty());
    }
}

#[test]
fn public_exports_are_validated_and_empty_is_explicit() {
    let directory = std::env::temp_dir().join(format!("rcheck-exports-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("main.model"),
        "model Surface\nbehavior tick() {}\nscope steps = 2",
    )
    .unwrap();
    let model = directory.join("project.rmodel.json");
    for (manifest, message) in [
        (
            json!({"fragments":["main.model"],"exports":["Tick"]}),
            "export names unknown action Tick",
        ),
        (
            json!({"fragments":["main.model"],"exports":["tick","tick"]}),
            "duplicate export tick",
        ),
    ] {
        std::fs::write(&model, manifest.to_string()).unwrap();
        let result = run(&model, &[], &[]);
        assert_eq!(result.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(message),
            "{result:?}"
        );
    }
    std::fs::write(&model, r#"{"fragments":["main.model"]}"#).unwrap();
    assert_eq!(
        run(&model, &["generate"], &["go", "--public"])
            .status
            .code(),
        Some(2)
    );
    std::fs::write(&model, r#"{"fragments":["main.model"],"exports":[]}"#).unwrap();
    let typescript = success(run(&model, &["generate"], &["typescript", "--public"]));
    assert!(typescript.contains("export type Command = never;"));
    let schema: Value = serde_json::from_str(&success(run(
        &model,
        &["generate"],
        &["schema", "--public"],
    )))
    .unwrap();
    assert_eq!(schema["$defs"]["Command"], false);
    assert!(idealis_check::schema::validate(&schema, "Command", &json!({"kind":"tick"})).is_err());
    std::fs::remove_dir_all(directory).unwrap();
}
