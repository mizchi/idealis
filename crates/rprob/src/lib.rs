//! Explicit exploration of finite rmodel sets and binary relations.
//! Lowering is independent of the reference evaluator and can be reused
//! across runs. The reference report types retain the same coverage semantics.

mod bits;
use bits::{Candidates, Cursor};
mod compile;
mod frontier;
mod indexed;
mod indexed_rows;
mod indexed_sets;
mod sequences;
mod sparse;
mod temporal;
mod values;
mod witness;
pub use temporal::{TemporalOutcome, TemporalWitness, temporal_support, temporal_witness};
pub use witness::{witness, witness_support, witness_with_budget};

use compile::{Environment, Predicate, Term};
use frontier::{BitmapLayout, Frontier};
use idealis_semantics::reference::{Outcome, Report, SearchBudget, StopReason, Verdict};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported(String);

impl Unsupported {
    fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Unsupported {}

/// Analysis of state and transition questions, with temporal questions named separately.
pub struct SafetyAnalysis {
    pub report: Report,
    pub omitted_temporal: Vec<String>,
}

/// Check the finite state graph without evaluating temporal properties or fairness.
pub fn analyze_safety(
    model: &idealis_syntax::Model,
    max_states: usize,
) -> Result<SafetyAnalysis, Unsupported> {
    Ok(SafetyAnalysis {
        report: compile::lower_safety(model)?.analyze(max_states),
        omitted_temporal: model
            .temporal
            .iter()
            .map(|check| check.name.clone())
            .collect(),
    })
}

/// Check whether the model can be lowered for finite safety exploration.
pub fn safety_support(model: &idealis_syntax::Model) -> Result<(), Unsupported> {
    compile::lower_safety(model).map(|_| ())
}

struct Action {
    state_guard: bool,
    guard: Predicate,
    assignments: Vec<(usize, Term)>,
    word: Option<WordUpdate>,
    transitions: Vec<(usize, Predicate)>,
}

struct ActionGroup {
    name: String,
    instances: Vec<Action>,
    toggle: Option<Toggle>,
    parameterized: Option<ParameterizedAction>,
}

struct ParameterizedAction {
    before_binding: Predicate,
    action: Action,
    domains: Vec<Candidates>,
    count: usize,
}

struct Toggle {
    mask: u64,
    set: bool,
}

#[derive(Clone, Copy)]
struct WordTest {
    mask: u64,
    expected: u64,
    negated: bool,
}

#[derive(Clone, Copy)]
struct WordUpdate {
    guard: Option<WordTest>,
    keep: u64,
    add: u64,
}

enum ActiveAction<'a> {
    Word(WordUpdate),
    General {
        guard: Option<&'a Predicate>,
        assignments: &'a [(usize, Term)],
    },
}

enum Step {
    Disabled,
    Invalid,
    ScopeExit,
    Admitted,
}

impl ActiveAction<'_> {
    fn step<const TRACE: bool>(
        &self,
        env: Environment<'_>,
        domain: &[bits::Domain],
        checked_storage: bool,
        next: &mut [u64],
    ) -> Step {
        match self {
            Self::Word(update) => {
                let state = env.before[0];
                if update
                    .guard
                    .is_some_and(|test| ((state & test.mask) == test.expected) == test.negated)
                {
                    return Step::Disabled;
                }
                next[0] = (state & update.keep) | update.add;
            }
            Self::General { guard, assignments } => {
                if guard.is_some_and(|guard| !guard.evaluate(env)) {
                    return Step::Disabled;
                }
                // Assignments are unique and read the same before-state.
                if assignments.len() != env.before.len() {
                    next.copy_from_slice(env.before);
                }
                let mut invalid = false;
                for (index, expr) in *assignments {
                    let value = if checked_storage {
                        let Some(value) = expr.evaluate_write(env) else {
                            return Step::ScopeExit;
                        };
                        value
                    } else {
                        expr.evaluate(env)
                    };
                    let domain = &domain[*index];
                    if domain.integer_mask.is_some_and(|mask| value & !mask != 0) {
                        return Step::ScopeExit;
                    }
                    if !domain.admits(value) {
                        if !TRACE && !checked_storage {
                            return Step::Invalid;
                        }
                        invalid = true;
                    }
                    next[*index] = value;
                }
                if invalid {
                    return Step::Invalid;
                }
            }
        }
        Step::Admitted
    }
}

