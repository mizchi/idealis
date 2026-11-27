//! The registered checkers: reference, TLA+ explicit, Alloy SAT, and rprob.
use crate::{Answer, Budget, Checker, Coverage, Diagnostic, Kind, Question, Support, Verdict};

pub(crate) fn answer(
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
        diagnostics: Vec::new(),
        lasso: None,
    }
}

/// The source of truth: exhaustive over configurations and states, when
/// every relation's values can be enumerated.
pub struct Reference;

impl Checker for Reference {
    fn name(&self) -> &'static str {
        "reference"
    }
    fn supports(&self, model: &rmodel::Model, question: &Question) -> Support {
        if question.kind == Kind::Temporal {
            return Support::No("the reference checker only checks state properties".into());
        }
        match rmodel::reference::enumerable(model) {
            Ok(()) => Support::Yes(Coverage::Complete),
            Err(reason) => Support::No(reason),
        }
    }
    fn answer(&self, model: &rmodel::Model, question: &Question, budget: Budget) -> Answer {
        use rmodel::reference::{SearchBudget, analyze_actions_with_budget};
        let report = analyze_actions_with_budget(model, SearchBudget::for_states(budget.states));
        finite_answer(self.name(), question, report)
    }
}

fn finite_answer(
    checker: &'static str,
    question: &Question,
    report: rmodel::reference::Report,
) -> Answer {
    use rmodel::reference::Outcome;
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
        diagnostics: if verdict == Verdict::Unknown {
            crate::reference_diagnostics(report.scope, report.stop.as_ref())
        } else {
            Vec::new()
        },
        ..answer(checker, question, Coverage::Complete, verdict)
    }
}

/// The rprob finite state explorer and its temporal graph search.
pub struct Rprob;

impl Checker for Rprob {
    fn name(&self) -> &'static str {
        "rprob"
    }

    fn supports(&self, model: &rmodel::Model, question: &Question) -> Support {
        match if question.kind == Kind::Temporal {
            rprob::temporal_support(model, &question.name)
        } else {
            rprob::safety_support(model)
        } {
            Ok(()) => Support::Yes(Coverage::Complete),
            Err(error) => Support::No(error.to_string()),
        }
    }

    fn answer(&self, model: &rmodel::Model, question: &Question, budget: Budget) -> Answer {
        let result = if question.kind == Kind::Temporal {
            crate::conform::rprob_temporal_answer(model, question, budget)
        } else {
            rprob::analyze_safety(model, budget.states)
                .map(|analysis| finite_answer(self.name(), question, analysis.report))
                .map_err(|error| error.to_string())
        };
        result.unwrap_or_else(|detail| Answer {
            detail: Some(detail),
            ..answer(self.name(), question, Coverage::Complete, Verdict::Error)
        })
    }
}

/// rtlc's explicit-state search over the TLA+ lowering.
pub struct Explicit;

impl Checker for Explicit {
    fn name(&self) -> &'static str {
        "explicit"
    }
    fn supports(&self, model: &rmodel::Model, _: &Question) -> Support {
        match rmodel::tla_supports(model) {
            Ok(()) => Support::Yes(Coverage::Complete),
            Err(reason) => Support::No(reason),
        }
    }
    fn answer(&self, model: &rmodel::Model, question: &Question, budget: Budget) -> Answer {
        self.answer_lowered(model, question, budget, || {
            rmodel::to_tla(model, &question.name)
        })
    }
}

