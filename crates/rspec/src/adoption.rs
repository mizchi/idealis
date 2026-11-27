use crate::{ClauseReport, LinkStatus, ReviewStatus};
use serde::{Deserialize, Serialize};

/// Pin this when scheduling a comparison or measurement. The candidate
/// identifies the implementation artifact, independently of the model and
/// requirement revision supplied by rspec.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct EvaluationRevision {
    pub candidate: String,
    pub rule: String,
    pub evaluation_revision: String,
}

impl EvaluationRevision {
    pub fn for_report(candidate: impl Into<String>, report: &ClauseReport) -> Self {
        Self {
            candidate: candidate.into(),
            rule: report.rule.clone(),
            evaluation_revision: report.evaluation_revision.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Evidence<T> {
    pub revision: EvaluationRevision,
    pub outcome: T,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticComparison {
    Equivalent,
    Diverges,
    Empty,
    Incomplete,
    Missing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GoalMeasurement {
    Improved,
    Unmeasured,
    NoGain,
    Regressed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdoptionBlocker {
    ReviewFromOtherRevision,
    ReviewIncomplete,
    ComparisonFromOtherRevision,
    ComparisonNotEquivalent,
    GoalFromOtherRevision,
    GoalNotImproved,
}

/// Check evidence against the current revision. This checks provenance
/// labels and outcomes; the producers are responsible for obtaining
/// the comparison and measurement and recording their original revision.
pub fn assess_adoption(
    current: &EvaluationRevision,
    review: &Evidence<&ClauseReport>,
    comparison: &Evidence<SemanticComparison>,
    goal: &Evidence<GoalMeasurement>,
) -> Result<(), Vec<AdoptionBlocker>> {
    use AdoptionBlocker::*;

    let mut blockers = Vec::new();
    let report = review.outcome;
    if &review.revision != current
        || report.rule != current.rule
        || report.evaluation_revision != current.evaluation_revision
    {
        blockers.push(ReviewFromOtherRevision);
    } else if report.clauses.is_empty()
        || !report.clauses.iter().all(|clause| {
            clause.link == LinkStatus::Resolved
                && clause.review == ReviewStatus::Reviewed
                && clause
                    .check
                    .as_ref()
                    .is_some_and(|check| check.matches_expectation)
        })
    {
        blockers.push(ReviewIncomplete);
    }
    if &comparison.revision != current {
        blockers.push(ComparisonFromOtherRevision);
    } else if comparison.outcome != SemanticComparison::Equivalent {
        blockers.push(ComparisonNotEquivalent);
    }
    if &goal.revision != current {
        blockers.push(GoalFromOtherRevision);
    } else if goal.outcome != GoalMeasurement::Improved {
        blockers.push(GoalNotImproved);
    }
    if blockers.is_empty() {
        Ok(())
    } else {
        Err(blockers)
    }
}
