use rmodel::reference::{Outcome, SearchBudget, analyze_actions_with_budget};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// Check the declared behavior contracts in the finite linked project.
pub fn check_contracts(path: &str, budget: SearchBudget) -> Result<Value, String> {
    if !super::is_manifest(path) {
        return Err("contracts requires a .rmodel.json project".into());
    }
    let source = super::read_source(path)?;
    let located = source.located_model(path)?;
    let model = &located.model;
    let report = analyze_actions_with_budget(model, budget);
    let outcomes: BTreeMap<_, _> = report
        .verdicts
        .into_iter()
        .map(|verdict| (verdict.check, verdict.outcome))
        .collect();
    let broken = matches!(report.declarations, Outcome::Violated { .. });
    let mut diagnostics = crate::reference_diagnostics(report.scope, report.stop.as_ref())
        .into_iter()
        .map(|diagnostic| json!(diagnostic))
        .collect::<Vec<_>>();
    if let Outcome::Violated { depth } = report.declarations {
        diagnostics.push(json!({"kind":"declaration_violation", "depth":depth}));
    }
    let mut components = Map::new();
    let mut matched = !broken && report.scope != Outcome::Empty;
    let mut mismatch = false;
    for (namespace, contract) in &source.contracts {
        let rows = |names: &[String]| -> Result<Vec<Value>, String> {
            names
                .iter()
                .map(|name| {
                    let check = format!("{namespace}_{name}");
                    let outcome = outcomes
                        .get(&check)
                        .ok_or_else(|| format!("no linked check named {check}"))?;
                    let verdict = match outcome {
                        Outcome::Holds | Outcome::Unreachable if broken => "unknown",
                        Outcome::Holds => "holds",
                        Outcome::Unreachable => "unreachable",
                        Outcome::Violated { .. } => "violated",
                        Outcome::Reachable { .. } => "reachable",
                        Outcome::Empty => "empty",
                        Outcome::Unknown => "unknown",
                    };
                    let position = source.position(located.declarations[&check]);
                    let mut row = json!({"check":check, "source":position, "verdict":verdict});
                    if matches!(verdict, "violated" | "reachable") {
                        row["evidence"] =
                            crate::conform::witness_with_budget(model, &check, budget)?;
                    }
                    Ok(row)
                })
                .collect()
        };
        let assumptions = rows(&contract.assumptions)?;
        let guarantees = rows(&contract.guarantees)?;
        let exercises = rows(&contract.exercises)?;
        let status = if assumptions.iter().any(|row| row["verdict"] == "violated") {
            "assumption_failed"
        } else if exercises.iter().any(|row| row["verdict"] == "unreachable") {
            "exercise_unreachable"
        } else if guarantees.iter().any(|row| row["verdict"] == "violated") {
            "guarantee_failed"
        } else if !(assumptions.is_empty() && guarantees.is_empty() && exercises.is_empty())
            && assumptions
                .iter()
                .chain(&guarantees)
                .all(|row| row["verdict"] == "holds")
            && exercises.iter().all(|row| row["verdict"] == "reachable")
        {
            "matched"
        } else {
            "inconclusive"
        };
        matched &= status == "matched";
        mismatch |= matches!(
            status,
            "assumption_failed" | "exercise_unreachable" | "guarantee_failed"
        );
        components.insert(namespace.clone(), json!({
            "status":status, "assumptions":assumptions, "guarantees":guarantees, "exercises":exercises,
        }));
    }
    let status = if mismatch {
        "mismatch"
    } else if matched && !components.is_empty() {
        "matched"
    } else {
        "inconclusive"
    };
    Ok(json!({"status":status, "components":components, "diagnostics":diagnostics}))
}