impl Explicit {
    pub(crate) fn answer_lowered(
        &self,
        model: &rmodel::Model,
        question: &Question,
        budget: Budget,
        lower: impl FnOnce() -> Result<(String, String), rmodel::Error>,
    ) -> Answer {
        if budget.states == 0 || budget.work == 0 {
            let diagnostics = [(budget.states == 0, "states"), (budget.work == 0, "work")]
                .into_iter()
                .filter(|(exhausted, _)| *exhausted)
                .map(|(_, resource)| Diagnostic::budget(resource))
                .collect();
            return Answer {
                diagnostics,
                ..answer(self.name(), question, Coverage::Complete, Verdict::Unknown)
            };
        }
        let failed = |detail: String| Answer {
            detail: Some(detail),
            ..answer(self.name(), question, Coverage::Complete, Verdict::Error)
        };
        if question.kind == Kind::Temporal {
            for check in [rmodel::DECLARATIONS, rmodel::SCOPE] {
                let verdict = self.answer(
                    model,
                    &Question {
                        name: check.into(),
                        kind: Kind::Invariant,
                    },
                    budget,
                );
                if verdict.verdict != Verdict::Holds {
                    let diagnostics =
                        if check == rmodel::SCOPE && verdict.verdict == Verdict::Violated {
                            vec![Diagnostic::ScopeExit {
                                depth: verdict.depth,
                            }]
                        } else {
                            verdict.diagnostics
                        };
                    return Answer {
                        detail: Some(format!("{check}: {:?}", verdict.verdict)),
                        diagnostics,
                        ..answer(
                            self.name(),
                            question,
                            Coverage::Complete,
                            if verdict.verdict == Verdict::Empty {
                                Verdict::Empty
                            } else {
                                Verdict::Unknown
                            },
                        )
                    };
                }
            }
        }
        let (source, config) = match lower() {
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
            max_temporal_nodes: (question.kind == Kind::Temporal)
                .then_some(u32::try_from(budget.states).unwrap_or(u32::MAX)),
        }) {
            Ok(result) => result,
            Err(error) => return failed(error.message),
        };
        let found = result.violation.is_some();
        let mut verdict = match (result.status.as_str(), found, question.kind) {
            (_, true, Kind::Invariant | Kind::Temporal | Kind::Transition) => Verdict::Violated,
            (_, true, Kind::Reachable) => Verdict::Reachable,
            ("empty", _, _) => Verdict::Empty,
            ("complete", _, Kind::Invariant | Kind::Temporal | Kind::Transition) => Verdict::Holds,
            ("complete", _, Kind::Reachable) => Verdict::Unreachable,
            _ => Verdict::Unknown,
        };
        let mut detail = (verdict == Verdict::Unknown).then(|| result.status.clone());
        let mut diagnostics = Vec::new();
        if verdict == Verdict::Unknown
            && let Some(reason) = result.reason
        {
            diagnostics.push(Diagnostic::budget(reason));
        }
        // A negative answer covers only the states within the integer scope:
        // when a state leaves it, the search is not complete (plan 0103).
        if model.has_values()
            && question.name != rmodel::SCOPE
            && matches!(verdict, Verdict::Holds | Verdict::Unreachable)
        {
            let scope = Question {
                name: rmodel::SCOPE.to_owned(),
                kind: Kind::Invariant,
            };
            let left = self.answer(model, &scope, budget);
            if left.verdict != Verdict::Holds {
                verdict = Verdict::Unknown;
                diagnostics = if left.verdict == Verdict::Violated {
                    vec![Diagnostic::ScopeExit { depth: left.depth }]
                } else {
                    left.diagnostics
                };
                detail = Some(match left.verdict {
                    Verdict::Violated => "a state leaves the Int scope".to_owned(),
                    other => format!("the Int scope check is {other:?}"),
                });
            }
        }
        let lasso = result
            .violation
            .filter(|trace| question.kind == Kind::Temporal && trace.loop_start.is_some())
            .map(|trace| serde_json::to_value(trace).expect("serializable trace"));
        Answer {
            lasso,
            depth: (found && question.kind != Kind::Temporal).then_some(result.depth as usize),
            detail,
            diagnostics,
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
    fn supports(&self, model: &rmodel::Model, question: &Question) -> Support {
        if question.kind == Kind::Temporal {
            return Support::No("the SAT lowering does not compile temporal properties".into());
        }
        let transitions = rmodel::alloy_transition_supports(model);
        let emit_transitions = transitions.is_ok();
        let support = if question.kind == Kind::Transition {
            transitions
        } else {
            rmodel::alloy_supports(model)
        };
        if let Err(reason) = support {
            return Support::No(reason);
        }

        // The lowering emits World, all named sorts, and a signature for
        // each enum member. Only emitted questions count as commands.
        let counts = ralloy::DeclarationCounts {
            signatures: 1
                + model.sigs.len()
                + model.sums.len()
                + model
                    .enums
                    .iter()
                    .map(|(_, members)| 1 + members.len())
                    .sum::<usize>(),
            fields: model.relations.len(),
            predicates_and_assertions: model.actions.len(),
            commands: 1
                + model.checks.len()
                + if emit_transitions {
                    model.transitions.len()
                } else {
                    0
                },
        };
        if let Err(error) = counts.check_limits() {
            return Support::No(error.message);
        }
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
            ralloy::Status::NoInstance
                if matches!(question.kind, Kind::Invariant | Kind::Transition) =>
            {
                Verdict::Empty
            }
            ralloy::Status::NoInstance => Verdict::Unreachable,
            ralloy::Status::Unknown => Verdict::Unknown,
        };
        let diagnostics = if verdict == Verdict::Unknown {
            vec![Diagnostic::budget("work")]
        } else {
            Vec::new()
        };
        Answer {
            diagnostics,
            ..answer(self.name(), question, coverage, verdict)
        }
    }
}
