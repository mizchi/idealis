//! spec_check [--write]: for every registered specification, evaluates the
//! Pkl source with `pkl eval -f json` and compares it with the committed
//! JSON (D6), checks the links (D1..D4), compares the rendered Markdown (D6)
//! and runs claims with the declared evaluator (D5). `--write`
//! regenerates the JSON and Markdown instead of comparing them (plan 0092).
use std::path::Path;
use std::process::Command;

#[derive(Clone, Copy)]
enum Checker {
    Backends,
    Reference,
}

const DESIGN_JSON: &str = "spec/rmodel-design.json";
const DESIGN_MARKDOWN: &str = "spec/rmodel-design.md";
const DESIGN_MANIFEST: &str = "spec/rmodel-design-documents.json";

/// Pkl source, evaluated JSON, rendered Markdown, checker.
const SPECS: &[(&str, &str, &str, Checker)] = &[
    (
        "spec/idealis.pkl",
        "spec/idealis.json",
        "docs/glossary.md",
        Checker::Backends,
    ),
    (
        "fixtures/rmodel/spec.pkl",
        "fixtures/rmodel/spec.json",
        "fixtures/rmodel/spec.md",
        Checker::Backends,
    ),
    (
        "spec/rmodel-design.pkl",
        DESIGN_JSON,
        DESIGN_MARKDOWN,
        Checker::Reference,
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
    let specs: Vec<(String, idealis_spec::Spec)> = SPECS
        .iter()
        .map(|(_, json, markdown, _)| {
            let spec = idealis_spec::from_json(&std::fs::read_to_string(root.join(json)).unwrap())
                .unwrap();
            (markdown.to_string(), spec)
        })
        .collect();
    idealis_spec::documents_manifest(&idealis_spec::Drift {
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
    .unwrap_or_else(|clash| panic!("{clash}"))
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let write = args.iter().any(|argument| argument == "--write");
    let design_only = args.iter().any(|argument| argument == "--rmodel-design");
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let mut failures = 0;
    for (pkl, json, markdown, checker) in SPECS {
        if design_only && *json != DESIGN_JSON {
            continue;
        }
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
        let spec = idealis_spec::from_json(&evaluated).unwrap();
        let rendered = idealis_spec::render(&spec, pkl);
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
        let problems = idealis_spec::links(&spec, root);
        if !problems.is_empty() {
            for problem in &problems {
                eprintln!("{} {pkl}: {}", problem.check, problem.message);
            }
            failures += problems.len();
            continue;
        }
        for rule in &spec.rules {
            for claim in rule.all_claims() {
                let verdicts = match checker {
                    Checker::Backends => engines(root, &claim.model, &claim.check),
                    Checker::Reference => vec![(
                        "rmodel",
                        reference(root, &claim.model, &claim.check).to_owned(),
                    )],
                };
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
    // Both manifests use the same checked rule text and model links.
    let mut manifests = vec![(DESIGN_MANIFEST, design_drift_manifest(root))];
    if !design_only || write {
        manifests.push((MANIFEST, drift_manifest(root)));
    }
    for (path, manifest) in manifests {
        let manifest = serde_json::to_string_pretty(&manifest).unwrap() + "\n";
        if write {
            std::fs::write(root.join(path), &manifest).unwrap();
        } else if std::fs::read_to_string(root.join(path)).ok().as_deref()
            != Some(manifest.as_str())
        {
            eprintln!("D6 {path} is stale; run spec_check --write");
            failures += 1;
        }
    }
    if failures > 0 {
        eprintln!("{failures} problems");
        std::process::exit(1);
    }
    println!("specifications checked");
}

fn design_drift_manifest(root: &Path) -> serde_json::Value {
    let spec =
        idealis_spec::from_json(&std::fs::read_to_string(root.join(DESIGN_JSON)).unwrap()).unwrap();
    idealis_spec::documents_manifest(&idealis_spec::Drift {
        glossary: DESIGN_MARKDOWN,
        documents: vec![],
        rules: vec![(DESIGN_MARKDOWN, &spec)],
        controls: vec![],
    })
    .unwrap()
}

fn reference(root: &Path, path: &str, name: &str) -> &'static str {
    let model = rmodel::parse(&std::fs::read_to_string(root.join(path)).unwrap()).unwrap();
    reference_verdict(&model, path, name)
}

fn reference_verdict(model: &rmodel::Model, path: &str, name: &str) -> &'static str {
    use rmodel::reference::Outcome;
    let verdict = rmodel::reference::evaluate(model, 2_000_000)
        .into_iter()
        .find(|verdict| verdict.check == name)
        .unwrap_or_else(|| panic!("{path}: no check {name}"));
    match verdict.outcome {
        Outcome::Holds => "holds",
        Outcome::Violated { .. } => "violated",
        Outcome::Reachable { .. } => "reachable",
        Outcome::Unreachable => "unreachable",
        Outcome::Empty => "empty",
        Outcome::Unknown => "unknown",
    }
}

fn engines(root: &Path, path: &str, name: &str) -> Vec<(&'static str, String)> {
    let model = rmodel::parse(&std::fs::read_to_string(root.join(path)).unwrap()).unwrap();
    if model.transitions.iter().any(|check| check.name == name) {
        return vec![("rmodel", reference_verdict(&model, path, name).to_owned())];
    }
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
