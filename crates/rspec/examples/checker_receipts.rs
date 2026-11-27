use rspec::{Checker, CheckerReceipt, ComparisonPlan, compare_receipts, run_checker};
use serde::de::DeserializeOwned;
use serde_json::json;
use std::process::ExitCode;

fn read<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{path}: {error}"))
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [mode, plan, files @ ..] = args.as_slice() else {
        return Err(
            "usage: checker_receipts run PLAN reference|rprob | collect PLAN [RECEIPT...]".into(),
        );
    };
    let plan = read::<ComparisonPlan>(plan)?;
    let answer = match (mode.as_str(), files) {
        ("run", [engine]) => {
            let checker = match engine.as_str() {
                "reference" => Checker::Reference,
                "rprob" => Checker::Rprob,
                _ => return Err("expected reference or rprob".into()),
            };
            let source = std::fs::read_to_string(&plan.model)
                .map_err(|error| format!("{}: {error}", plan.model))?;
            json!(run_checker(&plan, checker, &source, |model, budget| {
                match checker {
                    Checker::Reference => Ok(rmodel::reference::analyze(model, budget)),
                    Checker::Rprob => rprob::Compiled::new(model)
                        .map(|compiled| compiled.analyze(budget))
                        .map_err(|error| error.to_string()),
                }
            })?)
        }
        ("collect", _) => {
            let receipts = files
                .iter()
                .map(|file| read::<CheckerReceipt>(file))
                .collect::<Result<Vec<_>, _>>()?;
            json!(compare_receipts(&plan, &receipts))
        }
        _ => return Err("expected run PLAN reference|rprob or collect PLAN [RECEIPT...]".into()),
    };
    println!("{}", serde_json::to_string_pretty(&answer).expect("JSON"));
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
