use super::{
    Result,
    modules::{Bundle, from_cwd, with_suffix},
};
use rtlc::{SafetyBounds, SafetyOptions};
use std::path::Path;

pub fn run(mut args: impl Iterator<Item = String>) -> Result<u8> {
    let mut spec = None;
    let mut config = None;
    let mut workers = 1;
    let mut difftrace = false;
    let mut fulltrace = false;
    let mut expect_violation = false;
    let mut no_deadlock = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "-help" => {
                println!(
                    "Usage: idealis tlc [-config file] [-workers N|auto] [-modelcheck] [-deadlock] [-difftrace|--fulltrace] [--expect-violation] spec"
                );
                return Ok(1);
            }
            "--fulltrace" => fulltrace = true,
            "--expect-violation" => expect_violation = true,
            "-modelcheck" => {}
            "-difftrace" => difftrace = true,
            "-deadlock" => no_deadlock = true,
            "-config" => {
                config = Some(
                    args.next()
                        .filter(|s| !s.starts_with('-'))
                        .ok_or((1, "-config requires a file".into()))?,
                )
            }
            "-workers" => {
                let value = args.next().ok_or((1, "-workers requires a count".into()))?;
                workers = if value.trim().eq_ignore_ascii_case("auto") {
                    if cfg!(target_arch = "wasm32") {
                        1
                    } else {
                        std::thread::available_parallelism().map_or(1, |n| n.get().min(256))
                    }
                } else {
                    value
                        .parse()
                        .ok()
                        .filter(|n| (1..=256).contains(n))
                        .ok_or((1, format!("Unsupported worker count: {value}")))?
                };
                if cfg!(target_arch = "wasm32") && workers != 1 {
                    return Err((1, "The npm Wasm build supports -workers 1 or auto".into()));
                }
            }
            flag if flag.starts_with('-') => {
                return Err((1, format!("Unsupported TLC option: {flag}")));
            }
            _ if spec.is_some() => return Err((1, "Expected one root module".into())),
            _ => spec = Some(from_cwd(&with_suffix(&arg, ".tla"))),
        }
    }
    let path = spec.ok_or((1, "Usage: idealis tlc [options] spec".into()))?;

    let config_path = match config {
        None => path.with_extension("cfg"),
        Some(file) => {
            let cfg = with_suffix(&file, ".cfg");
            if from_cwd(&cfg).is_file() {
                from_cwd(&cfg)
            } else {
                path.parent().unwrap_or(Path::new(".")).join(cfg)
            }
        }
    };
    let text = std::fs::read_to_string(&config_path)
        .map_err(|e| (255, format!("{}: {e}", config_path.display())))?;
    let mut config =
        rtla::parse_config(&text).map_err(|e| (151, format!("{}: {e}", config_path.display())))?;
    if no_deadlock {
        config.check_deadlock = false;
    }
    let bundle = Bundle::load(&path, false).map_err(|msg| (150, msg))?;
    // Check the source before lowering: the executable IR omits proof declarations.
    rtla_compiler::analyze_modules(&bundle.root, &bundle.dependencies, true)
        .map_err(|e| (150, e.to_string()))?;
    let program = rtla_compiler::compile_modules(&bundle.root, &config, &bundle.dependencies)
        .map_err(|e| (150, e.to_string()))?;
    let started = std::time::Instant::now();
    let check = program.check_with_options(
        SafetyBounds {
            max_depth: 1000,
            max_states: 100000,
            max_work: 10000000,
            max_temporal_nodes: Some(100000),
        },
        SafetyOptions {
            workers,
            ..Default::default()
        },
    );
    let result = match check {
        Ok(result) => result,
        // rtlc exposes this configuration diagnostic through its portable protocol.
        Err(error)
            if error.code == "protocol"
                && error.message == "Constant invariant or property is false" =>
        {
            println!("Error: Constant invariant or property is equal to FALSE");
            return Ok(151);
        }
        Err(error) => {
            if let Some((module, at)) = rtla_compiler::incomplete_state_location(
                &error,
                std::iter::once(&bundle.root).chain(&bundle.dependencies),
            ) {
                return Err((
                    255,
                    format!("{error} ({module}.tla:{}:{})", at.line, at.column),
                ));
            }
            return Err((255, error.to_string()));
        }
    };
    let code = match result.status.as_str() {
        "complete" => {
            println!("Model checking completed. No error has been found.");
            0
        }
        "assumption-violation" => 10,
        "deadlock" => 11,
        "invariant-violation" => 12,
        "property-violation" | "liveness-violation" => 13,
        _ => {
            return Err((
                255,
                format!(
                    "Model checking incomplete: {} ({})",
                    result.status,
                    result.reason.as_deref().unwrap_or("no result")
                ),
            ));
        }
    };
    if let Some(violation) = result.violation {
        match result.status.as_str() {
            "invariant-violation" if difftrace && violation.trace.len() == 1 => println!(
                "Error: Invariant {} is violated by the initial state:",
                violation.name
            ),
            "liveness-violation" if difftrace => {
                println!("Error: Temporal properties were violated.")
            }
            "invariant-violation" => println!("Error: Invariant {} is violated.", violation.name),
            "deadlock" => println!("Error: Deadlock reached."),
            _ => println!("Error: {}: {}", result.status, violation.name),
        }
        if fulltrace || difftrace {
            print!(
                "{}",
                violation.format_trace(&program.variables, &result.status, !fulltrace)
            );
        } else {
            println!(
                "{}",
                serde_json::to_string(&violation).expect("serializable counterexample")
            );
        }
    }
    println!(
        "{} distinct states found. depth {}, {} ms",
        result.states,
        result.depth,
        started.elapsed().as_millis()
    );
    if expect_violation && code == 0 {
        return Err((
            1,
            "Expected a violation; model checking completed without one".into(),
        ));
    }
    Ok(if expect_violation && (11..=13).contains(&code) {
        0
    } else {
        code
    })
}
