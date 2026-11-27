use rmodel::reference::Report;
use std::process::ExitCode;

fn check() -> Result<Report, String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (path, budget) = match args.as_slice() {
        [path] => (path, 1_000_000),
        [path, flag, value] if flag == "--max-states" => {
            let budget = value.parse().map_err(|_| "invalid --max-states")?;
            (path, budget)
        }
        _ => return Err("usage: rprob <model> [--max-states N]".into()),
    };
    let source = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let model = rmodel::parse(&source).map_err(|error| error.to_string())?;
    let compiled = rprob::Compiled::new(&model).map_err(|error| format!("unsupported: {error}"))?;
    Ok(compiled.analyze(budget))
}

fn main() -> ExitCode {
    match check() {
        Ok(report) => {
            println!(
                "Scope: {:?}\nDeclarations: {:?}",
                report.scope, report.declarations
            );
            for verdict in report.verdicts {
                println!("{}: {:?}", verdict.check, verdict.outcome);
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