struct Check {
    name: String,
    expects_true: bool,
    predicate: Predicate,
}

struct Initial {
    count: Option<usize>,
    search: InitialSearch,
}

enum InitialSearch {
    Fixed(Option<Vec<u64>>),
    Enumerated(Vec<Candidates>),
}

/// A prepared model. Sets use one bit per atom. Binary relations use
/// one bit per ordered pair in an eight by eight matrix.
pub struct Compiled {
    config: Vec<Candidates>,
    state: Vec<bits::Domain>,
    initial: Initial,
    bitmap: Option<BitmapLayout>,
    facts: Predicate,
    init: Predicate,
    actions: Vec<ActionGroup>,
    checks: Vec<Check>,
    transitions: Vec<String>,
    global_transitions: Vec<(usize, Predicate)>,
    temporal: Option<temporal::Question>,
    checked_storage: bool,
}

impl Compiled {
    /// Lower a model produced by rmodel's checked parser.
    pub fn new(model: &idealis_syntax::Model) -> Result<Self, Unsupported> {
        if !model.temporal.is_empty() {
            return Err(Unsupported::new(
                "temporal obligations are outside the finite atom fragment",
            ));
        }
        compile::lower(model)
    }

    /// Same work budget as the reference evaluator: count configurations,
    /// initial candidates and reached states. Breadth-first order retains
    /// the shortest witness depth within the exploration.
    pub fn analyze(&self, max_states: usize) -> Report {
        self.analyze_with::<false, false>(
            SearchBudget::for_states(max_states),
            &mut witness::Search::default(),
            None,
        )
    }

