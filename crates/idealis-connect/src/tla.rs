//! Optional adapter for a finite TLA+ model with action and observation variables.
use crate::{ActionEdge, Effect, Error, Generator, Phase, Plan, Session, TestResult, Verdict};
use idealis_core::{
    ConformanceResult, CoreError, Edge, Effect as ModelEffect, FiniteModel, FiniteState, Initial,
    PreparedFiniteModel, Response,
};
pub use idealis_core::{SafetyBounds, Status, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub struct ConnectSpec<'a> {
    pub action: &'a str,
    pub observation: &'a str,
    /// Merge states differing only in the annotations if their projected edges agree.
    pub project_annotations: bool,
}
impl Default for ConnectSpec<'_> {
    fn default() -> Self {
        Self {
            action: "act",
            observation: "obs",
            project_annotations: true,
        }
    }
}

pub fn finite_model(
    source: &str,
    config: &str,
    spec: ConnectSpec<'_>,
    bounds: SafetyBounds,
) -> Result<FiniteModel, CoreError> {
    let program = idealis_core::compile_tla(source, config)?;
    let slot = |name: &str| {
        program
            .variables
            .iter()
            .position(|v| v == name)
            .ok_or_else(|| CoreError::protocol(&format!("Missing connect variable: {name}")))
    };
    let action = slot(spec.action)?;
    let observation = slot(spec.observation)?;
    if action == observation {
        return Err(CoreError::protocol(
            "Expected distinct action and observation variables",
        ));
    }
    let graph = program.reachable_graph(bounds)?;
    let mut groups = HashMap::new();
    let mut states = Vec::new();
    let mut indices = Vec::with_capacity(graph.states.len());
    for values in &graph.states {
        let key: Vec<_> = values
            .iter()
            .enumerate()
            .filter(|(i, _)| !spec.project_annotations || (*i != action && *i != observation))
            .map(|(_, v)| v)
            .collect();
        let next = groups.len();
        let group = *groups.entry(key).or_insert(next);
        if group == states.len() {
            states.push(FiniteState {
                key: group.to_string(),
                violations: vec![],
                transitions: vec![],
            });
        }
        indices.push(group);
    }

    let mut seen_initial = HashSet::new();
    let mut initial = vec![];
    for i in graph.initial {
        let value = &graph.states[i][observation];
        if seen_initial.insert((indices[i], value)) {
            initial.push(Initial {
                key: states[indices[i]].key.clone(),
                observation: value.clone(),
            });
        }
    }
    let mut signatures: Vec<Option<HashSet<_>>> = vec![None; states.len()];
    for (i, successors) in graph.successors.iter().enumerate() {
        let group = indices[i];
        let mut signature = HashSet::new();
        for &target in successors {
            let values = &graph.states[target];
            if signature.insert((&values[action], indices[target], &values[observation]))
                && signatures[group].is_none()
            {
                let key = states[indices[target]].key.clone();
                states[group].transitions.push(Edge {
                    action: values[action].clone(),
                    target: key,
                    observation: values[observation].clone(),
                });
            }
        }
        if let Some(previous) = &signatures[group] {
            if previous != &signature {
                return Err(CoreError::protocol(
                    "Unsafe connect projection: outgoing transitions differ; keep the annotations with project_annotations: false",
                ));
            }
        } else {
            signatures[group] = Some(signature);
        }
    }
    FiniteModel {
        format: "modelt.finite-model".into(),
        version: 1,
        name: program.name,
        initial,
        states,
    }
    .normalize()
}

/// Implement on a test adapter. Each reset starts a fresh case.
pub trait Driver {
    fn reset(&mut self) -> Result<Value, String>;
    fn step(&mut self, action: &Value) -> Result<Value, String>;
}

fn model_error(error: impl std::fmt::Display) -> Error {
    Error::protocol(error.to_string())
}

pub fn check_driver(
    model: &FiniteModel,
    actions: Vec<Value>,
    driver: &mut impl Driver,
) -> Result<ConformanceResult, Error> {
    drive(
        &model.clone().prepare().map_err(model_error)?,
        actions,
        driver,
    )
}
fn drive(
    model: &PreparedFiniteModel,
    actions: Vec<Value>,
    driver: &mut impl Driver,
) -> Result<ConformanceResult, Error> {
    let mut session =
        idealis_core::Session::new(model.name().into(), actions).map_err(model_error)?;
    loop {
        let effect = session.effect();
        let (id, response) = match &effect {
            ModelEffect::ModelInitial { id } | ModelEffect::ModelStep { id, .. } => (
                *id,
                model
                    .response(&effect)
                    .map_err(model_error)?
                    .expect("model effect"),
            ),
            ModelEffect::DriverReset { id } => (
                *id,
                Response::Observation {
                    observation: driver.reset().map_err(model_error)?,
                },
            ),
            ModelEffect::DriverStep { id, action } => (
                *id,
                Response::Observation {
                    observation: driver.step(action).map_err(model_error)?,
                },
            ),
            ModelEffect::Done { result } => return Ok(result.clone()),
        };
        session.resume(id, response).map_err(model_error)?;
    }
}

impl Generator {
    pub fn from_finite_model(
        model: &FiniteModel,
        version: &str,
        min_length: usize,
        max_length: usize,
    ) -> Result<Self, Error> {
        let model = model.clone().normalize().map_err(model_error)?;
        let initial = model
            .initial
            .into_iter()
            .map(|i| i.key)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut states = BTreeMap::new();
        for state in model.states {
            let mut edges = vec![];
            let mut seen = HashSet::new();
            for edge in state.transitions {
                if seen.insert((edge.action.clone(), edge.target.clone())) {
                    edges.push(ActionEdge {
                        action: serde_json::to_value(edge.action).map_err(model_error)?,
                        target: edge.target,
                    });
                }
            }
            states.insert(state.key, edges);
        }
        Self {
            id: model.name,
            version: version.into(),
            initial,
            states,
            min_length,
            max_length,
        }
        .normalize()
    }
}

pub struct Report {
    pub result: TestResult,
    pub counterexample: Option<ConformanceResult>,
}

pub fn check(model: &FiniteModel, plan: Plan, driver: &mut impl Driver) -> Result<Report, Error> {
    let model = model.clone().prepare().map_err(model_error)?;
    let mut session = Session::property(plan)?;
    let mut counterexample = None;
    loop {
        match session.effect() {
            Effect::Evaluate { id, phase, case } => {
                let actions = case
                    .actions
                    .into_iter()
                    .map(serde_json::from_value)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(model_error)?;
                let verdict = match drive(&model, actions, driver) {
                    Ok(result) => match result.status {
                        Status::Matched => Verdict::Matched {},
                        Status::Invalid => Verdict::Invalid {},
                        Status::Mismatch => {
                            if phase == Phase::Confirmation {
                                counterexample = Some(result);
                            }
                            Verdict::Mismatch {
                                signature: "observation".into(),
                            }
                        }
                    },
                    Err(error) => Verdict::Error {
                        message: error.to_string(),
                    },
                };
                session.resume(id, verdict)?;
            }
            Effect::Done { result } => {
                return Ok(Report {
                    result,
                    counterexample,
                });
            }
        }
    }
}
