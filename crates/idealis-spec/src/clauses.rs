use crate::{Claim, Rule, Spec, Text};
use rmodel::reference::{Outcome, Verdict};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewStatus {
    Unavailable,
    Unreviewed,
    NeedsReview,
    Reviewed,
}

#[derive(Debug, Serialize)]
pub struct ModelCheck {
    pub model: String,
    pub check: String,
    pub expected: String,
    pub outcome: &'static str,
    pub depth: Option<usize>,
    pub matches_expectation: bool,
}

impl ModelCheck {
    fn new(claim: &Claim, outcome: Outcome) -> Self {
        let (outcome, depth) = match outcome {
            Outcome::Empty => ("empty", None),
            Outcome::Unknown => ("unknown", None),
            Outcome::Holds => ("holds", None),
            Outcome::Unreachable => ("unreachable", None),
            Outcome::Violated { depth } => ("violated", Some(depth)),
            Outcome::Reachable { depth } => ("reachable", Some(depth)),
        };
        Self {
            model: claim.model.clone(),
            check: claim.check.clone(),
            expected: claim.expect.clone(),
            outcome,
            depth,
            matches_expectation: outcome == claim.expect,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkStatus {
    Unlinked,
    Invalid,
    Resolved,
}

#[derive(Debug, Serialize)]
pub struct ClauseEvidence {
    pub id: String,
    pub text: Text,
    pub link: LinkStatus,
    pub review: ReviewStatus,
    pub revision: Option<String>,
    pub reviewed_revision: Option<String>,
    pub check: Option<ModelCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ClauseReport {
    pub rule: String,
    pub evaluation_revision: String,
    pub model_inputs: BTreeMap<String, String>,
    pub attention: Vec<String>,
    pub clauses: Vec<ClauseEvidence>,
}

#[derive(Clone, Copy)]
pub(crate) enum ClaimKind {
    Invariant,
    Reachable,
    Transition,
}

impl ClaimKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Invariant => "Invariant",
            Self::Reachable => "Reachable",
            Self::Transition => "Transition",
        }
    }

    pub(crate) fn accepts(self, expectation: &str) -> bool {
        match self {
            Self::Reachable => matches!(expectation, "reachable" | "unreachable"),
            Self::Invariant | Self::Transition => matches!(expectation, "holds" | "violated"),
        }
    }
}

pub(crate) fn lookup_check<'m>(
    model: &'m rmodel::Model,
    name: &str,
) -> Option<(ClaimKind, &'m rmodel::Formula)> {
    if let Some(check) = model.checks.iter().find(|check| check.name == name) {
        let kind = match check.kind {
            rmodel::CheckKind::Invariant => ClaimKind::Invariant,
            rmodel::CheckKind::Reachable => ClaimKind::Reachable,
        };
        return Some((kind, &check.formula));
    }
    model
        .transitions
        .iter()
        .find(|check| check.name == name)
        .map(|check| (ClaimKind::Transition, &check.formula))
}

struct EvaluatedModel {
    source_digest: String,
    model: rmodel::Model,
    verdicts: Vec<Verdict>,
}

impl EvaluatedModel {
    fn load(path: &Path, max_states: usize) -> Result<Self, String> {
        let source = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let model = rmodel::parse(&source).map_err(|error| error.to_string())?;
        let verdicts = rmodel::reference::evaluate(&model, max_states);
        Ok(Self {
            source_digest: crate::comparison::source_digest(&source),
            model,
            verdicts,
        })
    }

    fn claim(&self, claim: &Claim) -> Result<(&'static str, &rmodel::Formula, Outcome), String> {
        let (kind, formula) = lookup_check(&self.model, &claim.check)
            .ok_or_else(|| format!("{}: no supported check {}", claim.model, claim.check))?;
        if !kind.accepts(&claim.expect) {
            return Err(format!(
                "{}: invalid expectation {} for {}",
                claim.check,
                claim.expect,
                kind.name()
            ));
        }
        let outcome = self
            .verdicts
            .iter()
            .find(|verdict| verdict.check == claim.check)
            .map_or(Outcome::Unknown, |verdict| verdict.outcome);
        Ok((kind.name(), formula, outcome))
    }
}

