use roracle::{BenchConfig, BenchFailure, CheckReport, Observation, OracleGroup, Status};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::rc::Rc;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Requirement {
    pointer: String,
    value: Value,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    request: Value,
    requirements: Vec<Requirement>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Executable {
    path: String,
    args: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    version: u32,
    name: String,
    mode: String,
    baseline: Executable,
    candidate: Executable,
    cases: Vec<Case>,
    config: BenchConfig,
}
struct Running {
    child: Child,
    input: Option<BufWriter<ChildStdin>>,
    output: BufReader<ChildStdout>,
}
impl Running {
    fn start(executable: &Executable) -> Result<Self> {
        let mut child = Command::new(&executable.path)
            .args(&executable.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let input = child.stdin.take().ok_or("missing process stdin")?;
        let output = BufReader::new(child.stdout.take().ok_or("missing process stdout")?);
        Ok(Self {
            child,
            input: Some(BufWriter::new(input)),
            output,
        })
    }
    fn finish(&mut self) -> Result<()> {
        self.input.take();
        let mut trailing = String::new();
        self.output.read_to_string(&mut trailing)?;
        let status = self.child.wait()?;
        if !status.success() || !trailing.trim().is_empty() {
            return Err(format!(
                "process close status {status}, unexpected trailing output: {trailing}"
            )
            .into());
        }
        Ok(())
    }
    fn request(&mut self, request: &Value) -> Result<Value> {
        let input = self.input.as_mut().ok_or("process input is closed")?;
        serde_json::to_writer(&mut *input, request)?;
        input.write_all(b"\n")?;
        input.flush()?;
        let mut line = String::new();
        if self.output.read_line(&mut line)? == 0 {
            self.input.take();
            return Err(format!("process closed before reply: {}", self.child.wait()?).into());
        }
        Ok(serde_json::from_str(&line)?)
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.wait();
    }
}
struct Client {
    executable: Executable,
    cold: bool,
    running: Option<Running>,
}
impl Client {
    fn request(&mut self, case: &Case) -> Observation<Value> {
        let result = (|| -> Result<Value> {
            if self.cold {
                let mut child = Command::new(&self.executable.path)
                    .args(&self.executable.args)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .spawn()?;
                let mut input = BufWriter::new(child.stdin.take().ok_or("missing cold stdin")?);
                serde_json::to_writer(&mut input, &case.request)?;
                input.write_all(b"\n")?;
                input.flush()?;
                drop(input);
                let result = child.wait_with_output()?;
                if !result.status.success() {
                    return Err(format!("process exited {}", result.status).into());
                }
                return Ok(serde_json::from_slice(&result.stdout)?);
            }
            if self.running.is_none() {
                self.running = Some(Running::start(&self.executable)?);
            }
            self.running
                .as_mut()
                .ok_or("missing reused process")?
                .request(&case.request)
        })();
        let value = match result {
            Ok(value) => value,
            Err(error) => {
                return Observation::Error {
                    message: error.to_string(),
                };
            }
        };
        for required in &case.requirements {
            if value.pointer(&required.pointer) != Some(&required.value) {
                let reason = format!(
                    "{} expected {}, received {}",
                    required.pointer, required.value, value
                );
                return if matches!(
                    value.pointer(&required.pointer).and_then(Value::as_str),
                    Some("unknown" | "bounded")
                ) {
                    Observation::Unknown { reason }
                } else {
                    Observation::Invalid { reason }
                };
            }
        }
        Observation::value(value)
    }
}
struct Harness {
    group: OracleGroup<'static, Case, Value>,
    clients: Vec<(String, Rc<RefCell<Client>>)>,
}
impl Harness {
    fn finish(&mut self) -> Result<()> {
        for (id, client) in &self.clients {
            if let Some(mut running) = client.borrow_mut().running.take() {
                running.finish().map_err(|error| format!("{id}: {error}"))?;
            }
        }
        Ok(())
    }
}
fn group(input: &Input) -> Result<Harness> {
    if input.version != 1
        || !["cold", "reused"].contains(&input.mode.as_str())
        || input.cases.iter().any(|case| case.requirements.is_empty())
    {
        return Err("invalid runner version/mode or missing result requirements".into());
    }
    let mut group = OracleGroup::new(&input.name);
    for case in &input.cases {
        group.case(&case.id, case.clone());
    }
    let mut clients = Vec::new();
    for (id, executable) in [
        ("oracle", &input.baseline),
        ("baseline", &input.baseline),
        ("candidate", &input.candidate),
    ] {
        let client = Rc::new(RefCell::new(Client {
            executable: executable.clone(),
            cold: input.mode == "cold",
            running: None,
        }));
        clients.push((id.to_owned(), Rc::clone(&client)));
        if id == "oracle" {
            group.reference(id, move |case| client.borrow_mut().request(case));
        } else {
            group.test_function(id, move |case| client.borrow_mut().request(case));
        }
    }
    Ok(Harness { group, clients })
}
fn save(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    Ok(())
}
fn fixture(kind: &str) -> Result<()> {
    let mut count = 0;
    for line in io::stdin().lock().lines() {
        let line = line?;
        if kind == "echo" {
            println!("{line}");
            io::stdout().flush()?;
            continue;
        }
        count += 1;
        if kind == "exit" {
            std::process::exit(7);
        }
        if kind == "bad-json" {
            println!("not json");
        } else {
            println!(
                "{}",
                json!({"kind":"fixture","status":if kind == "unknown" {"unknown"} else {"complete"},"value":if kind == "corrupt" || kind == "drift" && count > 2 { 2 } else { 1 }})
            );
        }
        io::stdout().flush()?;
    }
    if kind == "exit-after" {
        std::process::exit(7);
    }
    Ok(())
}
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|arg| arg == "--fixture") {
        return fixture(args.get(2).ok_or("fixture kind required")?);
    }
    let path = Path::new(args.get(1).ok_or("manifest required")?);
    let output = Path::new(args.get(2).ok_or("new output directory required")?);
    let input: Input = serde_json::from_slice(&fs::read(path)?)?;
    let mut harness = group(&input)?;
    fs::create_dir(output)?;
    if let Some(saved) = args.get(3) {
        let report: CheckReport<Case, Value> = serde_json::from_slice(&fs::read(saved)?)?;
        let actual = harness.group.replay(&report)?;
        if let Err(error) = harness.finish() {
            save(
                &output.join("error.json"),
                &json!({"status":"error","message":error.to_string()}),
            )?;
            return Err(error);
        }
        save(&output.join("replay.json"), &actual)?;
        actual.assert_matched();
    } else {
        let mut report = harness.group.bench(input.config)?;
        if let Err(error) = harness.finish() {
            report.status = Status::Error;
            report.failure = Some(BenchFailure {
                phase: "process-exit".into(),
                case: input
                    .cases
                    .last()
                    .map(|case| case.id.clone())
                    .unwrap_or_default(),
                implementation: "process lifecycle".into(),
                sample: None,
                iteration: 0,
                status: Status::Error,
                actual: Observation::Error {
                    message: error.to_string(),
                },
            });
            for case in &mut report.cases {
                for measurement in &mut case.measurements {
                    measurement.speedup = None;
                }
            }
        }
        save(&output.join("check.json"), &report.preflight)?;
        save(&output.join("bench.json"), &report)?;
        report.assert_matched();
    }
    Ok(())
}
