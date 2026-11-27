//! One entry point for an rmodel model's questions (plan 0101). Each checker
//! declares, per question, whether it answers completely, within a bound, or
//! not at all; answers use the glossary's words, and `auto` prefers a
//! complete answer.
use serde::{Deserialize, Serialize};

pub mod project;

/// A question a model asks: one of its checks, or the built-in multiplicity
/// check `Declarations` (plan 0099).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    pub name: String,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Temporal,
    Transition,
    Invariant,
    Reachable,
}

/// How far a checker's answer reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Coverage {
    /// Every reachable state of every configuration.
    Complete,
    /// Traces with at most `steps` states, including the initial state.
    Bounded { steps: usize },
}

/// Whether a checker answers a question, and how far.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Support {
    Yes(Coverage),
    No(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Holds,
    Violated,
    Reachable,
    Unreachable,
    /// No initial state: not a pass.
    Empty,
    /// An incomplete search could not establish a verdict.
    Unknown,
    /// The checker failed; never read as any other verdict.
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Answer {
    pub question: String,
    pub checker: &'static str,
    pub verdict: Verdict,
    pub coverage: Coverage,
    /// The shortest depth of a violation or a reachable state, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
    /// The checker's temporal counterexample, if it found a cycle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lasso: Option<serde_json::Value>,
}

/// Why a search could not cover the question's declared range.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Diagnostic {
    Budget {
        resource: String,
    },
    ScopeExit {
        #[serde(skip_serializing_if = "Option::is_none")]
        depth: Option<usize>,
    },
    DeclarationViolation {
        depth: usize,
    },
    EnumerationLimit {
        relation: String,
    },
    ArgumentEnumerationLimit {
        action: String,
        sort: String,
    },
}

impl Diagnostic {
    fn budget(resource: impl Into<String>) -> Self {
        Self::Budget {
            resource: resource.into(),
        }
    }
}

/// Translate reference coverage information without inferring causes from a verdict.
pub fn reference_diagnostics(
    scope: rmodel::reference::Outcome,
    stop: Option<&rmodel::reference::StopReason>,
) -> Vec<Diagnostic> {
    use rmodel::reference::{Outcome, StopReason};
    let mut diagnostics = Vec::new();
    if let Outcome::Violated { depth } = scope {
        diagnostics.push(Diagnostic::ScopeExit { depth: Some(depth) });
    }
    match stop {
        Some(StopReason::Budget) => diagnostics.push(Diagnostic::budget("states")),
        Some(StopReason::ArgumentBudget) => diagnostics.push(Diagnostic::budget("arguments")),
        Some(StopReason::EnumerationLimit { relation }) => {
            diagnostics.push(Diagnostic::EnumerationLimit {
                relation: relation.clone(),
            });
        }
        Some(StopReason::ArgumentEnumerationLimit { action, sort }) => {
            diagnostics.push(Diagnostic::ArgumentEnumerationLimit {
                action: action.clone(),
                sort: sort.clone(),
            });
        }
        None => {}
    }
    diagnostics
}

impl Answer {
    /// A witness, or a negative answer over the complete space.
    pub fn definite(&self) -> bool {
        match self.verdict {
            Verdict::Violated | Verdict::Reachable => true,
            Verdict::Holds | Verdict::Unreachable | Verdict::Empty => {
                self.coverage == Coverage::Complete
            }
            Verdict::Unknown | Verdict::Error => false,
        }
    }
}

/// Budgets shared by the checkers.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    pub states: usize,
    pub work: u64,
    pub sat: u64,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            states: 2_000_000,
            work: 500_000_000,
            sat: 50_000_000,
        }
    }
}

pub trait Checker {
    fn name(&self) -> &'static str;
    fn supports(&self, model: &rmodel::Model, question: &Question) -> Support;
    /// Answers a question the checker supports.
    fn answer(&self, model: &rmodel::Model, question: &Question, budget: Budget) -> Answer;
}

/// The model's questions, its checks first and `Declarations` last.
pub fn questions(model: &rmodel::Model) -> Vec<Question> {
    model
        .checks
        .iter()
        .map(|check| Question {
            name: check.name.clone(),
            kind: match check.kind {
                rmodel::CheckKind::Invariant => Kind::Invariant,
                rmodel::CheckKind::Reachable => Kind::Reachable,
            },
        })
        .chain(model.temporal.iter().map(|check| Question {
            name: check.name.clone(),
            kind: Kind::Temporal,
        }))
        .chain(model.transitions.iter().map(|check| Question {
            name: check.name.clone(),
            kind: Kind::Transition,
        }))
        .chain(std::iter::once(Question {
            name: rmodel::DECLARATIONS.to_owned(),
            kind: Kind::Invariant,
        }))
        .collect()
}

