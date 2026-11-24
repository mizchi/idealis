//! The registered checkers: the reference evaluator, rtlc on the TLA+
//! lowering and ralloy's SAT on the Alloy lowering.
use crate::{Answer, Budget, Checker, Coverage, Kind, Question, Support, Verdict};

fn answer(
    checker: &'static str,
    question: &Question,
    coverage: Coverage,
    verdict: Verdict,
) -> Answer {
    Answer {
        question: question.name.clone(),
        checker,
        verdict,
        coverage,
        depth: None,
        detail: None,
    }
}

/// The source of truth: exhaustive over configurations and states, when
/// every relation's values can be enumerated.
pub struct Reference;

impl Checker for Reference {
    fn name(&self) -> &'static str {
        "reference"
    }
    fn supports(&self, model: &rmodel::Model, _: &Question) -> Support {
        match rmodel::reference::enumerable(model) {
            Ok(()) => Support::Yes(Coverage::Complete),
            Err(reason) => Support::No(reason),
        }
    }
    fn answer(&self, model: &rmodel::Model, question: &Question, budget: Budget) -> Answer {
        use rmodel::reference::Outcome;
        let report = rmodel::reference::analyze(model, budget.states);
        let outcome = if question.name == rmodel::DECLARATIONS {
            report.declarations
        } else {
            report
                .verdicts
                .iter()
                .find(|verdict| verdict.check == question.name)
                .expect("a question of the model")
                .outcome
        };
        let (verdict, depth) = match outcome {
            Outcome::Empty => (Verdict::Empty, None),
            Outcome::Holds => (Verdict::Holds, None),
            Outcome::Violated { depth } => (Verdict::Violated, Some(depth)),
            Outcome::Reachable { depth } => (Verdict::Reachable, Some(depth)),
            Outcome::Unreachable => (Verdict::Unreachable, None),
            Outcome::Unknown => (Verdict::Unknown, None),
        };
        Answer {
            depth,
            ..answer(self.name(), question, Coverage::Complete, verdict)
        }
    }
}

/// rtlc's explicit-state search over the TLA+ lowering.
pub struct Explicit;

impl Checker for Explicit {
    fn name(&self) -> &'static str {
        "explicit"
    }
    fn supports(&self, _: &rmodel::Model, _: &Question) -> Support {
        Support::Yes(Coverage::Complete)
    }
    fn answer(&self, model: &rmodel::Model, question: &Question, budget: Budget) -> Answer {
        let failed = |detail: String| Answer {
            detail: Some(detail),
            ..answer(self.name(), question, Coverage::Complete, Verdict::Error)
        };
        let (source, config) = match rmodel::to_tla(model, &question.name) {
            Ok(lowered) => lowered,
            Err(error) => return failed(error.to_string()),
        };
        let program = match idealis_core::compile_tla_with_options(
            &source,
            &config,
            idealis_core::CompileOptions {
                irVersion: idealis_core::IrVersion::V2,
            },
        ) {
            Ok(program) => program,
            Err(error) => return failed(error.message),
        };
        let result = match program.check(idealis_core::SafetyBounds {
            max_depth: 1_000,
            max_states: u32::try_from(budget.states).unwrap_or(u32::MAX),
            max_work: u32::try_from(budget.work).unwrap_or(u32::MAX),
            max_temporal_nodes: None,
        }) {
            Ok(result) => result,
            Err(error) => return failed(error.message),
        };
        let found = result.violation.is_some();
        let verdict = match (result.status.as_str(), found, question.kind) {
            (_, true, Kind::Invariant) => Verdict::Violated,
            (_, true, Kind::Reachable) => Verdict::Reachable,
            ("empty", _, _) => Verdict::Empty,
            ("complete", _, Kind::Invariant) => Verdict::Holds,
            ("complete", _, Kind::Reachable) => Verdict::Unreachable,
            _ => Verdict::Unknown,
        };
        Answer {
            depth: found.then_some(result.depth as usize),
            detail: (verdict == Verdict::Unknown).then(|| result.status.clone()),
            ..answer(self.name(), question, Coverage::Complete, verdict)
        }
    }
}

/// ralloy's SAT over the Alloy lowering: traces up to the model's steps.
pub struct Sat;

impl Checker for Sat {
    fn name(&self) -> &'static str {
        "sat"
    }
    fn supports(&self, model: &rmodel::Model, _: &Question) -> Support {
        Support::Yes(Coverage::Bounded {
            steps: model.scope.steps,
        })
    }
    fn answer(&self, model: &rmodel::Model, question: &Question, budget: Budget) -> Answer {
        let coverage = Coverage::Bounded {
            steps: model.scope.steps,
        };
        let failed = |detail: String| Answer {
            detail: Some(detail),
            ..answer(self.name(), question, coverage, Verdict::Error)
        };
        let alloy = match ralloy::Model::parse(&rmodel::to_alloy(model)) {
            Ok(alloy) => alloy,
            Err(error) => return failed(error.to_string()),
        };
        let label = rmodel::alloy_label(&question.name);
        let Some(index) = alloy
            .commands()
            .iter()
            .position(|command| command.label == label)
        else {
            return failed(format!("no Alloy command {label}"));
        };
        let analysis = match alloy.analyze(index, budget.sat) {
            Ok(analysis) => analysis,
            Err(error) => return failed(error.to_string()),
        };
        let verdict = match analysis.status {
            ralloy::Status::Counterexample => Verdict::Violated,
            ralloy::Status::NoCounterexample => Verdict::Holds,
            ralloy::Status::Instance => Verdict::Reachable,
            // A check with no instance at all has unsatisfiable facts or
            // initial states. A run with none cannot tell empty from
            // unreachable; it answers unreachable within the bound.
            ralloy::Status::NoInstance if question.kind == Kind::Invariant => Verdict::Empty,
            ralloy::Status::NoInstance => Verdict::Unreachable,
            ralloy::Status::Unknown => Verdict::Unknown,
        };
        answer(self.name(), question, coverage, verdict)
    }
}
