//! Validate an implementation request against a tracked reference state.
use super::{canonical, element, relations};
use rmodel::bindings::command_argument_key;
use rmodel::reference::{BoundCommand, Data, LassoError, Next, Runtime, Snapshot};
use rmodel::{INT, Model, Mult, NAT, Shape};
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    InvalidInput(String),
    Disabled(String),
    OutsideModel(String),
    Domain(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, reason) = match self {
            Self::InvalidInput(reason) => ("invalid input", reason),
            Self::Disabled(reason) => ("guard is false", reason),
            Self::OutsideModel(reason) => ("outside the model", reason),
            Self::Domain(reason) => ("domain divergence", reason),
        };
        write!(f, "{kind}: {reason}")
    }
}

impl std::error::Error for ValidationError {}

pub struct Validator<'m> {
    model: &'m rmodel::Model,
    runtime: Runtime<'m>,
    schema: Value,
}

impl<'m> Validator<'m> {
    pub fn new(model: &'m rmodel::Model) -> Result<Self, String> {
        let runtime = Runtime::new(model);
        let mut schema: Value = serde_json::from_str(&rmodel::bindings::json_schema(model))
            .expect("the generator emits JSON Schema");
        if let Some((low, high)) = model.scope.ints {
            for (sort, low) in [(rmodel::INT, low), (rmodel::NAT, low.max(0))] {
                if let Some(bounds) = schema["$defs"].get_mut(sort) {
                    bounds["minimum"] = Value::from(low);
                    bounds["maximum"] = Value::from(high);
                }
            }
        }
        Ok(Self {
            model,
            runtime,
            schema,
        })
    }

    /// Check an explicitly supplied initial snapshot before starting the domain.
    pub fn start<'v>(
        &'v self,
        config: &Value,
        initial: &Value,
    ) -> Result<Session<'v, 'm>, ValidationError> {
        let session = self.start_path(config, initial)?;
        self.check_invariants(&session.configuration, &session.state)?;
        Ok(session)
    }

    fn start_path<'v>(
        &'v self,
        config: &Value,
        initial: &Value,
    ) -> Result<Session<'v, 'm>, ValidationError> {
        self.check_input("Config", config)?;
        self.check_input("State", initial)?;
        let configuration = self
            .runtime
            .read_configuration(|relation| {
                relation_input(self.model, &relation.shape, &config[&relation.name])
            })
            .ok_or_else(|| {
                ValidationError::InvalidInput("configuration does not satisfy the facts".into())
            })?;
        let state = self
            .runtime
            .read_initial(&configuration, |relation| {
                relation_input(self.model, &relation.shape, &initial[&relation.name])
            })
            .ok_or_else(|| ValidationError::InvalidInput("state does not satisfy init".into()))?;
        Ok(Session {
            validator: self,
            configuration,
            state,
            diverged: false,
        })
    }

    fn check_input(&self, definition: &str, value: &Value) -> Result<(), ValidationError> {
        crate::schema::validate(&self.schema, definition, value)
            .map_err(ValidationError::InvalidInput)
    }

    fn check_invariants(&self, config: &Snapshot, state: &Snapshot) -> Result<(), ValidationError> {
        match self.runtime.violated_invariant(config, state) {
            Some(name) => Err(ValidationError::OutsideModel(format!("invariant {name}"))),
            None => Ok(()),
        }
    }

    fn state_json(&self, state: &Snapshot) -> Value {
        relations(self.model, self.runtime.state_data(state))
    }
}

fn typed_value(model: &Model, sort: &str, wire: &Value) -> Option<Data> {
    if matches!(sort, INT | NAT) {
        return wire.as_i64().map(Data::Int);
    }
    if let Some(record) = model.record(sort) {
        let fields = record
            .fields
            .iter()
            .map(|(name, sort)| Some((name.clone(), typed_value(model, sort, wire.get(name)?)?)))
            .collect::<Option<_>>()?;
        return Some(Data::Record(record.name.clone(), fields));
    }
    if let Some(sequence) = model.sequence(sort) {
        let items = wire.get("$sequence")?.as_array()?;
        return items
            .iter()
            .map(|item| typed_value(model, &sequence.element, item))
            .collect::<Option<_>>()
            .map(Data::Sequence);
    }
    if let Some(variant) = model.variant(sort) {
        let tag = wire.get("tag")?.as_str()?;
        let case = variant.cases.iter().find(|case| case.tag == tag)?;
        let payload = match case.payload.as_deref() {
            Some(sort) => Some(Box::new(typed_value(model, sort, wire.get("value")?)?)),
            None => None,
        };
        return Some(Data::Variant(tag.to_owned(), payload));
    }
    let name = if let Some(boolean) = wire.as_bool() {
        boolean.to_string()
    } else {
        wire.as_str()?.to_owned()
    };
    let atom = Data::Atom(name);
    (element(&atom) == *wire).then_some(atom)
}

