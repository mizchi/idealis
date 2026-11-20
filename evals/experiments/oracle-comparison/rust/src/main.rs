use roracle::{BenchConfig, CheckReport, Observation, OracleGroup, Status};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::cell::Cell;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct Input {
    name: String,
    source: String,
    config: String,
    module: String,
    directory: String,
}
#[derive(Deserialize)]
struct Manifest {
    inputs: Vec<Input>,
    java: String,
    jar: String,
    node: String,
    native: String,
    worker: String,
}
fn save(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    Ok(())
}
fn project(value: &Value) -> std::result::Result<Value, String> {
    if value["status"] != "complete" || !value["states"].is_u64() || !value["generated"].is_u64() {
        return Err(format!(
            "incomplete or malformed checker observation: {value}"
        ));
    }
    Ok(json!({"status":"complete", "states":value["states"], "generated":value["generated"]}))
}
fn invoke(
    engine: &str,
    input: &Input,
    manifest: &Manifest,
    logs: &Path,
    invocation: &Cell<u64>,
) -> Observation<Value> {
    let result = (|| -> Result<Value> {
        let index = invocation.get();
        invocation.set(index + 1);
        let scratch = logs.join(format!("scratch-{index}"));
        let mut command = if engine == "tlc" {
            let mut command = Command::new(&manifest.java);
            command
                .args([
                    "-XX:+UseParallelGC",
                    "-jar",
                    &manifest.jar,
                    "-workers",
                    "1",
                    "-seed",
                    "16",
                    "-cleanup",
                    "-metadir",
                ])
                .arg(&scratch)
                .arg("-config")
                .arg(format!("{}.cfg", input.module))
                .arg(format!("{}.tla", input.module))
                .current_dir(&input.directory);
            command
        } else if engine == "idealis" {
            Command::new(&manifest.native)
        } else {
            let mut command = Command::new(&manifest.node);
            command
                .arg(&manifest.worker)
                .arg(Path::new(&input.directory).join("input.json"));
            command
        };
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn()?;
        if engine == "idealis" {
            let request = json!({"version":1,"request":{"kind":"check-tla", "source":input.source, "config":input.config,
                "bounds":{"maxDepth":10000000,"maxStates":10000000,"maxWork":4000000000_u64}}});
            let mut stdin = child.stdin.take().ok_or("missing stdin")?;
            serde_json::to_writer(&mut stdin, &request)?;
            stdin.write_all(b"\n")?;
        } else {
            drop(child.stdin.take());
        }
        let output = child.wait_with_output()?;
        let stdout = String::from_utf8(output.stdout)?;
        let stderr = String::from_utf8(output.stderr)?;
        save(
            &logs.join(format!("{index:04}-{engine}.json")),
            &json!({"engine":engine,"input":input.name,"exit":output.status.code(),"stdout":stdout,"stderr":stderr}),
        )?;
        if scratch.exists() {
            fs::remove_dir_all(scratch)?;
        }
        if !output.status.success() {
            return Err(format!("{engine} exited {}: {stderr}", output.status).into());
        }
        let value = if engine == "tlc" {
            if !stdout.contains("Model checking completed. No error has been found")
                || !stdout.contains("5a47802")
            {
                return Err(stdout.into());
            }
            let line = stdout
                .lines()
                .find(|line| {
                    line.contains(" states generated, ") && line.contains(" distinct states found")
                })
                .ok_or("missing TLC counts")?;
            let words = line.split_whitespace().collect::<Vec<_>>();
            json!({"status":"complete", "generated":words[0].replace(',', "").parse::<u64>()?, "states":words[3].replace(',', "").parse::<u64>()?})
        } else {
            let reply: Value = serde_json::from_str(&stdout)?;
            if engine == "idealis" {
                reply["result"].clone()
            } else {
                reply
            }
        };
        project(&value).map_err(Into::into)
    })();
    Observation::from(result)
}
fn group<'a>(
    manifest: &'a Manifest,
    logs: &'a Path,
    invocation: &'a Cell<u64>,
) -> OracleGroup<'a, Input, Value> {
    let mut group = OracleGroup::new("cold-source-to-result");
    for input in &manifest.inputs {
        group.case(&input.name, input.clone());
    }
    group.reference("tlc-5a47802", |input| {
        invoke("tlc", input, manifest, logs, invocation)
    });
    for engine in ["idealis", "tla-ts", "tlc"] {
        group.test_function(engine, move |input| {
            invoke(engine, input, manifest, logs, invocation)
        });
    }
    group
}
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|arg| arg == "--probe") {
        println!("{}", oracle_comparison::oracle_probe());
        return Ok(());
    }
    let directory = PathBuf::from(args.get(1).ok_or("run directory required")?);
    let manifest: Manifest = serde_json::from_slice(&fs::read(directory.join("input.json"))?)?;
    let replay = args.get(2).is_some_and(|arg| arg == "--replay");
    let logs = directory.join(if replay { "replay-logs" } else { "logs" });
    fs::create_dir(&logs)?;
    let invocation = Cell::new(0);
    let mut cold = group(&manifest, &logs, &invocation);
    if replay {
        let saved: CheckReport<Input, Value> =
            serde_json::from_slice(&fs::read(directory.join("check.json"))?)?;
        let actual = cold.replay(&saved)?;
        actual.assert_matched();
        assert_eq!(
            invocation.get(),
            6,
            "replay runs only targets, not the saved oracle"
        );
        save(&directory.join("replay.json"), &actual)?;
        println!("recorded TLC oracle replay matched: 6 target invocations");
        return Ok(());
    }
    let check = cold.check()?;
    save(&directory.join("check.json"), &check)?;
    check.assert_matched();
    // Fix correctness expectations once; prepared benchmarks do not invoke a process.
    let programs = manifest
        .inputs
        .iter()
        .map(|input| {
            Ok(rtla_compiler::compile(
                &rtla::parse_module(&input.source)?,
                &rtla::parse_config(&input.config)?,
            )?)
        })
        .collect::<Result<Vec<_>>>()?;
    let bounds = rtlc::SafetyBounds {
        max_depth: 10000000,
        max_states: 10000000,
        max_work: 4000000000,
        max_temporal_nodes: None,
    };
    let mut prepared = OracleGroup::new("prepared-rust-check");
    for (index, input) in manifest.inputs.iter().enumerate() {
        prepared.case(&input.name, index);
    }
    prepared
        .reference("saved-tlc-5a47802", |index| {
            check.cases[*index].reference.clone()
        })
        .test_function("check", |index| {
            Observation::from(
                programs[*index]
                    .check(bounds)
                    .map_err(|error| error.to_string())
                    .and_then(|value| project(&serde_json::to_value(value).unwrap())),
            )
        })
        .test_function("coverage", |index| {
            Observation::from(
                programs[*index]
                    .check_with_coverage(bounds)
                    .map_err(|error| error.to_string())
                    .and_then(|value| project(&serde_json::to_value(value.result).unwrap())),
            )
        });
    // All compilation and probe checks precede either timing run.
    assert_eq!(oracle_comparison::oracle_probe(), 51);
    prepared.check()?.assert_matched();
    let cold_bench = cold.bench(BenchConfig {
        baseline: "tlc".into(),
        scope:
            "cold source-to-result including process startup, transport, parsing and raw-log writes"
                .into(),
        samples: 7,
        warmup: 1,
        iterations: 1,
        max_evaluations: 100,
    })?;
    save(&directory.join("cold-bench.json"), &cold_bench)?;
    cold_bench.assert_matched();
    let prepared_bench = prepared.bench(BenchConfig {
        baseline: "check".into(),
        scope:
            "prepared Rust check and JSON projection; excludes parsing/compilation and comparison"
                .into(),
        samples: 20,
        warmup: 3,
        iterations: 10,
        max_evaluations: 1000,
    })?;
    save(&directory.join("prepared-bench.json"), &prepared_bench)?;
    prepared_bench.assert_matched();
    let expected = check.cases[0].reference.clone();
    let mut controls = OracleGroup::new("controls");
    controls
        .case("ring", ())
        .reference("saved-tlc", |_| expected.clone())
        .test_function("corrupt-count", |_| {
            Observation::value(json!({"status":"complete","states":999,"generated":1001}))
        })
        .test_function("unknown", |_| Observation::Unknown {
            reason: "injected budget exhaustion".into(),
        })
        .test_function("crash", |_| Observation::Error {
            message: "injected process error".into(),
        });
    let faults = controls.check()?;
    assert_eq!(
        faults.cases[0]
            .comparisons
            .iter()
            .map(|value| value.status)
            .collect::<Vec<_>>(),
        vec![Status::Mismatch, Status::Unknown, Status::Error]
    );
    save(&directory.join("controls.json"), &faults)?;
    let calls = Cell::new(0);
    let mut drift = OracleGroup::new("measured-drift");
    drift
        .case("input", ())
        .reference("constant", |_| Observation::value(1))
        .test_function("stable", |_| Observation::value(1))
        .test_function("drift", |_| {
            calls.set(calls.get() + 1);
            Observation::value(if calls.get() <= 2 { 1 } else { 2 })
        });
    let mut tick = 0;
    let drift = drift.bench_with_clock(
        BenchConfig {
            baseline: "stable".into(),
            scope:
                "synthetic drift control with deterministic logical clock; not a performance sample"
                    .into(),
            samples: 7,
            warmup: 1,
            ..BenchConfig::default()
        },
        || {
            tick += 10;
            tick
        },
    )?;
    save(&directory.join("drift.json"), &drift)?;
    assert_eq!(drift.status, Status::Mismatch);
    assert!(
        drift.cases[0]
            .measurements
            .iter()
            .all(|value| value.speedup.is_none())
    );
    println!("cold and prepared matched; corruption/unknown/error/drift controls detected");
    Ok(())
}