    fn analyze_with<const TRACE: bool, const GRAPH: bool>(
        &self,
        budget: SearchBudget,
        search: &mut witness::Search,
        mut graphs: Option<&mut temporal::Graphs>,
    ) -> Report {
        let mut remaining = budget.states;
        let mut complete = true;
        let mut stop = StopReason::Budget;
        let mut arguments_left = budget.arguments;
        let mut initial = false;
        let mut broken = None;
        let mut escaped = None;
        let mut witnesses = vec![None; self.checks.len()];
        let mut transition_witnesses = vec![None; self.transitions.len()];
        let mut enabled = vec![false; self.actions.len()];
        let mut next = vec![0; self.state.len()];
        let mut active: Vec<Vec<_>> = self.actions.iter().map(|_| Vec::new()).collect();
        let track_calls = TRACE || GRAPH;
        let mut active_indices: Vec<Vec<usize>> = if track_calls {
            self.actions.iter().map(|_| Vec::new()).collect()
        } else {
            Vec::new()
        };
        let mut configs = Combinations::new(&self.config);
        'configurations: while let Some(config) = configs.next() {
            if !consume(&mut remaining) {
                complete = false;
                break;
            }
            let env = Environment {
                config,
                before: &[],
                after: &[],
                arguments: &[],
            };
            if !self.facts.evaluate(env) {
                continue;
            }
            if TRACE {
                search.parents.clear();
            }
            // Initial candidate enumeration always finishes before visiting
            // any initial state. An overflowing count cannot fit any budget.
            let Some(budget) = self
                .initial
                .count
                .and_then(|count| remaining.checked_sub(count))
            else {
                complete = false;
                break 'configurations;
            };
            let mut frontier = Frontier::new(self.state.len(), self.bitmap.as_ref(), remaining);
            let mut graph = GRAPH.then(|| graphs.as_mut().expect("requested graph").start(config));
            remaining = budget;
            match &self.initial.search {
                InitialSearch::Fixed(state) => {
                    if let Some(state) = state
                        && self.init.evaluate(Environment {
                            before: state,
                            ..env
                        })
                    {
                        initial = true;
                        frontier.push(state, 0);
                        if let Some(graph) = &mut graph {
                            graph.initial(state);
                        }
                    }
                }
                InitialSearch::Enumerated(choices) => {
                    let mut candidates = Combinations::new(choices);
                    while let Some(candidate) = candidates.next() {
                        if self.init.evaluate(Environment {
                            before: candidate,
                            ..env
                        }) {
                            initial = true;
                            frontier.push(candidate, 0);
                            if let Some(graph) = &mut graph {
                                graph.initial(candidate);
                            }
                        }
                    }
                }
            }
            let Some(first) = frontier.pop() else {
                continue;
            };
            if remaining == 0 {
                complete = false;
                break 'configurations;
            }
            for (group_index, (group, active)) in self.actions.iter().zip(&mut active).enumerate() {
                if !track_calls && group.toggle.is_some() {
                    continue;
                }
                active.clear();
                if track_calls {
                    active_indices[group_index].clear();
                }
                for (instance, action) in group.instances.iter().enumerate() {
                    if action.state_guard || action.guard.evaluate(env) {
                        let step = match action.word {
                            Some(word) => ActiveAction::Word(word),
                            None => ActiveAction::General {
                                guard: action.state_guard.then_some(&action.guard),
                                assignments: &action.assignments,
                            },
                        };
                        active.push((step, action.transitions.as_slice()));
                        if track_calls {
                            active_indices[group_index].push(instance);
                        }
                    }
                }
            }
            let mut pending = Some(first);
            while let Some((snapshot, depth)) = pending.take().or_else(|| frontier.pop()) {
                let state = snapshot.words();
                if !consume(&mut remaining) {
                    complete = false;
                    break 'configurations;
                }
                let env = Environment {
                    before: state,
                    ..env
                };
                observe(&self.checks, &mut witnesses, depth, env, |index| {
                    if TRACE {
                        search.record(witness::Target::State(index), depth, config, state, None);
                    }
                });
                let mut enqueue = |next: &[u64],
                                   scoped: &[(usize, Predicate)],
                                   call,
                                   env: Environment<'_>,
                                   search: &mut witness::Search| {
                    let step = Environment { after: next, ..env };
                    for (index, predicate) in self.global_transitions.iter().chain(scoped) {
                        let witness = &mut transition_witnesses[*index];
                        if witness.is_none_or(|known| depth + 1 < known)
                            && !predicate.evaluate(step)
                        {
                            *witness = Some(depth + 1);
                            if TRACE {
                                search.record(
                                    witness::Target::Transition(*index),
                                    depth + 1,
                                    config,
                                    state,
                                    Some(witness::Edge::Encoded(call, next)),
                                );
                            }
                        }
                    }
                    if let Some(graph) = &mut graph {
                        graph.edge(state, next, call);
                    }
                    if frontier.push(next, depth + 1) && TRACE {
                        search.parents.insert(
                            next.into(),
                            witness::Link {
                                before: state.into(),
                                call,
                            },
                        );
                    }
                };
                for (group_index, ((group, instances), action_enabled)) in self
                    .actions
                    .iter()
                    .zip(&active)
                    .zip(&mut enabled)
                    .enumerate()
                {
                    if !track_calls && let Some(toggle) = &group.toggle {
                        let state = state[0];
                        let mut eligible = toggle.mask & if toggle.set { !state } else { state };
                        *action_enabled |= eligible != 0;
                        while eligible != 0 {
                            let remaining_bits = eligible & (eligible - 1);
                            enqueue(
                                &[state ^ (eligible ^ remaining_bits)],
                                &[],
                                (0, 0),
                                env,
                                search,
                            );
                            eligible = remaining_bits;
                        }
                        continue;
                    }
                    let mut try_step =
                        |action: &ActiveAction<'_>,
                         scoped: &[(usize, Predicate)],
                         call,
                         env: Environment<'_>| {
                            match action.step::<TRACE>(
                                env,
                                &self.state,
                                self.checked_storage,
                                &mut next,
                            ) {
                                Step::Disabled => return,
                                Step::ScopeExit => {
                                    *action_enabled = true;
                                    remember(&mut escaped, depth + 1);
                                    if TRACE {
                                        search.record(
                                            witness::Target::Scope,
                                            depth + 1,
                                            config,
                                            state,
                                            Some(witness::Edge::ScopeExit(call)),
                                        );
                                    }
                                    return;
                                }
                                Step::Invalid => {
                                    *action_enabled = true;
                                    remember(&mut broken, depth + 1);
                                    if TRACE {
                                        search.record(
                                            witness::Target::Declarations,
                                            depth + 1,
                                            config,
                                            state,
                                            Some(witness::Edge::Encoded(call, &next)),
                                        );
                                    }
                                    return;
                                }
                                Step::Admitted => *action_enabled = true,
                            }
                            enqueue(&next, scoped, call, env, search);
                        };
                    if let Some(plan) = &group.parameterized {
                        if !plan.before_binding.evaluate(env) {
                            continue;
                        }
                        let action = ActiveAction::General {
                            guard: Some(&plan.action.guard),
                            assignments: &plan.action.assignments,
                        };
                        let mut choices = Combinations::new(&plan.domains);
                        let mut instance = 0;
                        while let Some(arguments) = choices.next() {
                            let Some(left) = arguments_left.checked_sub(1) else {
                                complete = false;
                                stop = StopReason::ArgumentBudget;
                                break 'configurations;
                            };
                            arguments_left = left;
                            try_step(
                                &action,
                                &plan.action.transitions,
                                (group_index, instance),
                                Environment { arguments, ..env },
                            );
                            instance += 1;
                        }
                        continue;
                    }
                    for (active_index, (action, scoped)) in instances.iter().enumerate() {
                        if GRAPH {
                            let Some(left) = arguments_left.checked_sub(1) else {
                                complete = false;
                                stop = StopReason::ArgumentBudget;
                                break 'configurations;
                            };
                            arguments_left = left;
                        }
                        let call = if track_calls {
                            (group_index, active_indices[group_index][active_index])
                        } else {
                            (0, 0)
                        };
                        try_step(action, scoped, call, env);
                    }
                }
            }
        }

        let coverage = match (complete && escaped.is_none(), initial) {
            (false, _) => Outcome::Unknown,
            (true, false) => Outcome::Empty,
            (true, true) => Outcome::Holds,
        };
        let verdicts = self
            .checks
            .iter()
            .map(|check| (&check.name, check.expects_true))
            .chain(self.transitions.iter().map(|name| (name, false)))
            .zip(witnesses.into_iter().chain(transition_witnesses))
            .map(|((name, expects_true), depth)| {
                let outcome = match (depth, expects_true, coverage) {
                    (Some(depth), true, _) => Outcome::Reachable { depth },
                    (Some(depth), false, _) => Outcome::Violated { depth },
                    (None, true, Outcome::Holds) => Outcome::Unreachable,
                    _ => coverage,
                };
                Verdict {
                    check: name.clone(),
                    outcome,
                }
            })
            .collect();
        Report {
            verdicts,
            declarations: broken.map_or(coverage, |depth| Outcome::Violated { depth }),
            scope: escaped.map_or(coverage, |depth| Outcome::Violated { depth }),
            stop: (!complete).then_some(stop),
            enabled: self
                .actions
                .iter()
                .zip(enabled)
                .map(|(group, found)| {
                    (
                        group.name.clone(),
                        (found || (complete && initial && escaped.is_none())).then_some(found),
                    )
                })
                .collect(),
        }
    }
}

