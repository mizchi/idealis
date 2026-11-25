//! The committed specifications link cleanly and their Markdown is fresh;
//! each kind of broken link is reported (plan 0092, D1..D4 and D6).
use serde_json::{Value, json};
use std::path::Path;

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}
const SPECS: &[(&str, &str, &str)] = &[
    ("spec/idealis.pkl", "spec/idealis.json", "docs/glossary.md"),
    (
        "fixtures/rmodel/spec.pkl",
        "fixtures/rmodel/spec.json",
        "fixtures/rmodel/spec.md",
    ),
    (
        "spec/rmodel-design.pkl",
        "spec/rmodel-design.json",
        "spec/rmodel-design.md",
    ),
];

fn load(path: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(root().join(path)).unwrap()).unwrap()
}
fn problems(value: &Value) -> Vec<(String, String)> {
    let spec = rspec::from_json(&value.to_string()).unwrap();
    rspec::links(&spec, root())
        .into_iter()
        .map(|problem| (problem.check.to_owned(), problem.message))
        .collect()
}

#[test]
fn committed_specifications_link_and_render() {
    for (pkl, json, markdown) in SPECS {
        let value = load(json);
        assert_eq!(problems(&value), vec![], "{json}");
        let spec = rspec::from_json(&value.to_string()).unwrap();
        assert_eq!(
            rspec::render(&spec, pkl),
            std::fs::read_to_string(root().join(markdown)).unwrap(),
            "{markdown} is stale; run just spec-write"
        );
    }
}

/// A change that breaks one link of a specification.
type Fault = Box<dyn Fn(&mut Value)>;

/// One fault per check, each on a copy of the corpus specification.
#[test]
fn broken_links_are_reported() {
    let base = load("fixtures/rmodel/spec.json");
    let cases: Vec<(&str, &str, Fault)> = vec![
        (
            "D1",
            "unknown term ghost",
            Box::new(|spec| spec["spec"]["rules"][0]["terms"][0] = json!("ghost")),
        ),
        (
            "D1",
            "duplicate term",
            Box::new(|spec| spec["spec"]["terms"][1]["id"] = json!("proc")),
        ),
        (
            "D1",
            "unknown group",
            Box::new(|spec| spec["spec"]["terms"][0]["group"] = json!("無い")),
        ),
        (
            "D2",
            "declares no Ghost",
            Box::new(|spec| spec["spec"]["terms"][0]["model"][0]["name"] = json!("Ghost")),
        ),
        (
            "D2",
            "has no identifier Ghost",
            Box::new(|spec| {
                spec["spec"]["terms"][0]["code"] =
                    json!([{"path": "crates/ralloy/src/lib.rs", "symbol": "Ghost"}])
            }),
        ),
        (
            "D2",
            "No such file",
            Box::new(|spec| {
                spec["spec"]["terms"][0]["model"][0]["path"] = json!("fixtures/rmodel/ghost.model")
            }),
        ),
        (
            "D2",
            "more than one term",
            Box::new(|spec| {
                let reference = spec["spec"]["terms"][0]["model"][0].clone();
                spec["spec"]["terms"][1]["model"]
                    .as_array_mut()
                    .unwrap()
                    .push(reference);
            }),
        ),
        // `Proc` loses its only binding in mutex.model.
        (
            "D3",
            "Proc is bound to no term",
            Box::new(|spec| {
                spec["spec"]["terms"][0]["model"]
                    .as_array_mut()
                    .unwrap()
                    .remove(0);
            }),
        ),
        (
            "D3",
            "claimed by no rule",
            Box::new(|spec| {
                spec["spec"]["rules"][0]["claims"]
                    .as_array_mut()
                    .unwrap()
                    .remove(0);
            }),
        ),
        (
            "D4",
            "expects reachable of a Invariant check",
            Box::new(|spec| spec["spec"]["rules"][0]["claims"][0]["expect"] = json!("reachable")),
        ),
        (
            "D4",
            "has no check Ghost",
            Box::new(|spec| spec["spec"]["rules"][0]["claims"][0]["check"] = json!("Ghost")),
        ),
        (
            "D4",
            "claimed twice",
            Box::new(|spec| {
                let claim = spec["spec"]["rules"][0]["claims"][0].clone();
                spec["spec"]["rules"][1]["claims"]
                    .as_array_mut()
                    .unwrap()
                    .push(claim);
            }),
        ),
        (
            "D4",
            "no claim and no reason",
            Box::new(|spec| {
                spec["spec"]["rules"][3]["claims"] = json!([]);
            }),
        ),
    ];
    for (check, needle, fault) in cases {
        let mut spec = base.clone();
        fault(&mut spec);
        let found = problems(&spec);
        assert!(
            found
                .iter()
                .any(|(kind, message)| kind == check && message.contains(needle)),
            "{check} {needle}: {found:?}"
        );
    }
    // A reason grounds a rule without claims; its check is then unclaimed.
    let mut reasoned = base.clone();
    reasoned["spec"]["rules"][3]["claims"] = json!([]);
    reasoned["spec"]["rules"][3]["unformalized"] = json!("prose only");
    let found = problems(&reasoned);
    assert!(found.iter().all(|(kind, _)| kind != "D4"), "{found:?}");
    assert!(
        found
            .iter()
            .any(|(kind, message)| kind == "D3" && message.contains("OneWaiting"))
    );
}

