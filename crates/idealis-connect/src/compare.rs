use crate::{Error, canonical, text};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComparisonStatus {
    Matched,
    Mismatch,
    Invalid,
    Unknown,
    Error,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Observation {
    Allowed { values: Vec<Value> },
    Value { value: Value },
    Invalid { reason: String },
    Unknown { reason: String },
    Error { message: String },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub id: String,
    pub outcome: Observation,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Comparison {
    pub id: String,
    pub status: ComparisonStatus,
    pub expected: Vec<Value>,
    pub actual: Option<Value>,
    pub reason: Option<String>,
    pub outcome: Observation,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Differential {
    pub status: ComparisonStatus,
    pub oracle: Observation,
    pub comparisons: Vec<Comparison>,
}
fn unsuccessful(outcome: &Observation) -> Option<(ComparisonStatus, &str)> {
    match outcome {
        Observation::Invalid { reason } => Some((ComparisonStatus::Invalid, reason)),
        Observation::Unknown { reason } => Some((ComparisonStatus::Unknown, reason)),
        Observation::Error { message } => Some((ComparisonStatus::Error, message)),
        _ => None,
    }
}
pub fn compare(oracle: Observation, targets: Vec<Target>) -> Result<Differential, Error> {
    if targets.is_empty()
        || targets.len() > 256
        || targets
            .iter()
            .map(|target| &target.id)
            .collect::<BTreeSet<_>>()
            .len()
            != targets.len()
    {
        return Err(Error::protocol(
            "targets must have nonempty unique identities",
        ));
    }
    let expected = match &oracle {
        Observation::Allowed { values } => {
            if values.len() > 1024 {
                return Err(Error::resource("oracle exceeds 1024 allowed values"));
            }
            values
                .iter()
                .map(|value| canonical(value, 0))
                .collect::<Result<Vec<_>, _>>()?
        }
        Observation::Value { .. } => {
            return Err(Error::protocol("oracle must provide allowed values"));
        }
        _ => vec![],
    };
    if let Some((_, reason)) = unsuccessful(&oracle) {
        text(reason)?;
    }
    let mut comparisons = Vec::new();
    for target in targets {
        text(&target.id)?;
        if matches!(target.outcome, Observation::Allowed { .. }) {
            return Err(Error::protocol("target must provide a single observation"));
        }
        if let Some((_, reason)) = unsuccessful(&target.outcome) {
            text(reason)?;
        }
        let actual = if let Observation::Value { value } = &target.outcome {
            Some(canonical(value, 0)?)
        } else {
            None
        };
        let failure = if matches!(target.outcome, Observation::Error { .. }) {
            unsuccessful(&target.outcome)
        } else {
            unsuccessful(&oracle).or_else(|| unsuccessful(&target.outcome))
        };
        let (status, reason) = if let Some((status, reason)) = failure {
            (status, Some(reason.to_owned()))
        } else if expected.is_empty() {
            (ComparisonStatus::Invalid, Some("empty-oracle".into()))
        } else if actual
            .as_ref()
            .is_some_and(|actual| expected.contains(actual))
        {
            (ComparisonStatus::Matched, None)
        } else {
            (ComparisonStatus::Mismatch, None)
        };
        comparisons.push(Comparison {
            id: target.id,
            status,
            expected: expected.clone(),
            actual,
            reason,
            outcome: target.outcome,
        });
    }
    let status = [
        ComparisonStatus::Error,
        ComparisonStatus::Mismatch,
        ComparisonStatus::Unknown,
        ComparisonStatus::Invalid,
    ]
    .into_iter()
    .find(|status| {
        comparisons
            .iter()
            .any(|comparison| comparison.status == *status)
    })
    .unwrap_or(ComparisonStatus::Matched);
    Ok(Differential {
        status,
        oracle,
        comparisons,
    })
}
