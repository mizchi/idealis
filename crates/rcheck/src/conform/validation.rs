//! Validate an implementation request against a tracked reference state.
use super::{canonical, element, relation_value, relations};
use rmodel::reference::{Machine, Next, Snapshot};
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
    machine: Machine<'m>,
    schema: Value,
}

impl<'m> Validator<'m> {
    pub fn new(model: &'m rmodel::Model) -> Result<Self, String> {
        let machine = Machine::new(model)?;
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
            machine,
            schema,
        })
    }

    /// Check an explicitly supplied initial snapshot before starting the domain.
    pub fn start<'v>(
        &'v self,
        config: &Value,
        initial: &Value,
    ) -> Result<Session<'v, 'm>, ValidationError> {
        self.check_input("Config", config)?;
        self.check_input("State", initial)?;
        let matching = |wire: &Value, name: &str, data: Vec<Vec<rmodel::reference::Data>>| {
            canonical(&relation_value(self.model, name, &data)) == wire[name]
        };
        let config = canonical(config);
        let configuration = self
            .machine
            .match_configuration(|name, data| matching(&config, name, data))
            .ok_or_else(|| {
                ValidationError::InvalidInput("configuration does not satisfy the facts".into())
            })?;
        let initial = canonical(initial);
        let state = self
            .machine
            .match_initial(&configuration, |name, data| matching(&initial, name, data))
            .ok_or_else(|| ValidationError::InvalidInput("state does not satisfy init".into()))?;
        self.check_invariants(&configuration, &state)?;
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
        match self.machine.violated_invariant(config, state) {
            Some(name) => Err(ValidationError::OutsideModel(format!("invariant {name}"))),
            None => Ok(()),
        }
    }

    fn state_json(&self, state: &Snapshot) -> Value {
        canonical(&relations(self.model, self.machine.state_data(state)))
    }
}

pub struct Session<'v, 'm> {
    validator: &'v Validator<'m>,
    configuration: Snapshot,
    state: Snapshot,
    diverged: bool,
}

impl Session<'_, '_> {
    fn next(&self, input: &Value) -> Result<Snapshot, ValidationError> {
        if self.diverged {
            return Err(ValidationError::Domain(
                "previous operation did not match the model".into(),
            ));
        }
        self.validator.check_input("Command", input)?;
        let action = input["kind"]
            .as_str()
            .ok_or_else(|| ValidationError::InvalidInput("missing command kind".into()))?;
        let next = self
            .validator
            .machine
            .match_step(&self.configuration, &self.state, action, |param, value| {
                element(&value) == input[param]
            })
            .ok_or_else(|| {
                ValidationError::InvalidInput("command is outside the action domain".into())
            })?;
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
                if let Some(name) = self.validator.machine.violated_transition(
                    &self.configuration,
                    &self.state,
                    &state,
                ) {
                    return Err(ValidationError::OutsideModel(format!("transition {name}")));
                }
                Ok(state)
            }
        }
    }

    /// Pure preflight for a command. Returns its expected next snapshot.
    pub fn validate(&self, input: &Value) -> Result<Value, ValidationError> {
        self.next(input)
            .map(|state| self.validator.state_json(&state))
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
