mod finite;
pub use rtla_compiler::{CompileOptions, IrVersion};
pub use rtlc::{
    CoreError, ExpansionOptions, ExpansionStats, PreparedSafetyProgram, SafetyActionCoverage,
    SafetyBounds, SafetyCoverage, SafetyCoverageReport, SafetyExpansionCache, SafetyOptions,
    SafetyProgram, SafetyResult, Value,
};
mod history;
pub use history::{ConcurrentHistory, ConcurrentResult, ConcurrentStatus, HistoryEvent};
mod host;
pub use host::{HostEvent, HostMessage, HostTape, SimulationHost, replay_host};
mod shrink;
pub use finite::{Edge, FiniteModel, FiniteState, Initial, PreparedFiniteModel};
pub use shrink::{
    DeletionRange, ShrinkResult, ShrinkStatus, ShrinkVerdict, deletion_candidates,
    shrink_counterexample,
};
use std::{collections::BTreeMap, sync::Arc};
#[cfg(target_arch = "wasm32")]
mod wasm;
use serde::{Deserialize, Serialize};

pub fn compile_tla(source: &str, config: &str) -> Result<SafetyProgram, CoreError> {
    compile_tla_with_options(source, config, CompileOptions::default())
}

pub fn compile_tla_with_options(
    source: &str,
    config: &str,
    options: CompileOptions,
) -> Result<SafetyProgram, CoreError> {
    compile_tla_bundle(
        source,
        config,
        &BTreeMap::new(),
        options,
        rtla_compiler::SyntaxVersion::default(),
    )
}