fn values(model: &Model, sort: &str, mult: Mult, wire: &Value) -> Option<Vec<Data>> {
    match mult {
        Mult::Set | Mult::Some => wire
            .as_array()?
            .iter()
            .map(|item| typed_value(model, sort, item))
            .collect(),
        Mult::Lone if wire.is_null() => Some(Vec::new()),
        Mult::Lone | Mult::One => Some(vec![typed_value(model, sort, wire)?]),
    }
}

fn relation_input(model: &Model, shape: &Shape, wire: &Value) -> Option<Vec<Vec<Data>>> {
    match shape {
        Shape::Unary { sig, mult } => Some(
            values(model, sig, *mult, wire)?
                .into_iter()
                .map(|atom| vec![atom])
                .collect(),
        ),
        Shape::Binary { to, mult, .. } => {
            let mut tuples = Vec::new();
            for (key, row) in wire.as_object()? {
                tuples.extend(
                    values(model, to, *mult, row)?
                        .into_iter()
                        .map(|atom| vec![Data::Atom(key.clone()), atom]),
                );
            }
            Some(tuples)
        }
    }
}

pub struct Session<'v, 'm> {
    validator: &'v Validator<'m>,
    configuration: Snapshot,
    state: Snapshot,
    diverged: bool,
}

impl<'m> Session<'_, 'm> {
    /// Read the current reference state in the bindings' JSON form.
    pub fn snapshot(&self) -> Value {
        self.validator.state_json(&self.state)
    }

    /// Project named derivations at the current reference state.
    pub fn snapshot_derivations(&self, names: &[String]) -> Result<Value, ValidationError> {
        self.project_derivations(&self.state, names)
    }

    fn project_derivations(
        &self,
        state: &Snapshot,
        names: &[String],
    ) -> Result<Value, ValidationError> {
        self.validator
            .runtime
            .derived_data(&self.configuration, state, names)
            .map(|fields| super::observation_json(self.validator.model, fields))
            .map_err(ValidationError::InvalidInput)
    }

    fn next(&self, input: &Value) -> Result<Snapshot, ValidationError> {
        if self.diverged {
            return Err(ValidationError::Domain(
                "previous operation did not match the model".into(),
            ));
        }
        let (action, command) = self.bind_command(input)?;
        let runtime = &self.validator.runtime;
        let next = runtime.execute(&self.configuration, &self.state, &command);
        match next {
            None => Err(ValidationError::Disabled(action.to_owned())),
            Some(Next::Outside(_)) => Err(ValidationError::OutsideModel(format!(
                "{action} leaves Scope"
            ))),
            Some(Next::Broken(_)) => Err(ValidationError::OutsideModel(format!(
                "{action} violates Declarations"
            ))),
            Some(Next::State(state)) => {
                self.validator
                    .check_invariants(&self.configuration, &state)?;
                if let Some(name) = runtime.violated_command_transition(
                    &self.configuration,
                    &self.state,
                    &state,
                    &command,
                ) {
                    return Err(ValidationError::OutsideModel(format!("transition {name}")));
                }
                Ok(state)
            }
        }
    }

    fn bind_command<'i>(
        &self,
        input: &'i Value,
    ) -> Result<(&'i str, BoundCommand<'m>), ValidationError> {
        self.validator.check_input("Command", input)?;
        let action = input["kind"]
            .as_str()
            .ok_or_else(|| ValidationError::InvalidInput("missing command kind".into()))?;
        let command = self
            .validator
            .runtime
            .read_command(action, |param, sort| {
                typed_value(
                    self.validator.model,
                    sort,
                    &input[command_argument_key(param)],
                )
            })
            .ok_or_else(|| {
                ValidationError::InvalidInput("command is outside the action domain".into())
            })?;
        Ok((action, command))
    }

    /// Pure preflight for a command. Returns its expected next snapshot.
    pub fn validate(&self, input: &Value) -> Result<Value, ValidationError> {
        self.next(input)
            .map(|state| self.validator.state_json(&state))
    }

    /// Preflight a command and project named derivations at both endpoints.
    pub fn validate_with_derivations(
        &self,
        input: &Value,
        names: &[String],
    ) -> Result<Value, ValidationError> {
        let next = self.next(input)?;
        Ok(serde_json::json!({
            "state": self.validator.state_json(&next),
            "derived": {
                "before": self.snapshot_derivations(names)?,
                "after": self.project_derivations(&next, names)?
            }
        }))
    }

    /// Advance the model only, for replaying an accepted prefix.
    pub fn replay(&mut self, input: &Value) -> Result<(), ValidationError> {
        self.state = self.next(input)?;
        Ok(())
    }

    /// Validate before calling the domain. A divergent or failed domain
    /// call invalidates the session because it may already have side effects.
    pub fn dispatch(
        &mut self,
        input: &Value,
        apply: impl FnOnce(&Value) -> Result<(bool, Value), String>,
    ) -> Result<Value, ValidationError> {
        let next = self.next(input)?;
        let expected = self.validator.state_json(&next);
        self.diverged = true;
        let (accepted, actual) = apply(input).map_err(ValidationError::Domain)?;
        if !accepted || canonical(&actual) != expected {
            return Err(ValidationError::Domain(format!(
                "expected acceptance with {expected}, observed accepted={accepted} with {actual}"
            )));
        }
        self.state = next;
        self.diverged = false;
        Ok(expected)
    }
}

