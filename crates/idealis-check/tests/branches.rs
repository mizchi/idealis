use serde_json::Value;
use std::path::Path;
use std::process::Command;

#[test]
fn an_inline_record_field_branch_respects_the_enclosing_update_and_round_trips() {
    let directory =
        std::env::temp_dir().join(format!("rcheck-record-branch-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("cache.model");
    let base = include_str!("../../../examples/model-patterns/cache-refresh/conditional-304.ideal");
    let source = base.replace(
        "tag: cached.tag, body: cached.body, policy: Refreshed",
        "policy: if { cached.tag = requested } then Refreshed else Replacement, body: cached.body, tag: cached.tag",
    );
    assert_ne!(source, base);
    std::fs::write(&path, &source).unwrap();
    let report = branches(&path, "finish", &[]);
    let rows = report["branches"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["else"]["verdict"], "reachable");
    let inline = &rows[1];
    let (line, text) = source
        .lines()
        .enumerate()
        .find(|(_, text)| text.contains("policy: if"))
        .unwrap();
    assert!(inline["source"].as_str().unwrap().ends_with(&format!(
        "cache.model:{}:{}",
        line + 1,
        text.find("if").unwrap() + 1
    )));
    assert_eq!(inline["kind"], "value");
    assert!(inline.get("binding").is_none());
    assert_eq!(
        inline["then"]["trace"]["finalState"]["cached"]["policy"],
        "Refreshed"
    );
    assert_eq!(inline["else"]["verdict"], "unreachable");

    let ir = directory.join("model.json");
    let map = directory.join("branch-map.json");
    for (target, file) in [("ir", &ir), ("branch-map", &map)] {
        let output = Command::new(env!("CARGO_BIN_EXE_idealis-check"))
            .args(["generate", path.to_str().unwrap(), target])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        std::fs::write(file, output.stdout).unwrap();
    }
    let wire: Value = serde_json::from_slice(&std::fs::read(&map).unwrap()).unwrap();
    assert_eq!(wire["version"], 4);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        branches(&ir, "finish", &["--branch-map", map.to_str().unwrap()]),
        report
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn range_response_branch_preserves_let_location_through_ir() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/model-patterns/cache-refresh/range-assembly.ideal");
    let report = branches(&path, "request", &[]);
    let rows = report["branches"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    let source = std::fs::read_to_string(&path).unwrap();
    let (line, text) = source
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains("let status"))
        .unwrap();
    let column = text.find("if").unwrap() + 1;
    assert!(
        rows[0]["source"]
            .as_str()
            .unwrap()
            .ends_with(&format!("range-assembly.ideal:{}:{column}", line + 1))
    );
    assert_eq!(rows[0]["kind"], "value");
    assert_eq!(rows[0]["binding"], "status");
    assert_eq!(rows[0]["then"]["verdict"], "reachable");
    assert_eq!(rows[0]["else"]["verdict"], "reachable");
    assert_eq!(
        rows[0]["then"]["trace"]["commands"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["mode"],
        "Resume"
    );
    let limited = branches(&path, "request", &["--max-states", "1"]);
    assert_eq!(limited["complete"], false);
    assert_eq!(limited["branches"][0]["then"]["verdict"], "unknown");
    assert_eq!(limited["branches"][0]["else"]["verdict"], "unknown");

    let directory = std::env::temp_dir().join(format!("rcheck-let-branch-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let ir = directory.join("model.json");
    let map = directory.join("branches.json");
    for (target, file) in [("ir", &ir), ("branch-map", &map)] {
        let generated = Command::new(env!("CARGO_BIN_EXE_idealis-check"))
            .args(["generate", path.to_str().unwrap(), target])
            .output()
            .unwrap();
        assert!(generated.status.success(), "{generated:?}");
        std::fs::write(file, generated.stdout).unwrap();
    }
    assert_eq!(
        branches(&ir, "request", &["--branch-map", map.to_str().unwrap()]),
        report
    );
    let map_document: Value = serde_json::from_slice(&std::fs::read(&map).unwrap()).unwrap();
    assert_eq!(map_document["version"], 3);
    let audit = |model: &Path, args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_idealis-check"))
            .args(["coverage", model.to_str().unwrap()])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        output.stdout
    };
    assert_eq!(
        audit(&ir, &["--branch-map", map.to_str().unwrap()]),
        audit(&path, &[]),
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn the_same_range_conditional_can_be_inlined_or_bound_within_a_record() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let original = root.join("../../examples/model-patterns/cache-refresh/range-assembly.ideal");
    let source = std::fs::read_to_string(&original).unwrap();
    let condition = "if { mode = Resume and cachedTag != origin } then Full else Partial";
    let without_binding = source.replace(&format!("    let status = {condition}\n"), "");
    assert_ne!(source, without_binding);
    let inlined = without_binding.replace("status: status", &format!("status: {condition}"));
    let record_binding = format!("let reply = Reply {{ tag: origin, status: {condition} }}");
    let within_binding = without_binding.replace(
        "    pending = pending ++ part -> Reply { tag: origin, status: status }",
        &format!("    {record_binding}\n    pending = pending ++ part -> reply"),
    );
    let path =
        std::env::temp_dir().join(format!("rcheck-range-inline-{}.model", std::process::id()));
    let generate_ir = |file: &Path| {
        let output = Command::new(env!("CARGO_BIN_EXE_idealis-check"))
            .args(["generate", file.to_str().unwrap(), "ir"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        output.stdout
    };
    let original_ir = generate_ir(&original);
    let original_report = branches(&original, "request", &[]);
    for (variant, binding) in [
        (inlined, Value::Null),
        (within_binding, Value::from("reply")),
    ] {
        std::fs::write(&path, &variant).unwrap();
        assert_eq!(generate_ir(&path), original_ir);
        let report = branches(&path, "request", &[]);
        let rows = report["branches"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["binding"], binding);
        for arm in ["then", "else"] {
            assert_eq!(rows[0][arm], original_report["branches"][0][arm]);
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_let_branch_is_reached_only_at_selected_uses_and_reported_once() {
    let path =
        std::env::temp_dir().join(format!("rcheck-let-selected-{}.model", std::process::id()));
    std::fs::write(
        &path,
        "model LetBranches
var first: one Bool
var second: one Bool
var guarded: one Bool
init { not first second not guarded }
behavior choose(enabled: Bool, left: Bool) {
  update {
    let ignored = if { true } then true else false
    let value = if { enabled } then true else false
    let alias = value
    let onlyWhenEnabled = if { enabled } then true else false
    if { enabled and left } then { first = alias guarded = onlyWhenEnabled }
    if { not enabled and not left } then { second = value }
  }
}
scope steps = 2",
    )
    .unwrap();
    let report = branches(&path, "choose", &[]);
    let rows = report["branches"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    assert!(rows[0]["source"].as_str().unwrap().ends_with(":9:17"));
    assert_eq!(rows[0]["kind"], "value");
    assert_eq!(rows[0]["binding"], "value");
    assert_eq!(rows[0]["then"]["trace"]["finalState"]["first"], true);
    assert_eq!(rows[0]["else"]["trace"]["finalState"]["second"], false);
    assert_eq!(rows[1]["kind"], "value");
    assert_eq!(rows[1]["binding"], "onlyWhenEnabled");
    assert_eq!(rows[1]["then"]["trace"]["finalState"]["guarded"], true);
    assert_eq!(rows[1]["else"]["verdict"], "unreachable");
    std::fs::remove_file(path).unwrap();
}

fn branches(path: &Path, action: &str, flags: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .args(["branches", path.to_str().unwrap(), action])
        .args(flags)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn partition_publish_reports_both_arms_once_with_replayable_paths() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/model-patterns/subscriptions/partition-snapshot.ideal");
    let report = branches(&path, "publish", &["--max-states", "20000"]);
    assert_eq!(report["complete"], true);
    let rows = report["branches"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    let source = std::fs::read_to_string(&path).unwrap();
    let publish_line = source
        .lines()
        .position(|line| line.starts_with("    if { Payments.captured"))
        .unwrap()
        + 1;
    assert!(
        rows[0]["source"]
            .as_str()
            .unwrap()
            .ends_with(&format!("partition-snapshot.ideal:{publish_line}:5"))
    );
    for (arm, decision) in [("then", "Accepted"), ("else", "Rejected")] {
        let branch = &rows[0][arm];
        assert_eq!(branch["verdict"], "reachable");
        assert_eq!(
            branch["trace"]["commands"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["kind"],
            "publish"
        );
        assert_eq!(branch["trace"]["finalState"]["decision"], decision);
    }

    let budget = branches(&path, "publish", &["--max-states", "1"]);
    assert_eq!(budget["complete"], false);
    assert_eq!(budget["branches"][0]["then"]["verdict"], "unknown");
    assert_eq!(budget["branches"][0]["else"]["verdict"], "unknown");
    assert_eq!(budget["diagnostics"][0]["resource"], "states");
}

#[test]
fn nested_updates_respect_ancestors_and_include_implicit_else() {
    let path = std::env::temp_dir().join(format!("rcheck-branches-{}.model", std::process::id()));
    std::fs::write(
        &path,
        r#"model Nested
var seen: one Bool
init { not seen }
behavior choose(enabled: Bool) {
  update {
    let run = enabled
    if { run } then {
      if { not run } then { seen = false }
    }
  }
}
scope steps = 2
"#,
    )
    .unwrap();
    let report = branches(&path, "choose", &[]);
    assert_eq!(report["complete"], true);
    let rows = report["branches"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows[0]["source"].as_str().unwrap().ends_with(":7:5"));
    assert!(rows[1]["source"].as_str().unwrap().ends_with(":8:7"));
    assert_eq!(rows[0]["then"]["trace"]["commands"][0]["enabled"], true);
    assert_eq!(rows[0]["else"]["trace"]["commands"][0]["enabled"], false);
    assert_eq!(rows[1]["then"]["verdict"], "unreachable");
    assert!(rows[1]["then"]["trace"].is_null());
    assert_eq!(rows[1]["else"]["trace"]["commands"][0]["enabled"], true);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn linked_branches_point_to_original_update_groups_and_assigned_conditionals() {
    let directory = std::env::temp_dir().join(format!("rcheck-branch-link-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("contract.model"),
        "model Linked\nscope steps = 3",
    )
    .unwrap();
    let source = "behavior @choose() { update { if { true } then { @flag = if { true } then true else false } } }";
    std::fs::write(
        directory.join("part.model"),
        format!("// 日本語\nvar @flag: one Bool\ninit {{ not @flag }}\n{source}"),
    )
    .unwrap();
    let manifest = directory.join("project.rmodel.json");
    std::fs::write(
        &manifest,
        r#"{"fragments":["contract.model",{"path":"part.model","namespace":"subscriber"}]}"#,
    )
    .unwrap();
    let report = branches(&manifest, "subscriber_choose", &[]);
    let rows = report["branches"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows[0]["source"]
            .as_str()
            .unwrap()
            .ends_with(&format!("part.model:4:{}", source.find("if").unwrap() + 1))
    );
    assert_eq!(rows[0]["then"]["verdict"], "reachable");
    assert_eq!(rows[0]["else"]["verdict"], "unreachable");
    assert!(
        rows[1]["source"]
            .as_str()
            .unwrap()
            .ends_with(&format!("part.model:4:{}", source.rfind("if").unwrap() + 1))
    );
    assert_eq!(rows[1]["kind"], "value");
    assert_eq!(rows[1]["then"]["verdict"], "reachable");
    assert_eq!(rows[1]["else"]["verdict"], "unreachable");
    let output = Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .args(["coverage", manifest.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let coverage: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(coverage["actions"][0]["name"], "subscriber_choose");
    assert_eq!(coverage["actions"][0]["branches"], report["branches"]);
    let ir = directory.join("model.json");
    let map = directory.join("branches.json");
    for (target, file) in [("ir", &ir), ("branch-map", &map)] {
        let generated = Command::new(env!("CARGO_BIN_EXE_idealis-check"))
            .args(["generate", manifest.to_str().unwrap(), target])
            .output()
            .unwrap();
        assert!(generated.status.success(), "{generated:?}");
        std::fs::write(file, generated.stdout).unwrap();
    }
    std::fs::remove_file(directory.join("part.model")).unwrap();
    assert_eq!(
        branches(
            &ir,
            "subscriber_choose",
            &["--branch-map", map.to_str().unwrap()]
        ),
        report
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn the_branches_inside_an_assigned_conditional_follow_the_selected_arms() {
    let path = std::env::temp_dir().join(format!(
        "rcheck-value-branches-{}.model",
        std::process::id()
    ));
    std::fs::write(
        &path,
        "model Assigned
enum Item = A | B | C
var picked: one Item
var other: set Item
init { picked = C no other }
behavior choose(flag: Bool) {
  update {
    picked = (if { flag } then A else if { not flag } then B else C)
    other = (if { flag } then A else B) + C
  }
}
scope steps = 2",
    )
    .unwrap();
    let report = branches(&path, "choose", &[]);
    let rows = report["branches"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|row| row["kind"] == "value"));
    assert_eq!(rows[0]["then"]["trace"]["commands"][0]["flag"], true);
    assert_eq!(rows[1]["then"]["trace"]["commands"][0]["flag"], false);
    assert_eq!(rows[1]["then"]["trace"]["finalState"]["picked"], "B");
    assert_eq!(rows[1]["else"]["verdict"], "unreachable");
    assert_eq!(rows[2]["then"]["trace"]["commands"][0]["flag"], true);
    assert_eq!(rows[2]["else"]["trace"]["commands"][0]["flag"], false);
    std::fs::remove_file(path).unwrap();
}