fn compile_tla_bundle(
    source: &str,
    config: &str,
    sources: &BTreeMap<String, String>,
    options: CompileOptions,
    syntax: rtla_compiler::SyntaxVersion,
) -> Result<SafetyProgram, CoreError> {
    rtla_compiler::compile_source_bundle(source, config, sources, options, syntax)
        .map_err(|error| CoreError::protocol(&error))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub key: String,
    pub state: Value,
    pub observation: Value,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub key: String,
    pub state: Value,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub action: Value,
    pub observation: Value,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trace {
    pub format: String,
    pub version: u32,
    pub model: String,
    pub initial: Value,
    pub steps: Vec<Step>,
}
impl Trace {
    pub fn normalize(self) -> Result<Self, CoreError> {
        if self.version != 1 || self.format != "modelt.portable-trace" {
            return Err(CoreError::protocol("Unsupported trace format"));
        }
        Ok(Self {
            initial: self.initial.normalize()?,
            steps: self
                .steps
                .into_iter()
                .map(|step| {
                    Ok(Step {
                        action: step.action.normalize()?,
                        observation: step.observation.normalize()?,
                    })
                })
                .collect::<Result<_, CoreError>>()?,
            ..self
        })
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Matched,
    Mismatch,
    Invalid,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConformanceResult {
    pub status: Status,
    pub checked_steps: usize,
    pub actual: Value,
    pub expected: Vec<Value>,
    pub trace: Trace,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Effect {
    ModelInitial {
        id: u32,
    },
    DriverReset {
        id: u32,
    },
    ModelStep {
        id: u32,
        states: Vec<State>,
        action: Value,
    },
    DriverStep {
        id: u32,
        action: Value,
    },
    Done {
        result: ConformanceResult,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Response {
    Transitions { transitions: Vec<Transition> },
    Observation { observation: Value },
}
fn default_ir_version() -> u8 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SafetyCheck {
    pub bounds: SafetyBounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<SafetyOptions>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SafetyBatchResult {
    Safety { result: SafetyResult },
    Error { code: String, message: String },
}
#[allow(non_snake_case)]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Request {
    CompileTla {
        source: String,
        config: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        modules: BTreeMap<String, String>,
        #[serde(default = "default_ir_version")]
        irVersion: u8,
        #[serde(default)]
        syntaxVersion: rtla_compiler::SyntaxVersion,
    },
    CheckTla {
        source: String,
        config: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        modules: BTreeMap<String, String>,
        #[serde(default = "default_ir_version")]
        irVersion: u8,
        #[serde(default)]
        syntaxVersion: rtla_compiler::SyntaxVersion,
        bounds: SafetyBounds,
    },
    #[cfg(not(target_arch = "wasm32"))]
    CheckSafetyParallel {
        program: SafetyProgram,
        bounds: SafetyBounds,
        workers: usize,
    },
    CheckSafety {
        program: SafetyProgram,
        bounds: SafetyBounds,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<SafetyOptions>,
    },
    CheckSafetyCoverage {
        program: SafetyProgram,
        bounds: SafetyBounds,
    },
    PrepareSafety {
        program: SafetyProgram,
    },
    CheckPreparedSafety {
        handle: u32,
        bounds: SafetyBounds,
        #[serde(default)]
        options: Option<SafetyOptions>,
    },
    CheckPreparedSafetyBatch {
        handle: u32,
        jobs: Vec<SafetyCheck>,
    },
    ReleaseSafety {
        handle: u32,
    },
    #[serde(rename_all = "camelCase")]
    CheckSafetyIncremental {
        #[serde(default)]
        enabled: bool,
        program: SafetyProgram,
        bounds: SafetyBounds,
        cache: SafetyExpansionCache,
        build_digest: String,
        max_entries: u32,
    },
    #[serde(rename_all = "camelCase")]
    MergeSafetyExpansions {
        left: SafetyExpansionCache,
        right: SafetyExpansionCache,
        max_entries: u32,
    },
    LoadModel {
        model: FiniteModel,
    },
    ReleaseModel {
        handle: u32,
    },
    StartLoaded {
        handle: u32,
        actions: Vec<Value>,
    },
    CheckLoaded {
        handle: u32,
        traces: Vec<Trace>,
    },
    CheckHistory {
        handle: u32,
        history: ConcurrentHistory,
        #[serde(rename = "maxNodes")]
        max_nodes: u32,
    },
    ReplayHost {
        tape: HostTape,
    },
    StartFinite {
        model: FiniteModel,
        actions: Vec<Value>,
    },
    CheckFinite {
        model: FiniteModel,
        traces: Vec<Trace>,
    },
    ShrinkCandidates {
        length: u32,
    },
    Canonicalize {
        value: Value,
    },
    Start {
        model: String,
        actions: Vec<Value>,
    },
    Replay {
        trace: Trace,
    },
    Resume {
        id: u32,
        response: Response,
    },
    Cancel {
        id: u32,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub version: u32,
    pub request: Request,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Message {
    Compiled {
        program: SafetyProgram,
    },
    Safety {
        result: SafetyResult,
    },
    SafetyCoverage {
        report: SafetyCoverageReport,
    },
    Loaded {
        handle: u32,
    },
    SafetyPrepared {
        handle: u32,
    },
    SafetyBatch {
        results: Vec<SafetyBatchResult>,
    },
    IncrementalSafety {
        result: SafetyResult,
        cache: SafetyExpansionCache,
        stats: ExpansionStats,
    },
    SafetyExpansions {
        cache: SafetyExpansionCache,
    },
    Released,
    History {
        result: ConcurrentResult,
    },
    Host {
        snapshot: Value,
    },
    Results {
        results: Vec<ConformanceResult>,
    },
    Candidates {
        ranges: Vec<DeletionRange>,
    },
    Value {
        value: Value,
    },
    Effect {
        effect: Effect,
    },
    Cancelled,
    Error {
        code: String,
        message: String,
    },
}

#[derive(Clone)]
enum Phase {
    Initial,
    Reset(Vec<Transition>),
    Model,
    Driver(Vec<Transition>),
    Done(ConformanceResult),
}

/// A host drives effects and owns I/O, async execution, and cleanup.
pub struct Session {
    id: u32,
    actions: Vec<Value>,
    oracle: Option<Trace>,
    phase: Phase,
    candidates: Vec<Transition>,
    trace: Trace,
}
impl Session {
    pub fn new(model: String, actions: Vec<Value>) -> Result<Self, CoreError> {
        Ok(Self {
            id: 1,
            actions: actions
                .into_iter()
                .map(Value::normalize)
                .collect::<Result<_, _>>()?,
            oracle: None,
            phase: Phase::Initial,
            candidates: vec![],
            trace: Trace {
                format: "modelt.portable-trace".into(),
                version: 1,
                model,
                initial: Value::Null,
                steps: vec![],
            },
        })
    }
    pub fn replay(trace: Trace) -> Result<Self, CoreError> {
        let trace = trace.normalize()?;
        let mut session = Self::new(
            trace.model.clone(),
            trace.steps.iter().map(|step| step.action.clone()).collect(),
        )?;
        session.phase = Phase::Reset(vec![Transition {
            key: String::new(),
            state: Value::Null,
            observation: trace.initial.clone(),
        }]);
        session.oracle = Some(trace);
        Ok(session)
    }
    pub fn effect(&self) -> Effect {
        match &self.phase {
            Phase::Initial => Effect::ModelInitial { id: self.id },
            Phase::Reset(_) => Effect::DriverReset { id: self.id },
            Phase::Model => Effect::ModelStep {
                id: self.id,
                states: self
                    .candidates
                    .iter()
                    .map(|entry| State {
                        key: entry.key.clone(),
                        state: entry.state.clone(),
                    })
                    .collect(),
                action: self.actions[self.trace.steps.len()].clone(),
            },
            Phase::Driver(_) => Effect::DriverStep {
                id: self.id,
                action: self.actions[self.trace.steps.len()].clone(),
            },
            Phase::Done(result) => Effect::Done {
                result: result.clone(),
            },
        }
    }
    pub fn resume(&mut self, id: u32, response: Response) -> Result<Effect, CoreError> {
        if id != self.id || matches!(self.phase, Phase::Done(_)) {
            return Err(CoreError::protocol("Stale or unexpected reply"));
        }
        let response = match (&self.phase, response) {
            (Phase::Initial | Phase::Model, Response::Transitions { transitions }) => {
                Response::Transitions {
                    transitions: transitions
                        .into_iter()
                        .map(|entry| {
                            Ok(Transition {
                                state: entry.state.normalize()?,
                                observation: entry.observation.normalize()?,
                                ..entry
                            })
                        })
                        .collect::<Result<_, CoreError>>()?,
                }
            }
            (Phase::Reset(_) | Phase::Driver(_), Response::Observation { observation }) => {
                Response::Observation {
                    observation: observation.normalize()?,
                }
            }
            _ => {
                return Err(CoreError::protocol(
                    "Reply kind does not match the pending effect",
                ));
            }
        };
        self.id = self
            .id
            .checked_add(1)
            .ok_or_else(|| CoreError::protocol("Request ID exhausted"))?;
        match (self.phase.clone(), response) {
            (Phase::Initial, Response::Transitions { transitions }) => {
                self.phase = Phase::Reset(transitions)
            }
            (Phase::Model, Response::Transitions { transitions }) => {
                if transitions.is_empty() {
                    self.finish(Status::Invalid, vec![]);
                } else {
                    self.phase = Phase::Driver(transitions);
                }
            }
            (Phase::Reset(transitions), Response::Observation { observation }) => {
                self.trace.initial = observation.clone();
                self.accept(transitions, observation);
            }
            (Phase::Driver(transitions), Response::Observation { observation }) => {
                self.trace.steps.push(Step {
                    action: self.actions[self.trace.steps.len()].clone(),
                    observation: observation.clone(),
                });
                self.accept(transitions, observation);
            }
            _ => unreachable!(),
        }
        Ok(self.effect())
    }
    fn accept(&mut self, transitions: Vec<Transition>, actual: Value) {
        self.candidates.clear();
        for entry in transitions
            .iter()
            .filter(|entry| entry.observation == actual)
        {
            if let Some(previous) = self
                .candidates
                .iter_mut()
                .find(|previous| previous.key == entry.key)
            {
                *previous = entry.clone();
            } else {
                self.candidates.push(entry.clone());
            }
        }
        if self.candidates.is_empty() {
            self.finish(
                if transitions.is_empty() {
                    Status::Invalid
                } else {
                    Status::Mismatch
                },
                transitions
                    .iter()
                    .map(|entry| entry.observation.clone())
                    .collect(),
            );
        } else if self.trace.steps.len() == self.actions.len() {
            self.finish(Status::Matched, vec![actual]);
        } else if let Some(oracle) = &self.oracle {
            self.phase = Phase::Driver(vec![Transition {
                key: String::new(),
                state: Value::Null,
                observation: oracle.steps[self.trace.steps.len()].observation.clone(),
            }]);
        } else {
            self.phase = Phase::Model;
        }
    }
    fn finish(&mut self, status: Status, expected: Vec<Value>) {
        let actual = self.trace.steps.last().map_or_else(
            || self.trace.initial.clone(),
            |step| step.observation.clone(),
        );
        self.phase = Phase::Done(ConformanceResult {
            status,
            checked_steps: self.trace.steps.len(),
            actual,
            expected,
            trace: self.trace.clone(),
        });
    }
}

pub struct Engine {
    session: Option<Session>,
    finite: Option<Arc<PreparedFiniteModel>>,
    models: BTreeMap<u32, Arc<PreparedFiniteModel>>,
    safety_programs: BTreeMap<u32, PreparedSafetyProgram>,
    active_handle: Option<u32>,
    next_handle: u32,
    next_id: u32,
}
impl Default for Engine {
    fn default() -> Self {
        Self {
            session: None,
            finite: None,
            models: BTreeMap::new(),
            safety_programs: BTreeMap::new(),
            active_handle: None,
            next_handle: 1,
            next_id: 1,
        }
    }
}
impl Engine {
    pub fn dispatch(&mut self, bytes: &[u8]) -> Message {
        match serde_json::from_slice(bytes) {
            Ok(command) => self.handle(command),
            Err(error) => Message::Error {
                code: "protocol".into(),
                message: error.to_string(),
            },
        }
    }
    pub fn handle(&mut self, command: Command) -> Message {
        match self.execute(command) {
            Ok(message) => message,
            Err(error) => Message::Error {
                code: error.code.into(),
                message: error.message,
            },
        }
    }
    fn execute(&mut self, command: Command) -> Result<Message, CoreError> {
        if command.version != 1 {
            return Err(CoreError::protocol("Unsupported protocol version"));
        }
        match command.request {
            Request::CompileTla {
                source,
                config,
                modules,
                irVersion,
                syntaxVersion,
            } => Ok(Message::Compiled {
                program: compile_tla_bundle(
                    &source,
                    &config,
                    &modules,
                    CompileOptions {
                        irVersion: IrVersion::try_from(irVersion)
                            .map_err(|error| CoreError::protocol(&error.to_string()))?,
                    },
                    syntaxVersion,
                )?,
            }),
            Request::CheckTla {
                source,
                config,
                modules,
                irVersion,
                syntaxVersion,
                bounds,
            } => Ok(Message::Safety {
                result: compile_tla_bundle(
                    &source,
                    &config,
                    &modules,
                    CompileOptions {
                        irVersion: IrVersion::try_from(irVersion)
                            .map_err(|error| CoreError::protocol(&error.to_string()))?,
                    },
                    syntaxVersion,
                )?
                .check(bounds)?,
            }),
            #[cfg(not(target_arch = "wasm32"))]
            Request::CheckSafetyParallel {
                program,
                bounds,
                workers,
            } => Ok(Message::Safety {
                result: program.check_parallel(bounds, workers)?,
            }),
            Request::CheckSafety {
                program,
                bounds,
                options,
            } => Ok(Message::Safety {
                result: program.check_with_options(bounds, options.unwrap_or_default())?,
            }),
            Request::CheckSafetyCoverage { program, bounds } => Ok(Message::SafetyCoverage {
                report: program.check_with_coverage(bounds)?,
            }),
            Request::PrepareSafety { program } => {
                let prepared = program.prepare()?;
                let next = self
                    .next_handle
                    .checked_add(1)
                    .ok_or_else(|| CoreError::protocol("Model handles exhausted"))?;
                let handle = self.next_handle;
                self.safety_programs.insert(handle, prepared);
                self.next_handle = next;
                Ok(Message::SafetyPrepared { handle })
            }
            Request::CheckPreparedSafety {
                handle,
                bounds,
                options,
            } => Ok(Message::Safety {
                result: self
                    .get_safety_program(handle)?
                    .check_with_options(bounds, options.unwrap_or_default())?,
            }),
            Request::CheckPreparedSafetyBatch { handle, jobs } => {
                let program = self.get_safety_program(handle)?;
                if jobs.is_empty() || jobs.len() > 256 {
                    return Err(CoreError::protocol(
                        "Safety batch must contain 1 to 256 jobs",
                    ));
                }
                let results = jobs
                    .into_iter()
                    .map(|job| {
                        match program
                            .check_with_options(job.bounds, job.options.unwrap_or_default())
                        {
                            Ok(result) => SafetyBatchResult::Safety { result },
                            Err(error) => SafetyBatchResult::Error {
                                code: error.code.into(),
                                message: error.message,
                            },
                        }
                    })
                    .collect();
                Ok(Message::SafetyBatch { results })
            }
            Request::ReleaseSafety { handle } => {
                self.safety_programs
                    .remove(&handle)
                    .ok_or_else(|| CoreError::protocol("Unknown safety handle"))?;
                Ok(Message::Released)
            }
            Request::CheckSafetyIncremental {
                enabled,
                program,
                bounds,
                mut cache,
                build_digest,
                max_entries,
            } => {
                let checked = program.check_incremental(
                    bounds,
                    &mut cache,
                    ExpansionOptions {
                        enabled,
                        build_digest: &build_digest,
                        max_entries,
                    },
                )?;
                Ok(Message::IncrementalSafety {
                    result: checked.result,
                    stats: checked.stats,
                    cache,
                })
            }
            Request::MergeSafetyExpansions {
                mut left,
                right,
                max_entries,
            } => {
                left.merge(right, max_entries)?;
                Ok(Message::SafetyExpansions { cache: left })
            }
            Request::LoadModel { model } => {
                let model = Arc::new(model.prepare()?);
                let next = self
                    .next_handle
                    .checked_add(1)
                    .ok_or_else(|| CoreError::protocol("Model handles exhausted"))?;
                let handle = self.next_handle;
                self.models.insert(handle, model);
                self.next_handle = next;
                Ok(Message::Loaded { handle })
            }
            Request::ReleaseModel { handle } => {
                if self.active_handle == Some(handle)
                    && self
                        .session
                        .as_ref()
                        .is_some_and(|session| !matches!(session.phase, Phase::Done(_)))
                {
                    return Err(CoreError::protocol("Model handle is in use"));
                }
                self.models
                    .remove(&handle)
                    .ok_or_else(|| CoreError::protocol("Unknown model handle"))?;
                if self.active_handle == Some(handle) {
                    self.finite = None;
                    self.active_handle = None;
                }
                Ok(Message::Released)
            }
            Request::StartLoaded { handle, actions } => {
                let model = self.get_model(handle)?;
                self.begin(
                    Session::new(model.name().into(), actions)?,
                    Some(model),
                    Some(handle),
                )
            }
            Request::CheckLoaded { handle, traces } => {
                let model = self.get_model(handle)?;
                Ok(Message::Results {
                    results: traces
                        .into_iter()
                        .map(|trace| model.check_history(trace))
                        .collect::<Result<_, _>>()?,
                })
            }
            Request::CheckHistory {
                handle,
                history,
                max_nodes,
            } => Ok(Message::History {
                result: self
                    .get_model(handle)?
                    .check_concurrent(history, max_nodes)?,
            }),
            Request::ReplayHost { tape } => Ok(Message::Host {
                snapshot: replay_host(tape)?,
            }),
            Request::Canonicalize { value } => Ok(Message::Value {
                value: value.normalize()?,
            }),
            Request::Start { model, actions } => {
                self.begin(Session::new(model, actions)?, None, None)
            }
            Request::Replay { trace } => self.begin(Session::replay(trace)?, None, None),
            Request::StartFinite { model, actions } => {
                let model = Arc::new(model.prepare()?);
                self.begin(
                    Session::new(model.name().into(), actions)?,
                    Some(model),
                    None,
                )
            }
            Request::CheckFinite { model, traces } => {
                let model = model.prepare()?;
                Ok(Message::Results {
                    results: traces
                        .into_iter()
                        .map(|trace| model.check_history(trace))
                        .collect::<Result<_, _>>()?,
                })
            }
            Request::ShrinkCandidates { length } => Ok(Message::Candidates {
                ranges: deletion_candidates(length),
            }),
            Request::Resume { id, response } => {
                let session = self
                    .session
                    .as_mut()
                    .ok_or_else(|| CoreError::protocol("No active session"))?;
                let effect = session.resume(id, response)?;
                self.next_id = session.id;
                self.drive(effect)
            }
            Request::Cancel { id } => {
                let session = self
                    .session
                    .as_ref()
                    .ok_or_else(|| CoreError::protocol("No active session"))?;
                if id != session.id || matches!(session.phase, Phase::Done(_)) {
                    return Err(CoreError::protocol("Stale cancellation"));
                }
                self.next_id = id
                    .checked_add(1)
                    .ok_or_else(|| CoreError::protocol("Request ID exhausted"))?;
                self.session = None;
                self.finite = None;
                self.active_handle = None;
                Ok(Message::Cancelled)
            }
        }
    }
    fn begin(
        &mut self,
        mut session: Session,
        finite: Option<Arc<PreparedFiniteModel>>,
        handle: Option<u32>,
    ) -> Result<Message, CoreError> {
        if self
            .session
            .as_ref()
            .is_some_and(|current| !matches!(current.phase, Phase::Done(_)))
        {
            return Err(CoreError::protocol("Session already active"));
        }
        session.id = self.next_id;
        let effect = session.effect();
        self.session = Some(session);
        self.finite = finite;
        self.active_handle = handle;
        self.drive(effect)
    }
    fn get_safety_program(&self, handle: u32) -> Result<&PreparedSafetyProgram, CoreError> {
        self.safety_programs
            .get(&handle)
            .ok_or_else(|| CoreError::protocol("Unknown safety handle"))
    }

    fn get_model(&self, handle: u32) -> Result<Arc<PreparedFiniteModel>, CoreError> {
        self.models
            .get(&handle)
            .cloned()
            .ok_or_else(|| CoreError::protocol("Unknown model handle"))
    }
    fn drive(&mut self, mut effect: Effect) -> Result<Message, CoreError> {
        while let Some(response) = self
            .finite
            .as_ref()
            .map(|model| model.response(&effect))
            .transpose()?
            .flatten()
        {
            let id = match effect {
                Effect::ModelInitial { id } | Effect::ModelStep { id, .. } => id,
                _ => unreachable!(),
            };
            let session = self
                .session
                .as_mut()
                .ok_or_else(|| CoreError::protocol("No finite session"))?;
            effect = session.resume(id, response)?;
            self.next_id = session.id;
        }
        Ok(Message::Effect { effect })
    }
}
