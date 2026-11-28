use crate::{fail, load_model, usage};
use idealis_model::reference::{Outcome, analyze};
use serde_json::{Value, json};
use std::process::{Command, Stdio};

pub fn run(args: &[String]) {
    let (path, oracle, budget) = match args {
        [path, oracle] => (path, oracle, idealis_check::Budget::default().states),
        [path, oracle, flag, limit] if flag == "--max-states" => (
            path,
            oracle,
            limit
                .parse()
                .unwrap_or_else(|_| fail("invalid --max-states")),
        ),
        _ => usage(),
    };
    let (model, omitted) = load_model(path);
    if !omitted.is_empty() {
        fail("lean-oracle requires a complete model; the IR has adapter omissions");
    }

    let report = analyze(&model, budget);
    if let Outcome::Violated { depth } = report.declarations {
        fail(&format!("model violates Declarations at depth {depth}"));
    }
    if report.scope != Outcome::Holds {
        let status = if report.scope == Outcome::Empty {
            "empty"
        } else {
            "unknown"
        };
        print_report(&json!({"status": status, "observations": 0}));
        std::process::exit(3);
    }

    let observations =
        idealis_check::conform::reachable_observations(&model).unwrap_or_else(|error| fail(&error));
    if observations.is_empty() {
        fail("the completed model produced no oracle observations");
    }
    let answers = ask(oracle, &observations).unwrap_or_else(|error| fail(&error));
    if let Some(index) = answers.iter().position(|answer| !*answer) {
        print_report(&json!({
            "status": "violated",
            "observations": observations.len(),
            "counterexample": observations[index],
        }));
        std::process::exit(1);
    }
    print_report(&json!({"status": "holds", "observations": observations.len()}));
}

fn print_report(report: &Value) {
    println!("{}", serde_json::to_string_pretty(report).expect("JSON"));
}

fn ask(oracle: &str, observations: &[Value]) -> Result<Vec<bool>, String> {
    let mut child = Command::new("lean")
        .args(["-DwarningAsError=true", "--run", oracle])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start Lean: {error}"))?;
    let input = child.stdin.take().expect("piped");
    // Drain Lean's output while writing the input so either pipe can fill.
    let (output, sent) = std::thread::scope(|scope| {
        let writer = scope.spawn(|| serde_json::to_writer(input, observations));
        let output = child.wait_with_output();
        (output, writer.join().expect("oracle writer"))
    });
    let output = output.map_err(|error| format!("waiting for Lean: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Lean failed ({}): {}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    sent.map_err(|error| format!("sending observations to Lean: {error}"))?;
    let answers: Vec<bool> = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Lean must return a JSON array of booleans: {error}"))?;
    if answers.len() != observations.len() {
        return Err(format!(
            "Lean returned {} answers for {} observations",
            answers.len(),
            observations.len()
        ));
    }
    Ok(answers)
}