/// Replay a finite path as evidence about one property.
pub(super) fn verify_witness(
    mut model: Model,
    question: &crate::Question,
    witness: &super::external::Witness,
) -> Result<(), String> {
    use crate::Kind;
    model.checks.retain(|check| check.name == question.name);
    model
        .transitions
        .retain(|check| check.name == question.name);
    if question.kind == Kind::Reachable {
        let check = &mut model.checks[0];
        check.kind = rmodel::CheckKind::Invariant;
        check.formula = rmodel::Formula::Not(Box::new(check.formula.clone()));
    }
    let validator = Validator::new(&model)?;
    let mut session = validator
        .start_path(&witness.config, &witness.initial)
        .map_err(|error| format!("step 0: {error}"))?;
    let declares = question.name == rmodel::DECLARATIONS;
    let mut verified_step = false;
    for (index, input) in witness.commands.iter().enumerate() {
        let step = index + 1;
        let (action, command) = session
            .bind_command(input)
            .map_err(|error| format!("step {step}: {error}"))?;
        let runtime = &validator.runtime;
        let last = step == witness.commands.len();
        match runtime.execute(&session.configuration, &session.state, &command) {
            None => return Err(format!("step {step}: guard is false: {action}")),
            Some(Next::Outside(_)) => return Err(format!("step {step}: {action} leaves Scope")),
            Some(Next::Broken(state)) if last && declares => {
                session.state = state;
                verified_step = true;
            }
            Some(Next::Broken(_)) => {
                return Err(format!("step {step}: {action} violates Declarations"));
            }
            Some(Next::State(state)) => {
                if last && question.kind == Kind::Transition {
                    verified_step = runtime.violated_command_transition(
                        &session.configuration,
                        &session.state,
                        &state,
                        &command,
                    ) == Some(question.name.as_str());
                }
                session.state = state;
            }
        }
    }
    if session.snapshot() != canonical(&witness.final_state) {
        return Err("witness final state does not match the replayed state".into());
    }
    if !declares && matches!(question.kind, Kind::Invariant | Kind::Reachable) {
        verified_step = validator
            .runtime
            .violated_invariant(&session.configuration, &session.state)
            == Some(question.name.as_str());
    }
    if !verified_step {
        return Err(format!("witness does not establish {}", question.name));
    }
    Ok(())
}

/// Replay legal edges and stuttering from a model initial state before checking liveness.
pub(super) fn verify_lasso(
    model: &Model,
    question: &crate::Question,
    witness: &super::external::Witness,
    loop_start: usize,
    work: u64,
) -> Result<(), LassoError> {
    if loop_start >= witness.commands.len() {
        return Err(LassoError::Invalid(
            "loopStart must name a state before the closing edge".into(),
        ));
    }
    let validator = Validator::new(model).map_err(LassoError::Invalid)?;
    let mut session = validator
        .start_path(&witness.config, &witness.initial)
        .map_err(|error| LassoError::Invalid(format!("step 0: {error}")))?;
    let mut states = vec![session.state.clone()];
    for (index, input) in witness.commands.iter().enumerate() {
        if !input.is_null() {
            let invalid = |reason: &dyn fmt::Display| {
                LassoError::Invalid(format!("step {}: {reason}", index + 1))
            };
            let (action, command) = session
                .bind_command(input)
                .map_err(|error| invalid(&error))?;
            session.state =
                match validator
                    .runtime
                    .execute(&session.configuration, &session.state, &command)
                {
                    None => return Err(invalid(&format_args!("guard is false: {action}"))),
                    Some(Next::Outside(_)) => {
                        return Err(invalid(&format_args!("{action} leaves Scope")));
                    }
                    Some(Next::Broken(_)) => {
                        return Err(invalid(&format_args!("{action} violates Declarations")));
                    }
                    Some(Next::State(state)) => state,
                };
        }
        states.push(session.state.clone());
    }
    if session.snapshot() != canonical(&witness.final_state) {
        return Err(LassoError::Invalid(
            "witness final state does not match the replayed state".into(),
        ));
    }
    validator.runtime.verify_temporal_lasso(
        &session.configuration,
        &states,
        loop_start,
        &question.name,
        work,
    )
}
