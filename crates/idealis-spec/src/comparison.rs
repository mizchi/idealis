use crate::{ClauseReport, EvaluationRevision, Evidence, LinkStatus, SemanticComparison};
use rmodel::reference::{Outcome, Report};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(crate) fn source_digest(source: &str) -> String {
    format!("{:x}", Sha256::digest(source))
}

/// The pinned input for both workers. Exactly one linked model is supported.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonPlan {
    pub revision: EvaluationRevision,
    pub model: String,
    pub source_sha256: String,
    pub max_states: usize,
}

impl ComparisonPlan {
    pub fn pin(
        candidate: impl Into<String>,
        report: &ClauseReport,
        max_states: usize,
    ) -> Result<Self, String> {
        if report.clauses.is_empty()
            || report
                .clauses
                .iter()
                .any(|clause| clause.link != LinkStatus::Resolved)
        {
            return Err("comparison requires a rule with resolved clauses".into());
        }
        if report.model_inputs.len() != 1 {
            return Err("comparison requires exactly one linked model".into());
        }
        let (model, digest) = report
            .model_inputs
            .first_key_value()
            .expect("one linked model");
        Ok(Self {
            revision: EvaluationRevision::for_report(candidate, report),
            model: model.to_string(),
            source_sha256: digest.to_string(),
            max_states,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Checker {
    Reference,
    Rprob,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "status")]
enum ResultKind {
    Empty,
    Holds,
    Violated { depth: usize },
    Reachable { depth: usize },
    Unreachable,
    Unknown,
}

impl From<Outcome> for ResultKind {
    fn from(value: Outcome) -> Self {
        match value {
            Outcome::Empty => Self::Empty,
            Outcome::Holds => Self::Holds,
            Outcome::Violated { depth } => Self::Violated { depth },
            Outcome::Reachable { depth } => Self::Reachable { depth },
            Outcome::Unreachable => Self::Unreachable,
            Outcome::Unknown => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Snapshot {
    verdicts: BTreeMap<String, ResultKind>,
    declarations: ResultKind,
    scope: ResultKind,
    enabled: BTreeMap<String, Option<bool>>,
}

impl From<Report> for Snapshot {
    fn from(value: Report) -> Self {
        Self {
            verdicts: value
                .verdicts
                .into_iter()
                .map(|v| (v.check, v.outcome.into()))
                .collect(),
            declarations: value.declarations.into(),
            scope: value.scope.into(),
            enabled: value.enabled.into_iter().collect(),
        }
    }
}

impl Snapshot {
    fn is_conclusive(&self) -> bool {
        !self.verdicts.is_empty()
            && matches!(self.scope, ResultKind::Holds | ResultKind::Empty)
            && !matches!(self.declarations, ResultKind::Unknown)
            && !self
                .verdicts
                .values()
                .any(|status| *status == ResultKind::Unknown)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "value")]
enum ReceiptResult {
    Report(Snapshot),
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckerReceipt {
    pub plan: ComparisonPlan,
    pub checker: Checker,
    result: ReceiptResult,
}

/// Each worker runs its backend on the verified source, using the pinned budget.
pub fn run_checker(
    plan: &ComparisonPlan,
    checker: Checker,
    source: &str,
    evaluate: impl FnOnce(&rmodel::Model, usize) -> Result<Report, String>,
) -> Result<CheckerReceipt, String> {
    if source_digest(source) != plan.source_sha256 {
        return Err("the model source differs from the pinned input".into());
    }
    let model = rmodel::parse(source).map_err(|error| error.to_string())?;
    let result = match evaluate(&model, plan.max_states) {
        Ok(report) => ReceiptResult::Report(report.into()),
        Err(reason) => ReceiptResult::Unsupported(reason),
    };
    Ok(CheckerReceipt {
        plan: plan.clone(),
        checker,
        result,
    })
}

#[derive(Debug, Serialize)]
pub struct ComparisonResult {
    #[serde(flatten)]
    pub evidence: Evidence<SemanticComparison>,
    pub ignored_receipts: usize,
    pub missing_checkers: Vec<Checker>,
    pub conflicting_checkers: Vec<Checker>,
}

/// Combine just the two expected workers. Retries are idempotent only when
/// the worker returns exactly the same normalized report.
pub fn compare_receipts(plan: &ComparisonPlan, receipts: &[CheckerReceipt]) -> ComparisonResult {
    use SemanticComparison::*;
    let mut by_checker = BTreeMap::new();
    let mut conflicts = Vec::new();
    let mut ignored_receipts = 0;
    for receipt in receipts {
        if receipt.plan != *plan {
            ignored_receipts += 1;
            continue;
        }
        match by_checker.entry(receipt.checker) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(&receipt.result);
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                if *entry.get() != &receipt.result && !conflicts.contains(&receipt.checker) {
                    conflicts.push(receipt.checker);
                }
            }
        }
    }
    conflicts.sort();
    let missing_checkers: Vec<_> = [Checker::Reference, Checker::Rprob]
        .into_iter()
        .filter(|checker| !by_checker.contains_key(checker))
        .collect();
    let outcome = match (
        conflicts.is_empty(),
        by_checker.get(&Checker::Reference),
        by_checker.get(&Checker::Rprob),
    ) {
        (false, _, _) => Incomplete,
        (_, None, _) | (_, _, None) => Missing,
        (_, Some(ReceiptResult::Report(left)), Some(ReceiptResult::Report(right))) => {
            if !left.is_conclusive() || !right.is_conclusive() {
                Incomplete
            } else if left != right {
                Diverges
            } else if left.scope == ResultKind::Empty {
                Empty
            } else {
                Equivalent
            }
        }
        _ => Incomplete,
    };
    ComparisonResult {
        evidence: Evidence {
            revision: plan.revision.clone(),
            outcome,
        },
        ignored_receipts,
        missing_checkers,
        conflicting_checkers: conflicts,
    }
}
