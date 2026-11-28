//! Conformance of an implementation to a model (plan 0102). The model's
//! reference semantics is the oracle: for every configuration, the
//! implementation's initial state must be an initial state of the model,
//! and along every reachable state each action instance must be accepted
//! exactly when its guards hold and lead to the model's next state. The
//! implementation is reached through a `Driver` speaking the JSON form of
//! `idealis_model::bindings`; each answer is first checked against the model's
//! JSON Schema.
mod boundaries;
mod branch_map;
pub mod external;
pub mod itf;
mod temporal;
pub(crate) use temporal::rprob_temporal_answer;
mod validation;
pub use boundaries::{
    boundary_cases, boundary_go, boundary_typescript, data_boundary_cases, data_boundary_typescript,
};
pub use branch_map::{BranchMap, generate as generate_branch_map};
pub use validation::{Session, ValidationError, Validator};

use idealis_model::reference::{Data, Machine, Next, Snapshot};
use serde::{Deserialize, Serialize};
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
        Data::Sequence(items) => {
            serde_json::json!({"$sequence": items.iter().map(element).collect::<Vec<_>>()})
        }
        Data::Variant(tag, value) => match value {
            Some(value) => serde_json::json!({"tag": tag, "value": element(value)}),
            None => serde_json::json!({"tag": tag}),
        },
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

fn held(mult: idealis_model::Mult, elements: Vec<Value>) -> Value {
    match mult {
        idealis_model::Mult::Set | idealis_model::Mult::Some => sorted_set(elements),
        // A broken multiplicity shows as an array, which the schema rejects
        // for an implementation but the model may reach.
        idealis_model::Mult::Lone | idealis_model::Mult::One if elements.len() > 1 => {
            sorted_set(elements)
        }
        idealis_model::Mult::Lone | idealis_model::Mult::One => {
            elements.into_iter().next().unwrap_or(Value::Null)
        }
    }
}

/// Relations' tuples in the JSON form of the bindings.
fn relations(model: &idealis_model::Model, data: Vec<(String, Vec<Vec<Data>>)>) -> Value {
    Value::Object(
        data.into_iter()
            .map(|(name, tuples)| {
                let value = relation_value(model, &name, &tuples);
                (name, value)
            })
            .collect(),
    )
}

fn relation_value(model: &idealis_model::Model, name: &str, tuples: &[Vec<Data>]) -> Value {
    let relation = model.relation(name).expect("a relation of the model");
    match &relation.shape {
        idealis_model::Shape::Unary { mult, .. } => {
            held(*mult, tuples.iter().map(|t| element(&t[0])).collect())
        }
        idealis_model::Shape::Binary { from, mult, .. } => {
            let mut map = Map::new();
            for atom in idealis_model::bindings::keys(model, from) {
                let elements = tuples
                    .iter()
                    .filter(|t| key(&t[0]) == atom)
                    .map(|t| element(&t[1]))
                    .collect();
                map.insert(atom, held(*mult, elements));
            }
            Value::Object(map)
        }
    }
}

fn sorted_set(mut items: Vec<Value>) -> Value {
    items.sort_by_key(Value::to_string);
    Value::Array(items)
}