/// Experimental correspondence key. The version identifies the current
/// AST representation. Changes to that representation can require review.
/// Runtime edits are handled by re-checking the model on every report.
fn revision(
    spec: &Spec,
    rule: &Rule,
    clause: &crate::Clause,
    claim: &Claim,
    kind: &str,
    formula: &rmodel::Formula,
    model: &rmodel::Model,
) -> String {
    let terms = rule
        .terms
        .iter()
        .map(|id| {
            let Some(term) = spec.terms.iter().find(|term| term.id == *id) else {
                return json!({"missing": id});
            };
            json!({
                "id": id, "kind": term.kind, "name": term.name, "definition": term.definition,
                "aliases": term.aliases, "avoid": term.avoid, "model": term.model,
            })
        })
        .collect::<Vec<_>>();
    let declarations = format!(
        "{:?}",
        (
            &model.name,
            &model.sigs,
            &model.enums,
            &model.sums,
            &model.records,
            &model.sequences,
            &model.variants,
            &model.relations,
            &model.entity_definitions,
            &model.facts,
            &model.scope,
        )
    );
    let payload = json!([
        "rspec-clause-v1",
        spec.id,
        rule.id,
        rule.text,
        clause.id,
        clause.text,
        terms,
        claim.model,
        claim.check,
        claim.expect,
        kind,
        format!("{formula:?}"),
        declarations,
    ]);
    let bytes = serde_json::to_vec(&payload).expect("JSON values serialize");
    format!("v1:{:x}", Sha256::digest(bytes))
}

/// Report explicit correspondence reviews and freshly evaluated model
/// evidence. An unavailable clause does not suppress the other clauses.
pub fn clause_report(
    spec: &Spec,
    root: &Path,
    rule_id: &str,
    max_states: usize,
) -> Result<ClauseReport, String> {
    let rule = spec
        .rules
        .iter()
        .find(|rule| rule.id == rule_id)
        .ok_or_else(|| format!("unknown rule {rule_id}"))?;
    if rule.clauses.is_empty() {
        return Err(format!("{rule_id}: no clauses"));
    }
    let mut ids = BTreeSet::new();
    for clause in &rule.clauses {
        if clause.id.is_empty() || !ids.insert(clause.id.as_str()) {
            return Err(format!(
                "{rule_id}: empty or duplicate clause id {:?}",
                clause.id
            ));
        }
    }
    let mut models = BTreeMap::new();
    let mut clauses = Vec::new();
    let mut attention = Vec::new();
    for clause in &rule.clauses {
        let evidence = clause
            .claim
            .as_ref()
            .ok_or_else(|| "no model claim".to_string())
            .and_then(|claim| {
                let evaluated = models
                    .entry(claim.model.as_str())
                    .or_insert_with(|| EvaluatedModel::load(&root.join(&claim.model), max_states));
                let evaluated = evaluated
                    .as_ref()
                    .map_err(|error| format!("{}: {error}", claim.model))?;
                let (kind, formula, outcome) = evaluated.claim(claim)?;
                let key = revision(spec, rule, clause, claim, kind, formula, &evaluated.model);
                Ok((key, ModelCheck::new(claim, outcome)))
            });
        let (link, review, revision, check, diagnostic) = match evidence {
            Ok((key, check)) => {
                let review = match clause.reviewed.as_deref() {
                    None => ReviewStatus::Unreviewed,
                    Some(previous) if previous == key => ReviewStatus::Reviewed,
                    Some(_) => ReviewStatus::NeedsReview,
                };
                (LinkStatus::Resolved, review, Some(key), Some(check), None)
            }
            Err(error) => {
                let link = if clause.claim.is_some() {
                    LinkStatus::Invalid
                } else {
                    LinkStatus::Unlinked
                };
                (link, ReviewStatus::Unavailable, None, None, Some(error))
            }
        };
        if review != ReviewStatus::Reviewed
            || !check
                .as_ref()
                .is_some_and(|result| result.matches_expectation)
        {
            attention.push(clause.id.clone());
        }
        clauses.push(ClauseEvidence {
            id: clause.id.clone(),
            text: clause.text.clone(),
            link,
            review,
            revision,
            reviewed_revision: clause.reviewed.clone(),
            check,
            diagnostic,
        });
    }
    // The run key covers the full model behavior. A correspondence key can
    // remain current when an operation changes, but results from that run
    // must not be attributed to the edited model.
    let model_sources: Vec<_> = models
        .iter()
        .map(|(path, entry)| {
            [
                path.to_string(),
                entry
                    .as_ref()
                    .map_or_else(|_| String::new(), |entry| format!("{:?}", entry.model)),
            ]
        })
        .collect();
    let clause_keys: Vec<_> = clauses
        .iter()
        .map(|clause| (&clause.id, &clause.revision))
        .collect();
    let key = json!([
        "rspec-evaluation-v1",
        spec.id,
        rule.id,
        clause_keys,
        model_sources
    ]);
    let evaluation_revision = format!("v1:{:x}", Sha256::digest(key.to_string()));
    let model_inputs = models
        .into_iter()
        .filter_map(|(path, entry)| {
            entry
                .ok()
                .map(|entry| (path.to_string(), entry.source_digest))
        })
        .collect();
    Ok(ClauseReport {
        rule: rule.id.clone(),
        evaluation_revision,
        model_inputs,
        attention,
        clauses,
    })
}
