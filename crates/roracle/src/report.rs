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

#[cfg(test)]
mod tests {
    use super::Status;
    use std::collections::BTreeMap;

    /// Every combination of zero to three comparison statuses, folded by
    /// `Status::aggregate`, is an instance of spec/models/roracle-report.model
    /// that satisfies its facts (plan 0093).
    #[test]
    fn aggregate_conforms_to_the_report_model() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let model = rmodel::parse(
            &std::fs::read_to_string(root.join("spec/models/roracle-report.model")).unwrap(),
        )
        .unwrap();
        let alloy = ralloy::Model::parse(&rmodel::to_alloy(&model)).unwrap();
        let all = [
            Status::Matched,
            Status::Mismatch,
            Status::Invalid,
            Status::Unknown,
            Status::Error,
        ];
        let name = |status: Status| format!("{status:?}");
        let slots = ["Slot0", "Slot1", "Slot2"];
        let mut checked = 0;
        for count in 0..=slots.len() {
            for code in 0..all.len().pow(count as u32) {
                let statuses: Vec<Status> = (0..count)
                    .map(|position| all[code / all.len().pow(position as u32) % all.len()])
                    .collect();
                let report = Status::aggregate(statuses.iter().copied());
                let mut signatures = BTreeMap::from([
                    ("World".to_owned(), vec!["World".to_owned()]),
                    ("s_Slot".to_owned(), slots.map(String::from).to_vec()),
                    ("s_Status".to_owned(), all.map(name).to_vec()),
                ]);
                for status in all {
                    signatures.insert(format!("e_{}", name(status)), vec![name(status)]);
                }
                let instance = ralloy::Instance {
                    signatures,
                    fields: BTreeMap::from([
                        (
                            "r_status".to_owned(),
                            statuses
                                .iter()
                                .zip(slots)
                                .map(|(status, slot)| {
                                    vec!["World".into(), slot.into(), name(*status)]
                                })
                                .collect(),
                        ),
                        (
                            "r_report".to_owned(),
                            vec![vec!["World".into(), name(report)]],
                        ),
                    ]),
                    trace: vec![BTreeMap::new()],
                    back: Some(0),
                };
                let checked_instance = alloy.check_instance(0, &instance, 10_000_000).unwrap();
                assert!(
                    !matches!(
                        checked_instance.status,
                        ralloy::CheckStatus::Invalid | ralloy::CheckStatus::Unknown
                    ),
                    "{statuses:?} -> {report:?}: {:?} {:?}",
                    checked_instance.status,
                    checked_instance.reason
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 156);
    }
}
