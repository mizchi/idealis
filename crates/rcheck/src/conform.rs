//! Conformance of an implementation to a model (plan 0102). The model's
//! reference semantics is the oracle: for every configuration, the
//! implementation's initial state must be an initial state of the model,
//! and along every reachable state each action instance must be accepted
//! exactly when its guards hold and lead to the model's next state. The
//! implementation is reached through a `Driver` speaking the JSON form of
//! `rmodel::bindings`; each answer is first checked against the model's
//! JSON Schema.
use rmodel::reference::{Data, Machine, Next, Snapshot};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeSet, VecDeque};

/// An implementation, in the JSON form of the bindings.
pub trait Driver {
    /// Starts a fresh instance with a configuration; returns its state.
    fn reset(&mut self, config: &Value) -> Result<Value, String>;
    /// Applies a command; returns whether it was accepted, and the state.
    fn apply(&mut self, command: &Value) -> Result<(bool, Value), String>;
}

#[derive(Clone, Copy, Debug)]
pub struct Budget {
    /// Configurations to check; more make the result `bounded`.
    pub configurations: usize,
    /// Driver calls in all.
    pub calls: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            configurations: usize::MAX,
            calls: 10_000_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Every configuration and every reachable state agree.
    Matched,
    /// Agreement on the configurations checked, but not all of them.
    Bounded,
    /// The implementation answers differently from the model.
    Mismatch,
    /// The implementation's answer does not fit the model's JSON form.
    Invalid,
    /// No configuration has an initial state: nothing was checked.
    Empty,
    /// The call budget ran out.
    Unknown,
    /// The model cannot be driven, or the driver failed.
    Error,
}

