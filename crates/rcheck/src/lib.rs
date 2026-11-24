//! One entry point for an rmodel model's questions (plan 0101). Each checker
//! declares, per question, whether it answers completely, within a bound, or
//! not at all; answers use the glossary's words, and `auto` prefers a
//! complete answer.
use serde::Serialize;

/// A question a model asks: one of its checks, or the built-in multiplicity
/// check `Declarations` (plan 0099).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Question {
    pub name: String,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Invariant,
    Reachable,
}

/// How far a checker's answer reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Coverage {
    /// Every reachable state of every configuration.
    Complete,
    /// Only traces of at most `steps` steps.
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
    /// The budget ran out.
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
#[derive(Clone, Copy, Debug)]
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
    /// No checker answers the question; each one's reason.
    Unsupported(String, Vec<(&'static str, String)>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoQuestion(name) => write!(f, "no question named {name}"),
            Error::NoChecker(name) => write!(f, "no checker named {name}"),
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
}

pub fn check(
    model: &rmodel::Model,
    name: &str,
    selection: &Selection,
    budget: Budget,
) -> Result<Report, Error> {
    let question = questions(model)
        .into_iter()
        .find(|question| question.name == name)
        .ok_or_else(|| Error::NoQuestion(name.to_owned()))?;
    let registered = checkers();
    let candidates: Vec<&dyn Checker> = match selection {
        Selection::Named(wanted) => vec![
            registered
                .iter()
                .find(|checker| checker.name() == wanted)
                .ok_or_else(|| Error::NoChecker(wanted.clone()))?
                .as_ref(),
        ],
        _ => registered.iter().map(|checker| checker.as_ref()).collect(),
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
    let chosen: Vec<&dyn Checker> = match selection {
        Selection::Auto => {
            let complete = supported
                .iter()
                .find(|(_, coverage)| *coverage == Coverage::Complete);
            vec![complete.unwrap_or(&supported[0]).0]
        }
        _ => supported.iter().map(|(checker, _)| *checker).collect(),
    };
    let answers: Vec<Answer> = chosen
        .iter()
        .map(|checker| checker.answer(model, &question, budget))
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
    })
}

/// Two answers contradict each other. A bounded negative answer only claims
/// there is no witness shallower than the bound, so a deeper witness from a
/// complete search does not contradict it.
pub fn disagree(a: &Answer, b: &Answer, steps: usize) -> bool {
    let decided = |answer: &Answer| !matches!(answer.verdict, Verdict::Unknown | Verdict::Error);
    if !decided(a) || !decided(b) || a.verdict == b.verdict {
        return false;
    }
    for (bounded, other) in [(a, b), (b, a)] {
        if matches!(bounded.coverage, Coverage::Bounded { .. })
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
pub mod schema;
