//! A specification in Pkl (`pkl eval -f json`): a ubiquitous language of
//! terms bound to model declarations and code symbols, and rules grounded by
//! claims about model checks (plan 0092).
//!
//! [`links`] decides everything that is static: unique ids, resolving
//! references, a closed vocabulary over the referenced models, claimed
//! checks and expectations that fit their checks. Semantic drift between
//! the rendered glossary and prose is left to `jev-lint docs`.
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Clone, Debug, Deserialize)]
pub struct Text {
    pub ja: String,
    pub en: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ModelRef {
    pub path: String,
    pub name: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct CodeRef {
    pub path: String,
    pub symbol: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Term {
    pub id: String,
    pub kind: String,
    pub group: String,
    pub name: Text,
    pub definition: Text,
    pub aliases: Vec<String>,
    pub avoid: Vec<String>,
    pub model: Vec<ModelRef>,
    pub code: Vec<CodeRef>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Claim {
    pub model: String,
    pub check: String,
    pub expect: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Rule {
    pub id: String,
    pub text: Text,
    pub terms: Vec<String>,
    pub claims: Vec<Claim>,
    pub unformalized: Option<String>,
}
/// An implementation checked against a model by a named test (plan 0093).
#[derive(Clone, Debug, Deserialize)]
pub struct Grounding {
    pub model: String,
    pub implementation: CodeRef,
    pub test: CodeRef,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Spec {
    pub id: String,
    pub name: Text,
    pub groups: Vec<String>,
    pub terms: Vec<Term>,
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub groundings: Vec<Grounding>,
}

/// Reads the JSON of `pkl eval -f json`, whose module has a `spec` property.
pub fn from_json(json: &str) -> Result<Spec, String> {
    #[derive(Deserialize)]
    struct Module {
        spec: Spec,
    }
    serde_json::from_str::<Module>(json)
        .map(|module| module.spec)
        .map_err(|error| format!("spec JSON: {error}"))
}

/// A broken link, named by the check of plan 0092 it fails.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Problem {
    /// D1..D4.
    pub check: &'static str,
    pub message: String,
}
fn problem(check: &'static str, message: impl Into<String>) -> Problem {
    Problem {
        check,
        message: message.into(),
    }
}

/// Whether `symbol` occurs in `source` as a whole identifier.
fn occurs(source: &str, symbol: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    source.match_indices(symbol).any(|(start, _)| {
        let before = source[..start].chars().next_back();
        let after = source[start + symbol.len()..].chars().next();
        !before.is_some_and(word) && !after.is_some_and(word)
    })
}

/// Static checks D1..D4 against files under `root`. Model files are parsed
/// with rmodel; code files are read as text.
pub fn links(spec: &Spec, root: &Path) -> Vec<Problem> {
    let mut problems = Vec::new();
    // D1: unique ids, known groups, known terms in rules.
    let mut ids = BTreeSet::new();
    for term in &spec.terms {
        if !ids.insert(term.id.as_str()) {
            problems.push(problem("D1", format!("duplicate term {}", term.id)));
        }
        if !spec.groups.contains(&term.group) {
            problems.push(problem(
                "D1",
                format!("{}: unknown group {}", term.id, term.group),
            ));
        }
    }
    let mut rule_ids = BTreeSet::new();
    for rule in &spec.rules {
        if !rule_ids.insert(rule.id.as_str()) {
            problems.push(problem("D1", format!("duplicate rule {}", rule.id)));
        }
        for term in &rule.terms {
            if !ids.contains(term.as_str()) {
                problems.push(problem("D1", format!("{}: unknown term {term}", rule.id)));
            }
        }
    }
    // D2: references resolve.
    let mut models: BTreeMap<&str, Option<rmodel::Model>> = BTreeMap::new();
    let paths = spec
        .terms
        .iter()
        .flat_map(|term| term.model.iter().map(|reference| reference.path.as_str()))
        .chain(
            spec.rules
                .iter()
                .flat_map(|rule| rule.claims.iter().map(|claim| claim.model.as_str())),
        )
        .chain(
            spec.groundings
                .iter()
                .map(|grounding| grounding.model.as_str()),
        );
    for path in paths {
        models
            .entry(path)
            .or_insert_with(|| match std::fs::read_to_string(root.join(path)) {
                Ok(source) => match rmodel::parse(&source) {
                    Ok(model) => Some(model),
                    Err(error) => {
                        problems.push(problem("D2", format!("{path}: {error}")));
                        None
                    }
                },
                Err(error) => {
                    problems.push(problem("D2", format!("{path}: {error}")));
                    None
                }
            });
    }
    let declarations = |model: &rmodel::Model| -> Vec<String> {
        model
            .sigs
            .iter()
            .cloned()
            .chain(
                model
                    .enums
                    .iter()
                    .flat_map(|(name, members)| std::iter::once(name).chain(members).cloned()),
            )
            .chain(model.relations.iter().map(|relation| relation.name.clone()))
            .chain(model.actions.iter().map(|action| action.name.clone()))
            .collect()
    };
    let mut bound: BTreeSet<(String, String)> = BTreeSet::new();
    for term in &spec.terms {
        for reference in &term.model {
            let Some(Some(model)) = models.get(reference.path.as_str()) else {
                continue;
            };
            if !declarations(model).contains(&reference.name) {
                problems.push(problem(
                    "D2",
                    format!(
                        "{}: {} declares no {}",
                        term.id, reference.path, reference.name
                    ),
                ));
            }
            if !bound.insert((reference.path.clone(), reference.name.clone())) {
                problems.push(problem(
                    "D2",
                    format!(
                        "{} {} is bound to more than one term",
                        reference.path, reference.name
                    ),
                ));
            }
        }
        for reference in &term.code {
            match std::fs::read_to_string(root.join(&reference.path)) {
                Ok(source) if occurs(&source, &reference.symbol) => {}
                Ok(_) => problems.push(problem(
                    "D2",
                    format!(
                        "{}: {} has no identifier {}",
                        term.id, reference.path, reference.symbol
                    ),
                )),
                Err(error) => problems.push(problem(
                    "D2",
                    format!("{}: {}: {error}", term.id, reference.path),
                )),
            }
        }
    }
    for grounding in &spec.groundings {
        for reference in [&grounding.implementation, &grounding.test] {
            match std::fs::read_to_string(root.join(&reference.path)) {
                Ok(source) if occurs(&source, &reference.symbol) => {}
                Ok(_) => problems.push(problem(
                    "D2",
                    format!(
                        "grounding of {}: {} has no identifier {}",
                        grounding.model, reference.path, reference.symbol
                    ),
                )),
                Err(error) => problems.push(problem(
                    "D2",
                    format!(
                        "grounding of {}: {}: {error}",
                        grounding.model, reference.path
                    ),
                )),
            }
        }
    }
    // D3: a closed vocabulary over the referenced models, and every check
    // claimed.
    let claimed: BTreeSet<(String, String)> = spec
        .rules
        .iter()
        .flat_map(|rule| {
            rule.claims
                .iter()
                .map(|claim| (claim.model.clone(), claim.check.clone()))
        })
        .collect();
    for (path, model) in &models {
        let Some(model) = model else { continue };
        for name in declarations(model) {
            if !bound.contains(&(path.to_string(), name.clone())) {
                problems.push(problem("D3", format!("{path}: {name} is bound to no term")));
            }
        }
        for check in &model.checks {
            if !claimed.contains(&(path.to_string(), check.name.clone())) {
                problems.push(problem(
                    "D3",
                    format!("{path}: check {} is claimed by no rule", check.name),
                ));
            }
        }
    }
    // D4: claims fit their checks; rules are grounded or say why not.
    let mut seen = BTreeSet::new();
    for rule in &spec.rules {
        if rule.claims.is_empty() && rule.unformalized.as_deref().is_none_or(str::is_empty) {
            problems.push(problem(
                "D4",
                format!("{} has no claim and no reason", rule.id),
            ));
        }
        for claim in &rule.claims {
            if !seen.insert((claim.model.as_str(), claim.check.as_str())) {
                problems.push(problem(
                    "D4",
                    format!("{} {} is claimed twice", claim.model, claim.check),
                ));
            }
            let Some(Some(model)) = models.get(claim.model.as_str()) else {
                continue;
            };
            let Some(check) = model.checks.iter().find(|check| check.name == claim.check) else {
                problems.push(problem(
                    "D4",
                    format!("{}: {} has no check {}", rule.id, claim.model, claim.check),
                ));
                continue;
            };
            let fits = match check.kind {
                rmodel::CheckKind::Invariant => {
                    matches!(claim.expect.as_str(), "holds" | "violated")
                }
                rmodel::CheckKind::Reachable => {
                    matches!(claim.expect.as_str(), "reachable" | "unreachable")
                }
            };
            if !fits {
                problems.push(problem(
                    "D4",
                    format!(
                        "{}: {} expects {} of a {:?} check",
                        rule.id, claim.check, claim.expect, check.kind
                    ),
                ));
            }
        }
    }
    problems.sort();
    problems
}

/// The Japanese glossary and rules as Markdown: what jev-lint reads.
pub fn render(spec: &Spec, source: &str) -> String {
    let mut out = format!(
        "# {}\n\n<!-- Generated from {source} by rspec (plan 0092). Do not edit. -->\n",
        spec.name.ja
    );
    for group in &spec.groups {
        out.push_str(&format!("\n## {group}\n"));
        for term in spec.terms.iter().filter(|term| &term.group == group) {
            out.push_str(&format!(
                "\n### {}（{}）\n\n{}\n\n",
                term.name.ja, term.name.en, term.definition.ja
            ));
            out.push_str(&format!("- id: `{}`\n", term.id));
            if !term.aliases.is_empty() {
                out.push_str(&format!("- 同じ意味の語: {}\n", term.aliases.join("、")));
            }
            if !term.avoid.is_empty() {
                out.push_str(&format!(
                    "- この意味に使わない語: {}\n",
                    term.avoid.join("、")
                ));
            }
            for reference in &term.model {
                out.push_str(&format!(
                    "- モデル: `{}` の `{}`\n",
                    reference.path, reference.name
                ));
            }
            for reference in &term.code {
                out.push_str(&format!(
                    "- コード: `{}` の `{}`\n",
                    reference.path, reference.symbol
                ));
            }
        }
    }
    if !spec.groundings.is_empty() {
        out.push_str("\n## 実装の照合\n\n");
        for grounding in &spec.groundings {
            out.push_str(&format!(
                "- `{}` の `{}` を、`{}` の `{}` がモデル `{}` と照合する\n",
                grounding.implementation.path,
                grounding.implementation.symbol,
                grounding.test.path,
                grounding.test.symbol,
                grounding.model
            ));
        }
    }
    if !spec.rules.is_empty() {
        out.push_str("\n## rule\n");
        for rule in &spec.rules {
            out.push_str(&format!("\n### {}\n\n{}\n\n", rule.id, rule.text.ja));
            for claim in &rule.claims {
                out.push_str(&format!(
                    "- `{}` の `{}` は {}\n",
                    claim.model, claim.check, claim.expect
                ));
            }
            if let Some(reason) = &rule.unformalized {
                out.push_str(&format!("- 形式化しない理由: {reason}\n"));
            }
        }
    }
    out
}

/// Inputs of the `jev-lint docs` manifest (plan 0092).
pub struct Drift<'a> {
    /// The rendered glossary every crate document is compared against.
    pub glossary: &'a str,
    /// Crate documents, compared with the glossary for terminology (J1).
    pub documents: Vec<String>,
    /// Rendered rule lists and their specifications: each rule is compared
    /// with every model its claims name (J2).
    pub rules: Vec<(&'a str, &'a Spec)>,
    /// Control comparisons: (id, mode, reference, target path, heading).
    pub controls: Vec<(&'a str, &'a str, &'a str, &'a str, Option<&'a str>)>,
}

fn source_id(path: &str) -> String {
    path.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

/// The manifest for `jev-lint docs --document-manifest`, derived from the
/// specifications so a new rule or crate document is compared without a
/// hand edit.
pub fn documents_manifest(drift: &Drift<'_>) -> serde_json::Value {
    use serde_json::json;
    let mut files = serde_json::Map::new();
    let mut register = |path: &str, role: &str| {
        files.insert(source_id(path), json!({"path": path, "role": role}));
    };
    register(drift.glossary, "reference");
    for document in &drift.documents {
        register(document, "document");
    }
    let mut comparisons = Vec::new();
    for document in &drift.documents {
        comparisons.push(json!({
            "id": format!("terms-{}", source_id(document)),
            "topic": "idealis の語彙（用語集）の定義・避ける語と、この文書での語の使い方",
            "mode": "terminology",
            "reference": {"source": source_id(drift.glossary)},
            "target": {"source": source_id(document)},
        }));
    }
    for (rendered, spec) in &drift.rules {
        register(rendered, "document");
        for rule in &spec.rules {
            let models: BTreeSet<&str> = rule
                .claims
                .iter()
                .map(|claim| claim.model.as_str())
                .collect();
            for model in models {
                register(model, "reference");
                comparisons.push(json!({
                    "id": format!("rule-{}-{}", rule.id.to_ascii_lowercase(), source_id(model)),
                    "topic": format!("rule {} の文と、モデル {model} の宣言・action・checkの意味", rule.id),
                    "mode": "semantic",
                    "reference": {"source": source_id(model)},
                    "target": {"source": source_id(rendered), "heading": rule.id},
                }));
            }
        }
    }
    for (id, mode, reference, target, heading) in &drift.controls {
        register(target, "document");
        let mut selector = json!({"source": source_id(target)});
        if let Some(heading) = heading {
            selector["heading"] = json!(heading);
        }
        comparisons.push(json!({
            "id": format!("control-{id}"),
            "topic": if *mode == "terminology" {
                "idealis の語彙（用語集）の定義・避ける語と、この文書での語の使い方".to_owned()
            } else {
                format!("rule {} の文と、モデル {reference} の宣言・action・checkの意味", heading.unwrap_or(id))
            },
            "mode": mode,
            "reference": {"source": source_id(reference)},
            "target": selector,
        }));
    }
    let mut roots: Vec<String> = files
        .values()
        .map(|file| file["path"].as_str().unwrap().to_owned())
        .collect();
    roots.sort();
    json!({
        "schema": "jev-lint-documents-manifest-1",
        "roots": roots,
        "files": files,
        "comparisons": comparisons,
    })
}
