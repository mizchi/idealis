//! spec_check [--write]: for every registered specification, evaluates the
//! Pkl source with `pkl eval -f json` and compares it with the committed
//! JSON (D6), checks the links (D1..D4), compares the rendered Markdown (D6)
//! and runs every claim through ralloy's SAT and rtlc (D5). `--write`
//! regenerates the JSON and Markdown instead of comparing them (plan 0092).
use std::path::Path;
use std::process::Command;

/// Pkl source, evaluated JSON, rendered Markdown.
pub const SPECS: &[(&str, &str, &str)] = &[
    ("spec/idealis.pkl", "spec/idealis.json", "docs/glossary.md"),
    (
        "fixtures/rmodel/spec.pkl",
        "fixtures/rmodel/spec.json",
        "fixtures/rmodel/spec.md",
    ),
];

pub const MANIFEST: &str = "spec/documents.json";

/// Crate documents, every specification's rules and the fixed controls.
pub fn drift_manifest(root: &Path) -> serde_json::Value {
    let mut documents: Vec<String> = std::fs::read_dir(root.join("crates"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .flat_map(|name| {
            ["SUPPORT.md", "README.md"]
                .into_iter()
                .map(move |file| format!("crates/{name}/{file}"))
        })
        .filter(|path| root.join(path).is_file())
        .collect();
    documents.sort();
    let specs: Vec<(String, rspec::Spec)> = SPECS
        .iter()
        .filter(|(_, _, markdown)| *markdown != "docs/glossary.md")
        .map(|(_, json, markdown)| {
            let spec =
                rspec::from_json(&std::fs::read_to_string(root.join(json)).unwrap()).unwrap();
            (markdown.to_string(), spec)
        })
        .collect();
    rspec::documents_manifest(&rspec::Drift {
        glossary: "docs/glossary.md",
        documents,
        rules: specs
            .iter()
            .map(|(markdown, spec)| (markdown.as_str(), spec))
            .collect(),
        controls: vec![
            (
                "bounded-wrong",
                "terminology",
                "docs/glossary.md",
                "fixtures/spec-controls/bounded-wrong.md",
                None,
            ),
            (
                "bounded-right",
                "terminology",
                "docs/glossary.md",
                "fixtures/spec-controls/bounded-right.md",
                None,
            ),
            (
                "rule-negated",
                "semantic",
                "fixtures/rmodel/permissions.model",
                "fixtures/spec-controls/rule-negated.md",
                Some("PERM-NO-LEAK"),
            ),
            (
                "rule-paraphrase",
                "semantic",
                "fixtures/rmodel/permissions.model",
                "fixtures/spec-controls/rule-negated.md",
                Some("PERM-NO-LEAK-PARAPHRASE"),
            ),
        ],
    })
}

fn main() {
    let write = std::env::args().any(|argument| argument == "--write");
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let mut failures = 0;
    for (pkl, json, markdown) in SPECS {
        let output = Command::new("pkl")
            .args(["eval", "-f", "json", pkl])
            .current_dir(root)
            .output()
            .expect("pkl on PATH");
        assert!(
            output.status.success(),
            "{pkl}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let evaluated = String::from_utf8(output.stdout).unwrap();
        let spec = rspec::from_json(&evaluated).unwrap();
        let rendered = rspec::render(&spec, pkl);
        if write {
            std::fs::write(root.join(json), &evaluated).unwrap();
            std::fs::write(root.join(markdown), &rendered).unwrap();
        } else {
            for (path, fresh) in [(json, &evaluated), (markdown, &rendered)] {
                if std::fs::read_to_string(root.join(path)).ok().as_ref() != Some(fresh) {
                    eprintln!("D6 {path} is stale; run spec_check --write");
                    failures += 1;
                }
            }
        }
        for problem in rspec::links(&spec, root) {
            eprintln!("{} {pkl}: {}", problem.check, problem.message);
            failures += 1;
        }
        for rule in &spec.rules {
            for claim in &rule.claims {
                let verdicts = engines(root, &claim.model, &claim.check);
                for (engine, verdict) in verdicts {
                    if verdict != claim.expect {
                        eprintln!(
                            "D5 {}: {} {} expects {}, {engine} says {verdict}",
                            rule.id, claim.model, claim.check, claim.expect
                        );
                        failures += 1;
                    }
                }
            }
        }
    }
    // The jev-lint docs manifest, derived from the specifications.
    let manifest = serde_json::to_string_pretty(&drift_manifest(root)).unwrap() + "\n";
    let path = root.join(MANIFEST);
    if write {
        std::fs::write(&path, &manifest).unwrap();
    } else if std::fs::read_to_string(&path).ok().as_deref() != Some(manifest.as_str()) {
        eprintln!("D6 {MANIFEST} is stale; run spec_check --write");
        failures += 1;
    }
    if failures > 0 {
        eprintln!("{failures} problems");
        std::process::exit(1);
    }
    println!("specifications checked");
}

fn engines(root: &Path, path: &str, name: &str) -> Vec<(&'static str, String)> {
    let model = rmodel::parse(&std::fs::read_to_string(root.join(path)).unwrap()).unwrap();
    let check = model
        .checks
        .iter()
        .find(|check| check.name == name)
        .unwrap();
    let alloy = ralloy::Model::parse(&rmodel::to_alloy(&model)).unwrap();
    let label = rmodel::alloy_label(name);
    let index = alloy
        .commands()
        .iter()
        .position(|command| command.label == label)
        .unwrap();
    let sat = match alloy.analyze(index, 50_000_000).unwrap().status {
        ralloy::Status::Counterexample => "violated",
        ralloy::Status::NoCounterexample => "holds",
        ralloy::Status::Instance => "reachable",
        ralloy::Status::NoInstance => "unreachable",
        ralloy::Status::Unknown => "unknown",
    };
    let (source, config) = rmodel::to_tla(&model, name).unwrap();
    let result = idealis_core::compile_tla_with_options(
        &source,
        &config,
        idealis_core::CompileOptions {
            irVersion: idealis_core::IrVersion::V2,
        },
    )
    .unwrap()
    .check(idealis_core::SafetyBounds {
        max_depth: 1_000,
        max_states: 2_000_000,
        max_work: 500_000_000,
        max_temporal_nodes: None,
    })
    .unwrap();
    let found = result.violation.is_some();
    let explicit = match (result.status.as_str(), check.kind, found) {
        ("complete", rmodel::CheckKind::Invariant, _) => "holds",
        ("complete", rmodel::CheckKind::Reachable, _) => "unreachable",
        (_, rmodel::CheckKind::Invariant, true) => "violated",
        (_, rmodel::CheckKind::Reachable, true) => "reachable",
        (other, _, _) => other,
    };
    vec![("SAT", sat.to_owned()), ("rtlc", explicit.to_owned())]
}
