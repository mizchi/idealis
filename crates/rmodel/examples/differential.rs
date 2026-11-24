//! differential <first-seed> <last-seed> <output-dir>: generates a typed
//! rmodel model per seed and compares the reference evaluator, rtlc on the
//! TLA+ lowering and ralloy's SAT on the Alloy lowering (plan 0097).
//! Writes one JSON line per check to results.jsonl, and the source of every
//! model with a disagreement or an engine error to models/<seed>.model.
use std::fmt::Write as _;

#[path = "support/generator.rs"]
mod generator;
use generator::generate;

fn reference_word(outcome: rmodel::reference::Outcome) -> (String, Option<usize>) {
    use rmodel::reference::Outcome;
    match outcome {
        Outcome::Empty => ("empty".into(), None),
        Outcome::Holds => ("holds".into(), None),
        Outcome::Violated { depth } => ("violated".into(), Some(depth)),
        Outcome::Reachable { depth } => ("reachable".into(), Some(depth)),
        Outcome::Unreachable => ("unreachable".into(), None),
        Outcome::Unknown => ("unknown".into(), None),
    }
}

/// rtlc's verdict; a negative one is unknown when a state leaves the
/// integer scope (plan 0103).
fn explicit(model: &rmodel::Model, name: &str, kind: rmodel::CheckKind) -> String {
    let verdict = explicit_raw(model, name, kind);
    if model.has_values()
        && (verdict == "holds" || verdict == "unreachable")
        && explicit_raw(model, rmodel::SCOPE, rmodel::CheckKind::Invariant) != "holds"
    {
        return "unknown".into();
    }
    verdict
}

fn explicit_raw(model: &rmodel::Model, name: &str, kind: rmodel::CheckKind) -> String {
    let Ok((source, config)) = rmodel::to_tla(model, name) else {
        return "lowering-error".into();
    };
    let program = match idealis_core::compile_tla_with_options(
        &source,
        &config,
        idealis_core::CompileOptions {
            irVersion: idealis_core::IrVersion::V2,
        },
    ) {
        Ok(program) => program,
        Err(error) => return format!("error: {}", error.message),
    };
    match program.check(idealis_core::SafetyBounds {
        max_depth: 1_000,
        max_states: 200_000,
        max_work: 50_000_000,
        max_temporal_nodes: None,
    }) {
        Err(error) => format!("error: {}", error.message),
        Ok(result) => match (result.status.as_str(), result.violation.is_some(), kind) {
            ("complete", _, rmodel::CheckKind::Invariant) => "holds".into(),
            ("complete", _, rmodel::CheckKind::Reachable) => "unreachable".into(),
            (_, true, rmodel::CheckKind::Invariant) => "violated".into(),
            (_, true, rmodel::CheckKind::Reachable) => "reachable".into(),
            (other, _, _) => other.into(),
        },
    }
}

fn sat(alloy: &Result<ralloy::Model, String>, name: &str, kind: rmodel::CheckKind) -> String {
    let alloy = match alloy {
        Ok(alloy) => alloy,
        Err(error) => return format!("error: {error}"),
    };
    let label = rmodel::alloy_label(name);
    let Some(index) = alloy.commands().iter().position(|c| c.label == label) else {
        return "error: no command".into();
    };
    match alloy.analyze(index, 5_000_000) {
        Err(error) => format!("error: {error}"),
        Ok(analysis) => match analysis.status {
            ralloy::Status::Counterexample => "violated".into(),
            ralloy::Status::NoCounterexample => "holds".into(),
            ralloy::Status::Instance => "reachable".into(),
            // A check without instances has inconsistent facts: empty. A run
            // without one is empty or unreachable; compared below.
            ralloy::Status::NoInstance if kind == rmodel::CheckKind::Invariant => "empty".into(),
            ralloy::Status::NoInstance => "unreachable".into(),
            ralloy::Status::Unknown => "unknown".into(),
        },
    }
}

