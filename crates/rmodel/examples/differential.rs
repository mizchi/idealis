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

fn explicit(model: &rmodel::Model, name: &str, kind: rmodel::CheckKind) -> String {
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

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [first, last, output] = arguments.as_slice() else {
        panic!("usage: differential <first-seed> <last-seed> <output-dir>");
    };
    let (first, last): (u64, u64) = (first.parse().unwrap(), last.parse().unwrap());
    std::fs::create_dir_all(format!("{output}/models")).unwrap();
    let mut lines = String::new();
    for seed in first..=last {
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
                continue;
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
        let alloy = ralloy::Model::parse(&rmodel::to_alloy(&model)).map_err(|e| e.to_string());
        let mut keep = false;
        for (name, kind, outcome) in &checks {
            let (reference, depth) = reference_word(*outcome);
            let explicit = explicit(&model, name, *kind);
            let sat = sat(&alloy, name, *kind);
            let deep = depth.is_some_and(|d| d >= model.scope.steps);
            let explicit_agrees = explicit == reference;
            let sat_agrees =
                sat == reference || deep || reference == "empty" && sat == "unreachable";
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
    }
    std::fs::write(format!("{output}/results.jsonl"), lines).unwrap();
}