/// A grounding names an implementation and a test that exist (plan 0093).
#[test]
fn broken_groundings_are_reported() {
    let base = load("spec/idealis.json");
    for (field, needle) in [
        ("implementation", "has no identifier ghost_fn"),
        ("test", "has no identifier ghost_fn"),
    ] {
        let mut spec = base.clone();
        spec["spec"]["groundings"][0][field]["symbol"] = json!("ghost_fn");
        let found = problems(&spec);
        assert!(
            found
                .iter()
                .any(|(kind, message)| kind == "D2" && message.contains(needle)),
            "{field}: {found:?}"
        );
    }
    let mut spec = base.clone();
    spec["spec"]["terms"]
        .as_array_mut()
        .unwrap()
        .retain(|term| term["id"] != "compare.slot");
    assert!(
        problems(&spec)
            .iter()
            .any(|(kind, message)| kind == "D3" && message.contains("Slot is bound to no term"))
    );
}

/// D6: the Markdown is a function of the evaluated JSON.
#[test]
fn stale_markdown_is_detected() {
    let mut value = load("fixtures/rmodel/spec.json");
    value["spec"]["rules"][0]["text"]["ja"] = json!("書き換えた文");
    let spec = rspec::from_json(&value.to_string()).unwrap();
    assert_ne!(
        rspec::render(&spec, "fixtures/rmodel/spec.pkl"),
        std::fs::read_to_string(root().join("fixtures/rmodel/spec.md")).unwrap()
    );
}

/// Review of #103: terms may name checks (plan 0092); checks still need
/// claims.
#[test]
fn terms_may_reference_checks() {
    let mut spec = load("spec/idealis.json");
    spec["spec"]["terms"][0]["model"] =
        json!([{"path": "spec/models/roracle-report.model", "name": "MatchedOnlyIfAllMatched"}]);
    assert_eq!(problems(&spec), vec![]);
}

/// Review of #103: two paths with the same source id are an error, and a
/// control's reference file is registered.
#[test]
fn manifest_sources_are_distinct_and_registered() {
    let spec = rspec::from_json(
        &std::fs::read_to_string(root().join("fixtures/rmodel/spec.json")).unwrap(),
    )
    .unwrap();
    let clash = rspec::documents_manifest(&rspec::Drift {
        glossary: "docs/glossary.md",
        documents: vec!["crates/a-b/README.md".into(), "crates/a_b/README.md".into()],
        rules: vec![],
        controls: vec![],
    });
    assert!(clash.is_err());
    let manifest = rspec::documents_manifest(&rspec::Drift {
        glossary: "docs/glossary.md",
        documents: vec![],
        rules: vec![("fixtures/rmodel/spec.md", &spec)],
        controls: vec![(
            "own",
            "semantic",
            "fixtures/spec-controls/bounded-right.md",
            "fixtures/spec-controls/rule-negated.md",
            Some("PERM-NO-LEAK"),
        )],
    })
    .unwrap();
    let roots = manifest["roots"].as_array().unwrap();
    assert!(
        roots
            .iter()
            .any(|root| root == "fixtures/spec-controls/bounded-right.md")
    );
}
