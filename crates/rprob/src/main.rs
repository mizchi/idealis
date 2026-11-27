use rprob::SafetyAnalysis;
use std::process::ExitCode;

fn check() -> Result<SafetyAnalysis, String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (safety, args) = match args.split_first() {
        Some((flag, rest)) if flag == "--safety" => (true, rest),
        _ => (false, args.as_slice()),
    };
    let (path, budget) = match args {
        [path] => (path, 1_000_000),
        [path, flag, value] if flag == "--max-states" => {
            let budget = value.parse().map_err(|_| "invalid --max-states")?;
            (path, budget)
        }
        _ => return Err("usage: rprob [--safety] <model> [--max-states N]".into()),
    };
    let source = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let model = rmodel::parse(&source).map_err(|error| error.to_string())?;
    if !safety && !model.temporal.is_empty() {
        return Err(
            "unsupported: temporal obligations are outside the finite atom fragment".into(),
        );
    }
    rprob::analyze_safety(&model, budget).map_err(|error| format!("unsupported: {error}"))
}

fn main() -> ExitCode {
    match check() {
        Ok(SafetyAnalysis {
            report,
            omitted_temporal,
        }) => {
            println!(
                "Scope: {:?}\nDeclarations: {:?}",
                report.scope, report.declarations
            );
            for verdict in report.verdicts {
                println!("{}: {:?}", verdict.check, verdict.outcome);
            }
            for name in omitted_temporal {
                println!("{name}: unsupported (temporal)");
            }
            for (action, enabled) in report.enabled {
                println!("action {action}: {enabled:?}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}
