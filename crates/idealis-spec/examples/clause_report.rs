//! Report the evidence for a rule in a Pkl specification.
use idealis_spec::{
    ComparisonPlan, EvaluationRevision, Evidence, GoalMeasurement, SemanticComparison,
};
use serde::de::DeserializeOwned;
use serde_json::json;
use std::{path::Path, process::Command};

fn read_evidence<T: DeserializeOwned>(path: &str) -> Result<Evidence<T>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    serde_json::from_str(&text).map_err(|error| format!("{path}: {error}"))
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [pkl, rule, mode @ ..] = args.as_slice() else {
        return Err("usage: clause_report <spec.pkl> <rule-id> [--pin CANDIDATE | --plan CANDIDATE MAX_STATES | --assess CANDIDATE COMPARISON.json GOAL.json]".into());
    };
    let output = Command::new("pkl")
        .args(["eval", "-f", "json", pkl])
        .output()
        .map_err(|error| format!("pkl: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let data = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    let spec = idealis_spec::from_json(&data)?;
    let report = idealis_spec::clause_report(&spec, Path::new("."), rule, 2_000_000)?;
    let output = match mode {
        [] => json!(report),
        [flag, candidate] if flag == "--pin" => {
            json!(EvaluationRevision::for_report(candidate, &report))
        }
        [flag, candidate, budget] if flag == "--plan" => {
            let max_states = budget.parse().map_err(|_| "invalid exploration budget")?;
            json!(ComparisonPlan::pin(candidate, &report, max_states)?)
        }
        [flag, candidate, comparison, goal] if flag == "--assess" => {
            let current = EvaluationRevision::for_report(candidate, &report);
            let comparison = read_evidence::<SemanticComparison>(comparison)?;
            let goal = read_evidence::<GoalMeasurement>(goal)?;
            let review = Evidence {
                revision: current.clone(),
                outcome: &report,
            };
            let blockers = idealis_spec::assess_adoption(&current, &review, &comparison, &goal)
                .err()
                .unwrap_or_default();
            json!({"ready": blockers.is_empty(), "blockers": blockers, "revision": current, "report": report})
        }
        _ => {
            return Err(
                "expected --pin CANDIDATE, --plan CANDIDATE MAX_STATES, or --assess CANDIDATE COMPARISON.json GOAL.json".into(),
            );
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output).expect("report is JSON")
    );
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