/// The registered checkers, in `auto`'s order of preference.
pub fn checkers() -> Vec<Box<dyn Checker>> {
    vec![
        Box::new(checkers::Reference),
        Box::new(checkers::Explicit),
        Box::new(checkers::Sat),
        Box::new(checkers::Rprob),
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    /// The first checker with a complete answer, else the first with any.
    Auto,
    Named(String),
    All,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    NoQuestion(String),
    NoChecker(String),
    DuplicateChecker(&'static str),
    /// No checker answers the question; each one's reason.
    Unsupported(String, Vec<(&'static str, String)>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoQuestion(name) => write!(f, "no question named {name}"),
            Error::NoChecker(name) => write!(f, "no checker named {name}"),
            Error::DuplicateChecker(name) => write!(f, "duplicate checker {name}"),
            Error::Unsupported(name, reasons) => {
                write!(f, "no checker answers {name}:")?;
                for (checker, reason) in reasons {
                    write!(f, " {checker}: {reason};")?;
                }
                Ok(())
            }
        }
    }
}

/// The answers to one question, and whether two definite answers disagree.
#[derive(Debug, Serialize)]
pub struct Report {
    pub question: String,
    pub answers: Vec<Answer>,
    pub disagreement: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unsupported: Vec<UnsupportedChecker>,
}

#[derive(Debug, Serialize)]
pub struct UnsupportedChecker {
    pub checker: &'static str,
    pub reason: String,
}

pub fn check(
    model: &rmodel::Model,
    name: &str,
    selection: &Selection,
    budget: Budget,
) -> Result<Report, Error> {
    let checkers = checkers();
    let registered: Vec<_> = checkers.iter().map(Box::as_ref).collect();
    check_with(model, name, selection, budget, &registered)
}

/// Check a question with an ordered list of caller-supplied checkers.
pub fn check_with(
    model: &rmodel::Model,
    name: &str,
    selection: &Selection,
    budget: Budget,
    registered: &[&dyn Checker],
) -> Result<Report, Error> {
    let question = questions(model)
        .into_iter()
        .find(|question| question.name == name)
        .ok_or_else(|| Error::NoQuestion(name.to_owned()))?;
    let mut names = std::collections::BTreeSet::new();
    for checker in registered {
        if !names.insert(checker.name()) {
            return Err(Error::DuplicateChecker(checker.name()));
        }
    }
    let candidates: Vec<&dyn Checker> = match selection {
        Selection::Named(wanted) => vec![
            registered
                .iter()
                .copied()
                .find(|checker| checker.name() == wanted)
                .ok_or_else(|| Error::NoChecker(wanted.clone()))?,
        ],
        _ => registered.to_vec(),
    };
    let mut reasons = Vec::new();
    let mut supported = Vec::new();
    for checker in candidates {
        match checker.supports(model, &question) {
            Support::Yes(coverage) => supported.push((checker, coverage)),
            Support::No(reason) => reasons.push((checker.name(), reason)),
        }
    }
    if supported.is_empty() {
        return Err(Error::Unsupported(question.name, reasons));
    }
    let chosen = match selection {
        Selection::Auto => {
            let complete = supported
                .iter()
                .find(|(_, coverage)| *coverage == Coverage::Complete);
            vec![*complete.unwrap_or(&supported[0])]
        }
        _ => supported,
    };
    let answers: Vec<Answer> = chosen
        .iter()
        .map(|&(checker, coverage)| {
            let answer = checker.answer(model, &question, budget);
            validate_answer(checker, &question, coverage, answer)
        })
        .collect();
    let disagreement = answers.iter().enumerate().any(|(i, a)| {
        answers[i + 1..]
            .iter()
            .any(|b| disagree(a, b, model.scope.steps))
    });
    Ok(Report {
        question: question.name,
        answers,
        disagreement,
        unsupported: reasons
            .into_iter()
            .map(|(checker, reason)| UnsupportedChecker { checker, reason })
            .collect(),
    })
}

fn validate_answer(
    checker: &dyn Checker,
    question: &Question,
    coverage: Coverage,
    answer: Answer,
) -> Answer {
    let problem = if answer.question != question.name || answer.checker != checker.name() {
        "answer identifies a different checker or question"
    } else if answer.coverage != coverage {
        "answer coverage differs from declared support"
    } else if match answer.verdict {
        Verdict::Reachable | Verdict::Unreachable => question.kind != Kind::Reachable,
        Verdict::Holds | Verdict::Violated => question.kind == Kind::Reachable,
        _ => false,
    } {
        "answer verdict does not fit the question"
    } else if let Coverage::Bounded { steps } = coverage
        && matches!(answer.verdict, Verdict::Reachable | Verdict::Violated)
        && answer.depth.is_some_and(|depth| depth >= steps)
    {
        "witness depth lies outside the declared coverage"
    } else {
        return answer;
    };
    Answer {
        question: question.name.clone(),
        checker: checker.name(),
        verdict: Verdict::Error,
        coverage,
        depth: None,
        detail: Some(problem.into()),
        diagnostics: Vec::new(),
        lasso: None,
    }
}

/// Two answers contradict each other. A bounded negative answer only claims
/// there is no witness shallower than the bound, so a deeper witness from a
/// complete search does not contradict it.
pub fn disagree(a: &Answer, b: &Answer, _model_steps: usize) -> bool {
    let decided = |answer: &Answer| !matches!(answer.verdict, Verdict::Unknown | Verdict::Error);
    if !decided(a) || !decided(b) || a.verdict == b.verdict {
        return false;
    }
    for (bounded, other) in [(a, b), (b, a)] {
        if let Coverage::Bounded { steps } = bounded.coverage
            && matches!(bounded.verdict, Verdict::Holds | Verdict::Unreachable)
        {
            if other.depth.is_some_and(|depth| depth >= steps) {
                return false;
            }
            // A run with no instance cannot tell empty from unreachable.
            if bounded.verdict == Verdict::Unreachable && other.verdict == Verdict::Empty {
                return false;
            }
        }
    }
    true
}

pub mod checkers;
pub mod conform;
pub mod ir;
pub mod schema;