#[derive(Clone, Debug, Serialize)]
pub struct Failure {
    pub config: Value,
    /// Accepted commands from the initial state, then the failing one.
    pub path: Vec<Value>,
    pub expected: Value,
    pub actual: Value,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub status: Status,
    pub configurations: usize,
    pub states: usize,
    pub transitions: usize,
    pub calls: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A value of the model in the JSON form.
fn element(data: &Data) -> Value {
    match data {
        // Bool's atoms are the only ones named true and false (keywords).
        Data::Atom(name) if name == "true" => Value::Bool(true),
        Data::Atom(name) if name == "false" => Value::Bool(false),
        Data::Atom(name) => Value::String(name.clone()),
        Data::Int(value) => Value::from(*value),
        Data::Record(_, fields) => Value::Object(
            fields
                .iter()
                .map(|(field, value)| (field.clone(), element(value)))
                .collect(),
        ),
    }
}

fn key(data: &Data) -> String {
    match data {
        Data::Atom(name) => name.clone(),
        other => unreachable!("values are not keys: {other:?}"),
    }
}

fn held(mult: rmodel::Mult, elements: Vec<Value>) -> Value {
    match mult {
        rmodel::Mult::Set | rmodel::Mult::Some => Value::Array(elements),
        // A broken multiplicity shows as an array, which the schema rejects
        // for an implementation but the model may reach.
        rmodel::Mult::Lone | rmodel::Mult::One if elements.len() > 1 => Value::Array(elements),
        rmodel::Mult::Lone | rmodel::Mult::One => {
            elements.into_iter().next().unwrap_or(Value::Null)
        }
    }
}

/// Relations' tuples in the JSON form of the bindings.
fn relations(model: &rmodel::Model, data: Vec<(String, Vec<Vec<Data>>)>) -> Value {
    let mut object = Map::new();
    for (name, tuples) in data {
        let relation = model.relation(&name).expect("a relation of the model");
        let value = match &relation.shape {
            rmodel::Shape::Unary { mult, .. } => {
                held(*mult, tuples.iter().map(|t| element(&t[0])).collect())
            }
            rmodel::Shape::Binary { from, mult, .. } => {
                let mut map = Map::new();
                for atom in rmodel::bindings::keys(model, from) {
                    let elements = tuples
                        .iter()
                        .filter(|t| key(&t[0]) == atom)
                        .map(|t| element(&t[1]))
                        .collect();
                    map.insert(atom, held(*mult, elements));
                }
                Value::Object(map)
            }
        };
        object.insert(name, value);
    }
    Value::Object(object)
}

/// Arrays are sets in the JSON form: sorted for comparison.
fn canonical(value: &Value) -> Value {
    match value {
        Value::Array(items) => {
            let mut items: Vec<Value> = items.iter().map(canonical).collect();
            items.sort_by_key(|item| item.to_string());
            Value::Array(items)
        }
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(name, value)| (name.clone(), canonical(value)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn command(action: &str, args: &[(String, Data)]) -> Value {
    let mut object = Map::new();
    object.insert("kind".into(), Value::String(action.to_owned()));
    for (param, value) in args {
        object.insert(param.clone(), element(value));
    }
    Value::Object(object)
}

struct Run<'d> {
    driver: &'d mut dyn Driver,
    schema: Value,
    calls: usize,
    budget: usize,
}

enum Stop {
    Failure(Status, Box<Failure>),
    Budget,
    Driver(String),
}

fn stop(status: Status, failure: Failure) -> Stop {
    Stop::Failure(status, Box::new(failure))
}

impl Run<'_> {
    fn spend(&mut self) -> Result<(), Stop> {
        self.calls += 1;
        if self.calls > self.budget {
            return Err(Stop::Budget);
        }
        Ok(())
    }
    fn valid(&self, state: &Value, config: &Value, path: &[Value]) -> Result<(), Stop> {
        crate::schema::validate(&self.schema, "State", state).map_err(|reason| {
            stop(
                Status::Invalid,
                Failure {
                    config: config.clone(),
                    path: path.to_vec(),
                    expected: Value::String("a State of the JSON Schema".into()),
                    actual: state.clone(),
                    reason,
                },
            )
        })
    }
    /// A fresh instance moved along `path`, each command accepted as before.
    fn replay(&mut self, config: &Value, path: &[Value]) -> Result<(), Stop> {
        self.spend()?;
        self.driver.reset(config).map_err(Stop::Driver)?;
        for (index, step) in path.iter().enumerate() {
            self.spend()?;
            let (accepted, _) = self.driver.apply(step).map_err(Stop::Driver)?;
            if !accepted {
                return Err(stop(
                    Status::Mismatch,
                    Failure {
                        config: config.clone(),
                        path: path[..=index].to_vec(),
                        expected: Value::Bool(true),
                        actual: Value::Bool(false),
                        reason: "replaying an accepted command was refused: the implementation is not deterministic".into(),
                    },
                ));
            }
        }
        Ok(())
    }
}

pub fn conform(model: &rmodel::Model, driver: &mut dyn Driver, budget: Budget) -> Report {
    let mut report = Report {
        status: Status::Matched,
        configurations: 0,
        states: 0,
        transitions: 0,
        calls: 0,
        failure: None,
        detail: None,
    };
    let machine = match Machine::new(model) {
        Ok(machine) => machine,
        Err(reason) => {
            report.status = Status::Error;
            report.detail = Some(reason);
            return report;
        }
    };
    let schema: Value = serde_json::from_str(&rmodel::bindings::json_schema(model))
        .expect("the generated schema is JSON");
    let mut run = Run {
        driver,
        schema,
        calls: 0,
        budget: budget.calls,
    };
    let mut initial_seen = false;
    let mut complete = true;
    let outcome = (|| -> Result<(), Stop> {
        for configuration in machine.configurations() {
            let initial = machine.initial(&configuration);
            if initial.is_empty() {
                continue;
            }
            if report.configurations == budget.configurations {
                complete = false;
                break;
            }
            report.configurations += 1;
            initial_seen = true;
            let config = relations(model, machine.configuration_data(&configuration));
            run.spend()?;
            let actual = run.driver.reset(&config).map_err(Stop::Driver)?;
            run.valid(&actual, &config, &[])?;
            let start: Option<Snapshot> = initial.into_iter().find(|state| {
                canonical(&relations(model, machine.state_data(state))) == canonical(&actual)
            });
            let Some(start) = start else {
                return Err(stop(
                    Status::Mismatch,
                    Failure {
                        config,
                        path: Vec::new(),
                        expected: Value::String("an initial state of the model".into()),
                        actual,
                        reason: "the initial state is not one the model allows".into(),
                    },
                ));
            };
            let mut seen = BTreeSet::from([start.clone()]);
            let mut queue = VecDeque::from([(start, Vec::<Value>::new())]);
            while let Some((state, path)) = queue.pop_front() {
                report.states += 1;
                for step in machine.steps(&configuration, &state) {
                    let cmd = command(&step.action, &step.args);
                    run.replay(&config, &path)?;
                    run.spend()?;
                    let (accepted, actual) = run.driver.apply(&cmd).map_err(Stop::Driver)?;
                    report.transitions += 1;
                    let mut trail = path.clone();
                    trail.push(cmd.clone());
                    let fail = |expected: Value, actual: Value, reason: &str| {
                        stop(
                            Status::Mismatch,
                            Failure {
                                config: config.clone(),
                                path: trail.clone(),
                                expected,
                                actual,
                                reason: reason.to_owned(),
                            },
                        )
                    };
                    let Some(next) = step.next else {
                        if accepted {
                            return Err(fail(
                                Value::Bool(false),
                                Value::Bool(true),
                                "accepted a command whose guards do not hold",
                            ));
                        }
                        continue;
                    };
                    if !accepted {
                        return Err(fail(
                            Value::Bool(true),
                            Value::Bool(false),
                            "refused a command whose guards hold",
                        ));
                    }
                    let snapshot = match &next {
                        Next::State(s) | Next::Broken(s) | Next::Outside(s) => s,
                    };
                    let expected = relations(model, machine.state_data(snapshot));
                    if matches!(next, Next::State(_)) {
                        run.valid(&actual, &config, &trail)?;
                    }
                    if canonical(&expected) != canonical(&actual) {
                        return Err(fail(expected, actual, "a different next state"));
                    }
                    // Broken and outside states end the exploration there,
                    // as in the reference semantics.
                    if let Next::State(s) = next
                        && seen.insert(s.clone())
                    {
                        queue.push_back((s, trail));
                    }
                }
            }
        }
        Ok(())
    })();
    report.calls = run.calls;
    match outcome {
        Ok(()) if !initial_seen => report.status = Status::Empty,
        Ok(()) if !complete => report.status = Status::Bounded,
        Ok(()) => {}
        Err(Stop::Failure(status, failure)) => {
            report.status = status;
            report.failure = Some(*failure);
        }
        Err(Stop::Budget) => report.status = Status::Unknown,
        Err(Stop::Driver(reason)) => {
            report.status = Status::Error;
            report.detail = Some(reason);
        }
    }
    report
}

/// A driver in another process speaking JSON Lines: `{"op":"reset",
/// "config":..}` answered by `{"state":..}`, and `{"op":"apply",
/// "command":..}` by `{"accepted":..,"state":..}`; `{"error":..}` fails.
pub struct Process {
    child: std::process::Child,
    input: std::process::ChildStdin,
    output: std::io::BufReader<std::process::ChildStdout>,
}

impl Process {
    pub fn spawn(program: &str, args: &[&str]) -> Result<Self, String> {
        use std::process::{Command, Stdio};
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| format!("{program}: {error}"))?;
        let input = child.stdin.take().expect("piped");
        let output = std::io::BufReader::new(child.stdout.take().expect("piped"));
        Ok(Self {
            child,
            input,
            output,
        })
    }
    fn ask(&mut self, message: Value) -> Result<Value, String> {
        use std::io::{BufRead, Write};
        writeln!(self.input, "{message}").map_err(|error| error.to_string())?;
        let mut line = String::new();
        self.output
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;
        if line.is_empty() {
            return Err("the driver closed its output".into());
        }
        let reply: Value = serde_json::from_str(&line).map_err(|error| error.to_string())?;
        if let Some(error) = reply.get("error") {
            return Err(format!("driver: {error}"));
        }
        Ok(reply)
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Driver for Process {
    fn reset(&mut self, config: &Value) -> Result<Value, String> {
        let reply = self.ask(serde_json::json!({ "op": "reset", "config": config }))?;
        reply.get("state").cloned().ok_or("no state".into())
    }
    fn apply(&mut self, command: &Value) -> Result<(bool, Value), String> {
        let reply = self.ask(serde_json::json!({ "op": "apply", "command": command }))?;
        let accepted = reply
            .get("accepted")
            .and_then(Value::as_bool)
            .ok_or("no accepted")?;
        Ok((accepted, reply.get("state").cloned().ok_or("no state")?))
    }
}

/// Every distinct state the model reaches in any configuration, in the
/// JSON form, canonical and sorted: the states an implementation must be
/// able to show, and the schema must accept.
pub fn reachable_states(model: &rmodel::Model) -> Result<Vec<Value>, String> {
    let mut states = BTreeSet::new();
    visit_reachable(model, |machine, _, state| {
        states.insert(canonical(&relations(model, machine.state_data(state))).to_string());
    })?;
    Ok(states
        .into_iter()
        .map(|text| serde_json::from_str(&text).expect("JSON"))
        .collect())
}

/// Configuration and state pairs for a snapshot oracle. Check the model
/// and exploration budget with the reference evaluator before enumerating.
pub fn reachable_observations(model: &rmodel::Model) -> Result<Vec<Value>, String> {
    let mut observations = Vec::new();
    visit_reachable(model, |machine, configuration, state| {
        observations.push(serde_json::json!({
            "config": canonical(&relations(model, machine.configuration_data(configuration))),
            "state": canonical(&relations(model, machine.state_data(state))),
        }));
    })?;
    Ok(observations)
}

fn visit_reachable(
    model: &rmodel::Model,
    mut visit: impl FnMut(&Machine<'_>, &Snapshot, &Snapshot),
) -> Result<(), String> {
    let machine = Machine::new(model)?;
    for configuration in machine.configurations() {
        let mut seen: BTreeSet<Snapshot> = machine.initial(&configuration).into_iter().collect();
        let mut queue: VecDeque<Snapshot> = seen.iter().cloned().collect();
        while let Some(state) = queue.pop_front() {
            visit(&machine, &configuration, &state);
            for step in machine.steps(&configuration, &state) {
                if let Some(Next::State(next)) = step.next
                    && seen.insert(next.clone())
                {
                    queue.push_back(next);
                }
            }
        }
    }
    Ok(())
}
