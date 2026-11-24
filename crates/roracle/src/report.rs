use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub enum Status {
    Matched,
    Mismatch,
    Invalid,
    Unknown,
    Error,
}

impl Status {
    pub(crate) fn aggregate(values: impl Iterator<Item = Self>) -> Self {
        values
            .max_by_key(|status| match status {
                Self::Matched => 0,
                Self::Invalid => 1,
                Self::Unknown => 2,
                Self::Mismatch => 3,
                Self::Error => 4,
            })
            .unwrap_or(Self::Invalid)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub enum Observation<T> {
    Value { value: T },
    Allowed { values: Vec<T> },
    Unknown { reason: String },
    Invalid { reason: String },
    Error { message: String },
}

impl<T> Observation<T> {
    pub fn value(value: T) -> Self {
        Self::Value { value }
    }

    fn failure(&self) -> Option<Status> {
        match self {
            Self::Unknown { .. } => Some(Status::Unknown),
            Self::Invalid { .. } => Some(Status::Invalid),
            Self::Error { .. } => Some(Status::Error),
            _ => None,
        }
    }
}

impl<T, E: Display> From<Result<T, E>> for Observation<T> {
    fn from(result: Result<T, E>) -> Self {
        match result {
            Ok(value) => Self::value(value),
            Err(error) => Self::Error {
                message: error.to_string(),
            },
        }
    }
}

pub(crate) fn compare<T: PartialEq>(reference: &Observation<T>, actual: &Observation<T>) -> Status {
    if reference.failure().is_some() || actual.failure().is_some() {
        return Status::aggregate(reference.failure().into_iter().chain(actual.failure()));
    }
    let Observation::Value { value } = actual else {
        return Status::Invalid;
    };
    let matched = match reference {
        Observation::Value { value: expected } => value == expected,
        Observation::Allowed { values } if !values.is_empty() => values.contains(value),
        _ => return Status::Invalid,
    };
    if matched {
        Status::Matched
    } else {
        Status::Mismatch
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct Case<I> {
    pub id: String,
    pub input: I,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct Comparison<O> {
    pub id: String,
    pub status: Status,
    pub actual: Observation<O>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct CaseReport<I, O> {
    pub case: Case<I>,
    pub reference: Observation<O>,
    pub comparisons: Vec<Comparison<O>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct CheckReport<I, O> {
    pub version: u32,
    pub group: String,
    pub reference: String,
    pub implementations: Vec<String>,
    pub status: Status,
    pub cases: Vec<CaseReport<I, O>>,
}

impl<I: Debug, O: Debug> CheckReport<I, O> {
    #[track_caller]
    pub fn assert_matched(&self) {
        assert_eq!(self.status, Status::Matched, "{self:#?}");
    }
}

pub(crate) fn identities<'a>(ids: impl Iterator<Item = &'a str>) -> Result<(), Error> {
    let mut seen = std::collections::BTreeSet::new();
    for id in ids {
        if id.trim().is_empty() || id.len() > 256 || !seen.insert(id) {
            return Err(Error(
                "identities must be nonempty, unique and at most 256 bytes".into(),
            ));
        }
    }
    Ok(())
}