fn consume(remaining: &mut usize) -> bool {
    if let Some(next) = remaining.checked_sub(1) {
        *remaining = next;
        true
    } else {
        false
    }
}

fn remember(witness: &mut Option<usize>, depth: usize) {
    if witness.is_none_or(|known| depth < known) {
        *witness = Some(depth);
    }
}

fn observe(
    checks: &[Check],
    witnesses: &mut [Option<usize>],
    depth: usize,
    env: Environment<'_>,
    mut on_hit: impl FnMut(usize),
) {
    for (index, (check, witness)) in checks.iter().zip(witnesses).enumerate() {
        if witness.is_none_or(|known| depth < known)
            && check.predicate.evaluate(env) == check.expects_true
        {
            *witness = Some(depth);
            on_hit(index);
        }
    }
}

/// Odometer in declaration order. Reuses the same slice each time.
struct Combinations<'a> {
    choices: &'a [Candidates],
    cursors: Vec<Cursor>,
    values: Vec<u64>,
    first: bool,
    done: bool,
}

impl<'a> Combinations<'a> {
    fn new(choices: &'a [Candidates]) -> Self {
        Self {
            choices,
            cursors: vec![Cursor::default(); choices.len()],
            values: choices
                .iter()
                .filter_map(|options| options.get(0))
                .collect(),
            first: true,
            done: choices.iter().any(|options| options.len() == 0),
        }
    }

    fn next(&mut self) -> Option<&[u64]> {
        if self.done {
            return None;
        }
        if self.first {
            self.first = false;
            return Some(&self.values);
        }
        for index in (0..self.cursors.len()).rev() {
            if self.choices[index].advance(&mut self.cursors[index], &mut self.values[index]) {
                return Some(&self.values);
            }
        }
        self.done = true;
        None
    }
}
