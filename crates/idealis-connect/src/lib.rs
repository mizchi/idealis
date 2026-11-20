mod compare;
mod engine;
mod generator;
mod schedule;
mod session;
mod shrink;
#[cfg(feature = "tla")]
pub mod tla;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use compare::{Comparison, ComparisonStatus, Differential, Observation, Target, compare};
pub use engine::Engine;
pub use generator::{ActionEdge, Case, Generator};
pub use schedule::{Schedule, ScheduleActor};
pub use session::{
    Effect, Phase, Plan, Session, ShrinkStatus, ShrinkSummary, TestResult, TestStatus, Trial,
    Verdict,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorKind {
    Protocol,
    Resource,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}
impl Error {
    fn protocol(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Protocol,
            message: message.into(),
        }
    }
    fn resource(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Resource,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for Error {}

fn text(value: &str) -> Result<(), Error> {
    if value.is_empty() || value.len() > 256 {
        return Err(Error::protocol("identifier must have 1-256 bytes"));
    }
    Ok(())
}
fn canonical(value: &Value, depth: usize) -> Result<Value, Error> {
    if depth > 32 {
        return Err(Error::resource("JSON nesting exceeds 32"));
    }
    Ok(match value {
        Value::Number(number) => {
            let number = number
                .as_f64()
                .ok_or_else(|| Error::protocol("invalid JSON number"))?;
            if number.fract() != 0.0 || number.abs() > 9_007_199_254_740_991.0 {
                return Err(Error::protocol(
                    "use tagged strings for numbers outside safe integers",
                ));
            }
            Value::from(number as i64)
        }
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| canonical(value, depth + 1))
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(name, value)| Ok((name.clone(), canonical(value, depth + 1)?)))
                .collect::<Result<_, Error>>()?,
        ),
        _ => value.clone(),
    })
}
struct Work {
    used: u32,
    maximum: u32,
}
impl Work {
    fn tick(&mut self) -> Result<(), ()> {
        if self.used == self.maximum {
            return Err(());
        }
        self.used += 1;
        Ok(())
    }
}