/// Normalize a JSON snapshot supplied by a caller for comparison.
fn canonical(value: &Value) -> Value {
    match value {
        Value::Array(items) => sorted_set(items.iter().map(canonical).collect()),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(name, value)| {
                    let value = match (name.as_str(), value) {
                        ("$sequence", Value::Array(items)) => {
                            Value::Array(items.iter().map(canonical).collect())
                        }
                        _ => canonical(value),
                    };
                    (name.clone(), value)
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Encode named arguments in the JSON form of the bindings.
pub fn arguments(args: &[(String, Data)]) -> impl Iterator<Item = (String, Value)> + '_ {
    args.iter()
        .map(|(param, value)| (param.clone(), element(value)))
}

fn command(action: &str, args: &[(String, Data)]) -> Value {
    let mut object = Map::new();
    object.insert("kind".into(), Value::String(action.to_owned()));
    object.extend(args.iter().map(|(param, value)| {
        (
            idealis_model::bindings::command_argument_key(param).to_owned(),
            element(value),
        )
    }));
    Value::Object(object)
}

/// Encode a finite reference witness or an explicit temporal lasso for replay.
pub fn witness(
    model: &idealis_model::Model,
    question: &str,
    budget: usize,
) -> Result<Value, String> {
    if model.temporal.iter().any(|check| check.name == question) {
        return temporal::witness(model, question, budget);
    }
    witness_with_budget(
        model,
        question,
        idealis_model::reference::SearchBudget::for_states(budget),
    )
}

/// Encode a witness from finite reference search with separate limits.
pub fn witness_with_budget(
    model: &idealis_model::Model,
    question: &str,
    budget: idealis_model::reference::SearchBudget,
) -> Result<Value, String> {
    let result = idealis_model::reference::witness_with_budget(model, question, budget)?;
    Ok(question_witness(model, question, result))
}

pub fn rprob_witness(
    model: &idealis_model::Model,
    question: &str,
    budget: usize,
) -> Result<Value, String> {
    if model.temporal.iter().any(|check| check.name == question) {
        return temporal::rprob_witness(model, question, budget);
    }
    let result = rprob::witness(model, question, budget).map_err(|error| error.to_string())?;
    Ok(question_witness(model, question, result))
}

pub fn rprob_witness_with_budget(
    model: &idealis_model::Model,
    question: &str,
    budget: idealis_model::reference::SearchBudget,
) -> Result<Value, String> {
    let result =
        rprob::witness_with_budget(model, question, budget).map_err(|error| error.to_string())?;
    Ok(question_witness(model, question, result))
}

fn question_witness(
    model: &idealis_model::Model,
    question: &str,
    result: idealis_model::reference::WitnessResult,
) -> Value {
    let declarations = result.declarations;
    let mut response = witness_payload(model, result);
    response["question"] = Value::String(question.to_owned());
    add_declaration_diagnostic(&mut response, declarations);
    response
}

/// Encode the shortest matching finite transition path for replay.
pub fn transition_witness(
    model: &idealis_model::Model,
    action: &str,
    predicate: &str,
    budget: usize,
) -> Result<Value, String> {
    transition_witness_with_budget(
        model,
        action,
        predicate,
        idealis_model::reference::SearchBudget::for_states(budget),
    )
}

pub fn transition_witness_with_budget(
    model: &idealis_model::Model,
    action: &str,
    predicate: &str,
    budget: idealis_model::reference::SearchBudget,
) -> Result<Value, String> {
    let result =
        idealis_model::reference::transition_witness_with_budget(model, action, predicate, budget)?;
    let mut response = witness_payload(model, result);
    response["action"] = Value::String(action.to_owned());
    Ok(response)
}

/// Encode a path to a state without any enabled operation.
pub fn deadlock_witness(
    model: &idealis_model::Model,
    unless: Option<&str>,
    budget: idealis_model::reference::SearchBudget,
) -> Result<Value, String> {
    deadlock_witness_with_mode(
        model,
        unless,
        budget,
        idealis_model::reference::DeadlockMode::Guards,
    )
}

pub fn deadlock_witness_with_mode(
    model: &idealis_model::Model,
    unless: Option<&str>,
    budget: idealis_model::reference::SearchBudget,
    mode: idealis_model::reference::DeadlockMode,
) -> Result<Value, String> {
    let result = idealis_model::reference::deadlock_witness_with_mode(model, unless, budget, mode)?;
    let declarations = result.declarations;
    let mut response = witness_payload(model, result);
    add_declaration_diagnostic(&mut response, declarations);
    Ok(response)
}

/// Examine source-level update groups without repeating their conditions in a query.
pub fn update_branch_report(
    path: &str,
    action: &str,
    budget: idealis_model::reference::SearchBudget,
) -> Result<Value, String> {
    update_branch_report_with_map(path, action, budget, None)
}

pub fn update_branch_report_with_map(
    path: &str,
    action: &str,
    budget: idealis_model::reference::SearchBudget,
    map: Option<&str>,
) -> Result<Value, String> {
    use idealis_model::reference::Outcome;
    let (model, locations) = branch_map::load(path, map)?;
    let locations = locations
        .ok_or("branches requires model source, a project manifest, or an IR with --branch-map")?;
    let branches = locations.groups.get(action).map_or(&[][..], Vec::as_slice);
    let result =
        idealis_model::reference::update_branch_witnesses(&model, action, branches, budget)?;
    let absent = match (result.complete, result.scope) {
        (true, Outcome::Empty) => crate::Verdict::Empty,
        (true, _) => crate::Verdict::Unreachable,
        (false, _) => crate::Verdict::Unknown,
    };
    let (branches, _) = encode_branches(
        &model,
        branches,
        &locations.positions,
        result.branches,
        &absent,
    );
    let mut response = serde_json::json!({
        "action": action,
        "complete": result.complete,
        "branches": branches,
    });
    let diagnostics = crate::reference_diagnostics(result.scope, result.stop.as_ref());
    if !diagnostics.is_empty() {
        response["diagnostics"] = serde_json::json!(diagnostics);
    }
    add_declaration_diagnostic(&mut response, result.declarations);
    Ok(response)
}

#[derive(Default)]
struct BranchFindings {
    unreachable: bool,
    expectation_mismatch: bool,
}

fn encode_branches(
    model: &idealis_model::Model,
    groups: &[idealis_model::UpdateBranch],
    positions: &std::collections::BTreeMap<usize, String>,
    paths: Vec<[Option<idealis_model::reference::Witness>; 2]>,
    absent: &crate::Verdict,
) -> (Vec<Value>, BranchFindings) {
    use crate::Verdict;
    let mut findings = BranchFindings::default();
    let mut arm = |witness: Option<_>, reason: Option<&str>| {
        let (verdict, trace) = match witness {
            Some(trace) => (&Verdict::Reachable, Some(encode_witness(model, trace))),
            None => (absent, None),
        };
        let mut row = serde_json::json!({"verdict": verdict, "trace": trace});
        if let Some(reason) = reason {
            let expectation = match verdict {
                Verdict::Unreachable => "matched",
                Verdict::Reachable => {
                    findings.expectation_mismatch = true;
                    "mismatch"
                }
                _ => "unknown",
            };
            row["expected"] = serde_json::json!(Verdict::Unreachable);
            row["reason"] = serde_json::json!(reason);
            row["expectation"] = serde_json::json!(expectation);
        } else {
            findings.unreachable |= *verdict == Verdict::Unreachable;
        }
        row
    };
    let rows = groups.iter().zip(paths).map(|(group, [yes, no])| {
        let [yes_reason, no_reason] = group.unreachable_reasons();
        let mut row = serde_json::json!({ "source": positions[&group.offset], "then": arm(yes, yes_reason.as_deref()), "else": arm(no, no_reason.as_deref()) });
        if group.is_value_conditional() {
            row["kind"] = "value".into();
        }
        if let Some(binding) = group.binding_name() {
            row["binding"] = binding.into();
        }
        row
    }).collect();
    (rows, findings)
}

/// Operation and source update-arm reachability with one shared search budget.
pub fn coverage_report(
    path: &str,
    budget: idealis_model::reference::SearchBudget,
) -> Result<Value, String> {
    coverage_report_with_map(path, budget, None)
}

pub fn coverage_report_with_map(
    path: &str,
    budget: idealis_model::reference::SearchBudget,
    map: Option<&str>,
) -> Result<Value, String> {
    coverage_report_with_instances(path, budget, map, false)
}

pub fn coverage_report_with_instances(
    path: &str,
    budget: idealis_model::reference::SearchBudget,
    map: Option<&str>,
    include_instances: bool,
) -> Result<Value, String> {
    use idealis_model::reference::{
        Outcome, analyze_actions_with_budget, update_coverage, update_coverage_with_instances,
    };
    const MAX_INSTANCES: usize = 4096;
    let (model, locations) = branch_map::load(path, map)?;
    let (report, paths, mut instances) = if include_instances {
        let empty = std::collections::BTreeMap::new();
        let groups = locations
            .as_ref()
            .map_or(&empty, |locations| &locations.groups);
        let audit = update_coverage_with_instances(&model, groups, budget, MAX_INSTANCES)?;
        (audit.report, audit.paths, Some(audit.instances.into_iter()))
    } else if let Some(ref locations) = locations {
        let (report, paths) = update_coverage(&model, &locations.groups, budget)?;
        (report, paths, None)
    } else {
        (
            analyze_actions_with_budget(&model, budget),
            Vec::new(),
            None,
        )
    };
    let excluded_values = if locations
        .as_ref()
        .is_some_and(|locations| locations.includes_assigned_values)
    {
        "embedded_value_conditionals"
    } else {
        "value_conditionals"
    };
    let mut source = locations.map(|locations| (locations, paths.into_iter()));
    let (coverage, absent) = match (report.scope, report.declarations) {
        (_, Outcome::Violated { .. }) => ("unknown", crate::Verdict::Unknown),
        (Outcome::Holds, _) => ("complete", crate::Verdict::Unreachable),
        (Outcome::Empty, _) => ("empty", crate::Verdict::Empty),
        _ => ("unknown", crate::Verdict::Unknown),
    };
    let branch_coverage = if source.is_some() {
        coverage
    } else {
        "unsupported"
    };
    let mut dead = false;
    let mut expectation_mismatch = false;
    let mut catalogue_complete = true;
    let actions = report
        .enabled
        .into_iter()
        .map(|(name, enabled)| {
            let status = if enabled == Some(true) {
                &crate::Verdict::Reachable
            } else {
                &absent
            };
            dead |= *status == crate::Verdict::Unreachable;
            let branches = source.as_mut().map(|(locations, paths)| {
                let paths = paths.next().expect("one path list per action");
                let (rows, findings) = encode_branches(
                    &model,
                    locations.groups.get(&name).map_or(&[][..], Vec::as_slice),
                    &locations.positions,
                    paths,
                    &absent,
                );
                dead |= findings.unreachable;
                expectation_mismatch |= findings.expectation_mismatch;
                rows
            });
            let mut row = serde_json::json!({ "name": name, "status": status, "branches": branches });
            if let Some(instances) = &mut instances {
                match instances.next().expect("one instance catalogue per action") {
                    Ok(calls) => {
                        row["instances"] = serde_json::json!(calls.into_iter().map(|instance| {
                            let status = if instance.enabled == Some(true) {
                                &crate::Verdict::Reachable
                            } else {
                                &absent
                            };
                            dead |= *status == crate::Verdict::Unreachable;
                            serde_json::json!({
                                "action": instance.call.action,
                                "args": arguments(&instance.call.args).collect::<serde_json::Map<_, _>>(),
                                "status": status,
                            })
                        }).collect::<Vec<_>>());
                    }
                    Err(reason) => {
                        catalogue_complete = false;
                        row["instances"] = Value::Null;
                        row["instanceDiagnostic"] = reason.into();
                    }
                }
            }
            row
        })
        .collect::<Vec<_>>();
    let status = match (
        coverage,
        branch_coverage,
        dead,
        expectation_mismatch,
        catalogue_complete,
    ) {
        (_, _, _, true, _) => "expectation_mismatch",
        ("complete", "complete", false, false, true) => "clear",
        (_, _, true, false, _) => "unreachable",
        _ => "inconclusive",
    };
    let mut response = serde_json::json!({
        "status": status,
        "coverage": coverage,
        "branchCoverage": branch_coverage,
        "actions": actions,
        "excluded": ["argument_instances", excluded_values],
    });
    if include_instances {
        response["instanceCoverage"] = if catalogue_complete {
            coverage
        } else {
            "partial"
        }
        .into();
        response["excluded"] = serde_json::json!([excluded_values]);
    }
    let diagnostics = crate::reference_diagnostics(report.scope, report.stop.as_ref());
    if !diagnostics.is_empty() {
        response["diagnostics"] = serde_json::json!(diagnostics);
    }
    add_declaration_diagnostic(&mut response, report.declarations);
    Ok(response)
}

fn add_declaration_diagnostic(
    response: &mut Value,
    declarations: idealis_model::reference::Outcome,
) {
    if let idealis_model::reference::Outcome::Violated { depth } = declarations {
        let diagnostic = serde_json::json!(crate::Diagnostic::DeclarationViolation { depth });
        if let Some(diagnostics) = response["diagnostics"].as_array_mut() {
            diagnostics.push(diagnostic);
        } else {
            response["diagnostics"] = serde_json::json!([diagnostic]);
        }
    }
}

fn witness_payload(
    model: &idealis_model::Model,
    result: idealis_model::reference::WitnessResult,
) -> Value {
    use idealis_model::reference::Outcome;
    let verdict = match result.outcome {
        Outcome::Empty => crate::Verdict::Empty,
        Outcome::Holds => crate::Verdict::Holds,
        Outcome::Violated { .. } => crate::Verdict::Violated,
        Outcome::Reachable { .. } => crate::Verdict::Reachable,
        Outcome::Unreachable => crate::Verdict::Unreachable,
        Outcome::Unknown => crate::Verdict::Unknown,
    };
    let trace = result.witness.map(|trace| encode_witness(model, trace));
    let mut response = serde_json::json!({
        "verdict": verdict, "complete": result.complete, "trace": trace
    });
    let diagnostics = crate::reference_diagnostics(result.scope, result.stop.as_ref());
    if !diagnostics.is_empty() {
        response["diagnostics"] = serde_json::json!(diagnostics);
    }
    response
}

fn encode_witness(model: &idealis_model::Model, trace: idealis_model::reference::Witness) -> Value {
    let mut encoded = serde_json::json!({
        "config": relations(model, trace.config),
        "initial": relations(model, trace.initial),
        "commands": trace.commands.iter().map(|step| command(&step.action, &step.args)).collect::<Vec<_>>(),
        "finalState": relations(model, trace.final_state),
    });
    if let Some(events) = trace.events {
        encoded["events"] = Value::Array(events.into_iter().map(|event| serde_json::json!({
            "step": event.step, "event": event.event, "observation": observation_json(model, event.observation),
        })).collect());
    }
    encoded
}

fn observation_json(model: &idealis_model::Model, fields: Vec<(String, Vec<Vec<Data>>)>) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(name, tuples)| {
                let value = if model.relation(&name).is_some() {
                    relation_value(model, &name, &tuples)
                } else {
                    sorted_set(
                        tuples
                            .iter()
                            .map(|tuple| Value::Array(tuple.iter().map(element).collect()))
                            .collect(),
                    )
                };
                (name, value)
            })
            .collect(),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationQuery {
    pub at: String,
    pub observe: Vec<String>,
    pub distinguish: Vec<String>,
    #[serde(default)]
    pub comparison: ObservationComparison,
    #[serde(default)]
    pub silent: Vec<String>,
    #[serde(default)]
    pub emit: Vec<ObservationEmission>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationEmission {
    pub action: String,
    pub event: String,
    #[serde(default)]
    pub when: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObservationComparison {
    #[default]
    Snapshot,
    History,
}

pub fn observation_pair(
    model: &idealis_model::Model,
    query: &ObservationQuery,
    budget: usize,
) -> Result<Value, String> {
    use idealis_model::reference::Outcome;
    let result = match query.comparison {
        ObservationComparison::Snapshot if !query.silent.is_empty() => {
            return Err("silent operations require history comparison".into());
        }
        ObservationComparison::Snapshot if !query.emit.is_empty() => {
            return Err("event rules require history comparison".into());
        }
        ObservationComparison::Snapshot => idealis_model::reference::observation_pair(
            model,
            &query.at,
            &query.observe,
            &query.distinguish,
            budget,
        ),
        ObservationComparison::History => idealis_model::reference::observation_history_pair(
            model,
            &query.at,
            &query.observe,
            &query.distinguish,
            &query.silent,
            &query
                .emit
                .iter()
                .map(|rule| idealis_model::reference::ObservationEvent {
                    action: &rule.action,
                    event: &rule.event,
                    when: rule.when.as_deref(),
                })
                .collect::<Vec<_>>(),
            budget,
        ),
    }?;
    let verdict = match result.outcome {
        Outcome::Reachable { .. } => crate::Verdict::Reachable,
        Outcome::Unreachable => crate::Verdict::Unreachable,
        Outcome::Empty => crate::Verdict::Empty,
        Outcome::Unknown => crate::Verdict::Unknown,
        _ => unreachable!("observation-pair outcome"),
    };
    let pair = result
        .pair
        .map(|pair| pair.map(|trace| encode_witness(model, trace)));
    Ok(serde_json::json!({"verdict": verdict, "complete": result.complete, "pair": pair}))
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

pub fn conform(model: &idealis_model::Model, driver: &mut dyn Driver, budget: Budget) -> Report {
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
    let schema: Value = serde_json::from_str(&idealis_model::bindings::json_schema(model))
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
            let normalized = canonical(&actual);
            let start: Option<Snapshot> = initial
                .into_iter()
                .find(|state| relations(model, machine.state_data(state)) == normalized);
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
                    if expected != canonical(&actual) {
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
pub fn reachable_states(model: &idealis_model::Model) -> Result<Vec<Value>, String> {
    let mut states = BTreeSet::new();
    visit_reachable(model, |machine, _, state| {
        states.insert(relations(model, machine.state_data(state)).to_string());
    })?;
    Ok(states
        .into_iter()
        .map(|text| serde_json::from_str(&text).expect("JSON"))
        .collect())
}

/// Configuration and state pairs for a snapshot oracle. Check the model
/// and exploration budget with the reference evaluator before enumerating.
pub fn reachable_observations(model: &idealis_model::Model) -> Result<Vec<Value>, String> {
    let mut observations = Vec::new();
    visit_reachable(model, |machine, configuration, state| {
        observations.push(serde_json::json!({
            "config": relations(model, machine.configuration_data(configuration)),
            "state": relations(model, machine.state_data(state)),
        }));
    })?;
    Ok(observations)
}

fn visit_reachable(
    model: &idealis_model::Model,
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
