use rcheck::{Kind, conform, project, questions};
use rspec::{Claim, Rule};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::Path;

enum Query<'a> {
    Rule(&'a str),
    Check { model: &'a str, check: &'a str },
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Matched,
    Unknown,
    Unlinked,
    Mismatch,
    Invalid,
}

impl Status {
    fn exit(self) -> i32 {
        match self {
            Self::Matched => 0,
            Self::Mismatch => 1,
            Self::Invalid => 2,
            Self::Unknown | Self::Unlinked => 3,
        }
    }
}

pub(crate) fn run(spec_path: &str, mut flags: &[String]) {
    let mut query = None;
    let mut root = None;
    let mut document = None;
    let mut budget = None;
    while let [flag, args @ ..] = flags {
        match (flag.as_str(), args) {
            ("--rule", [id, rest @ ..]) if query.is_none() => {
                query = Some(Query::Rule(id));
                flags = rest;
            }
            ("--check", [model, check, rest @ ..]) if query.is_none() => {
                query = Some(Query::Check { model, check });
                flags = rest;
            }
            ("--root", [directory, rest @ ..]) if root.is_none() => {
                root = Some(directory.as_str());
                flags = rest;
            }
            ("--document", [path, rest @ ..]) if document.is_none() => {
                document = Some(path.as_str());
                flags = rest;
            }
            ("--max-states", [limit, rest @ ..]) if budget.is_none() => {
                budget = Some(
                    limit
                        .parse()
                        .unwrap_or_else(|_| crate::fail("invalid --max-states")),
                );
                flags = rest;
            }
            _ => crate::usage(),
        }
    }
    let Some(query) = query else { crate::usage() };
    let result = report(
        spec_path,
        query,
        Path::new(root.unwrap_or(".")),
        document,
        budget.unwrap_or_else(|| rcheck::Budget::default().states),
    );
    let (status, output) = result.unwrap_or_else(|error| crate::fail(&error));
    println!("{}", serde_json::to_string_pretty(&output).expect("JSON"));
    if status != Status::Matched {
        std::process::exit(status.exit());
    }
}

fn report(
    spec_path: &str,
    query: Query<'_>,
    root: &Path,
    document: Option<&str>,
    budget: usize,
) -> Result<(Status, Value), String> {
    let read =
        |path: &str| std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"));
    let spec = rspec::from_json(&read(spec_path)?)?;
    let rendered = document.map(read).transpose()?;
    let mut status = Status::Matched;
    let mut rules = Vec::new();
    for rule in &spec.rules {
        if matches!(query, Query::Rule(id) if id != rule.id) {
            continue;
        }
        let mut rule_status = Status::Matched;
        let mut claims = Vec::new();
        for (claim, clause) in rule.claims.iter().map(|claim| (claim, None)).chain(
            rule.clauses
                .iter()
                .filter_map(|clause| clause.claim.as_ref().map(|claim| (claim, Some(clause)))),
        ) {
            if matches!(query, Query::Check { model, check } if claim.model != model || claim.check != check)
            {
                continue;
            }
            let mut row = json!({
                "model": claim.model,
                "check": claim.check,
                "expected": claim.expect,
            });
            if let Some(clause) = clause {
                row["clause"] = json!({"id": clause.id, "text": clause.text});
            }
            let claim_status = match evidence(root, claim, budget) {
                Ok(evidence) => {
                    let broken = claim.check != rmodel::DECLARATIONS
                        && evidence["diagnostics"].as_array().is_some_and(|entries| {
                            entries
                                .iter()
                                .any(|entry| entry["kind"] == "declaration_violation")
                        });
                    let status = match evidence["verdict"].as_str() {
                        _ if broken => Status::Invalid,
                        Some("empty" | "unknown") => Status::Unknown,
                        Some(verdict) if verdict == claim.expect => Status::Matched,
                        _ => Status::Mismatch,
                    };
                    row["evidence"] = evidence;
                    status
                }
                Err(error) => {
                    row["diagnostic"] = json!(error);
                    Status::Invalid
                }
            };
            row["status"] = json!(claim_status);
            rule_status = rule_status.max(claim_status);
            claims.push(row);
        }
        if claims.is_empty() {
            if matches!(query, Query::Check { .. }) {
                continue;
            }
            rule_status = Status::Unlinked;
        }
        let mut row =
            json!({"id": rule.id, "text": rule.text, "status": rule_status, "claims": claims});
        if let (Some(path), Some(rendered)) = (document, rendered.as_deref()) {
            row["document"] = locate_rule(path, rendered, rule)?;
        }
        if let Some(reason) = &rule.unformalized {
            row["unformalized"] = json!(reason);
        }
        rules.push(row);
        status = status.max(rule_status);
    }
    if rules.is_empty() {
        if let Query::Rule(id) = query {
            return Err(format!("{spec_path}: no rule named {id}"));
        }
        status = Status::Unlinked;
    }
    Ok((
        status,
        json!({"spec": spec.id, "source": spec_path, "status": status, "rules": rules}),
    ))
}

fn locate_rule(path: &str, rendered: &str, rule: &Rule) -> Result<Value, String> {
    let heading = format!("### {}", rule.id);
    let mut matches = rendered
        .lines()
        .enumerate()
        .filter(|(_, line)| *line == heading);
    let (Some((line, _)), None) = (matches.next(), matches.next()) else {
        return Err(format!("{path}: expected one heading for {}", rule.id));
    };
    let expected = format!("\n{}\n\n", rule.text.ja);
    if !rendered
        .lines()
        .skip(line + 1)
        .take(expected.lines().count())
        .eq(expected.lines())
    {
        return Err(format!(
            "{path}: rendered rule {} differs from the specification",
            rule.id
        ));
    }
    Ok(json!({"path": path, "line": line + 1}))
}

fn evidence(root: &Path, claim: &Claim, budget: usize) -> Result<Value, String> {
    let path = root.join(&claim.model);
    let (model, _) = project::load_model_source(&path.to_string_lossy())?;
    let question = questions(&model)
        .into_iter()
        .find(|question| question.name == claim.check)
        .ok_or_else(|| format!("{}: no check named {}", claim.model, claim.check))?;
    let valid = match question.kind {
        Kind::Reachable => matches!(claim.expect.as_str(), "reachable" | "unreachable"),
        _ => matches!(claim.expect.as_str(), "holds" | "violated"),
    };
    if !valid {
        return Err(format!(
            "{}: invalid expectation {} for {}",
            claim.model, claim.expect, claim.check
        ));
    }
    conform::witness(&model, &claim.check, budget)
}