/// One seed's rows. Each seed runs in its own process: memory grew across
/// seeds in one process until the system killed it (plan 0100), and a crash
/// must not lose or pass the other seeds.
fn one(seed: u64, output: &str) -> String {
    let mut lines = String::new();
    let source = generate(seed);
    let model = match rmodel::parse(&source) {
        Ok(model) => model,
        Err(error) => {
            let _ = writeln!(
                lines,
                "{{\"seed\":{seed},\"generator\":{:?}}}",
                error.to_string()
            );
            std::fs::write(format!("{output}/models/{seed}.model"), &source).unwrap();
            return lines;
        }
    };
    let report = rmodel::reference::analyze(&model, 200_000);
    // The model's checks and the built-in multiplicity check (plan 0099).
    let mut checks: Vec<(String, rmodel::CheckKind, rmodel::reference::Outcome)> = model
        .checks
        .iter()
        .zip(&report.verdicts)
        .map(|(check, verdict)| (check.name.clone(), check.kind, verdict.outcome))
        .collect();
    checks.push((
        rmodel::DECLARATIONS.to_owned(),
        rmodel::CheckKind::Invariant,
        report.declarations,
    ));
    // Models with integer or record values have no Alloy lowering.
    let alloy = rmodel::alloy_supports(&model)
        .ok()
        .map(|()| ralloy::Model::parse(&rmodel::to_alloy(&model)).map_err(|e| e.to_string()));
    let mut keep = false;
    for (name, kind, outcome) in &checks {
        let (reference, depth) = reference_word(*outcome);
        let explicit = explicit(&model, name, *kind);
        let sat = alloy
            .as_ref()
            .map_or("unsupported".to_owned(), |alloy| sat(alloy, name, *kind));
        let deep = depth.is_some_and(|d| d >= model.scope.steps);
        let explicit_agrees = explicit == reference;
        let sat_agrees = sat == "unsupported"
            || sat == reference
            || deep
            || reference == "empty" && sat == "unreachable";
        if !explicit_agrees || !sat_agrees || reference == "unknown" {
            keep = true;
        }
        let _ = writeln!(
            lines,
            "{{\"seed\":{seed},\"check\":{:?},\"reference\":{reference:?},\"depth\":{},\"explicit\":{explicit:?},\"sat\":{sat:?},\"deep\":{deep},\"explicitAgrees\":{explicit_agrees},\"satAgrees\":{sat_agrees}}}",
            name,
            depth.map_or("null".into(), |d| d.to_string()),
        );
    }
    if keep {
        std::fs::write(format!("{output}/models/{seed}.model"), &source).unwrap();
    }
    lines
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, seed, output] = arguments.as_slice()
        && flag == "--one"
    {
        print!("{}", one(seed.parse().unwrap(), output));
        return;
    }
    let [first, last, output] = arguments.as_slice() else {
        panic!("usage: differential <first-seed> <last-seed> <output-dir>");
    };
    let (first, last): (u64, u64) = (first.parse().unwrap(), last.parse().unwrap());
    std::fs::create_dir_all(format!("{output}/models")).unwrap();
    // Rows are appended per seed, so an interrupted run keeps what it did.
    let mut file = std::fs::File::create(format!("{output}/results.jsonl")).unwrap();
    let program = std::env::current_exe().unwrap();
    for seed in first..=last {
        use std::io::Write as _;
        let mut child = std::process::Command::new(&program)
            .args(["--one", &seed.to_string(), output])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        // The engines' budgets count work, not time; a seed may still spin
        // (seed 6130, plan 0100), so each seed gets a wall-clock limit.
        let started = std::time::Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break Some(status);
            }
            if started.elapsed() > std::time::Duration::from_secs(600) {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        };
        let mut stdout = Vec::new();
        {
            use std::io::Read as _;
            child
                .stdout
                .take()
                .unwrap()
                .read_to_end(&mut stdout)
                .unwrap();
        }
        if status.is_some_and(|status| status.success()) {
            file.write_all(&stdout).unwrap();
        } else {
            // Neither agreement nor disagreement: recorded as a crash.
            let reason = status.map_or("timeout after 600s".to_owned(), |s| s.to_string());
            writeln!(file, "{{\"seed\":{seed},\"crash\":{reason:?}}}").unwrap();
            std::fs::write(format!("{output}/models/{seed}.model"), generate(seed)).unwrap();
        }
    }
}
