//! Explicit exploration of finite rmodel sets and binary relations.
//! Lowering is independent of the reference evaluator and can be reused
//! across runs. The reference report types retain the same coverage semantics.

mod bits;
use bits::{Candidates, Cursor};
mod compile;
mod frontier;

use compile::{Environment, Predicate, Term};
use frontier::{BitmapLayout, Frontier};
use rmodel::reference::{Outcome, Report, Verdict};
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

struct Action {
    state_guard: bool,
    guard: Predicate,
    assignments: Vec<(usize, Term)>,
    word: Option<WordUpdate>,
}

struct ActionGroup {
    name: String,
    instances: Vec<Action>,
    toggle: Option<Toggle>,
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
    Admitted,
}

impl ActiveAction<'_> {
    fn step(&self, env: Environment<'_>, domain: &[bits::Domain], next: &mut [u64]) -> Step {
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
                for (index, expr) in *assignments {
                    let value = expr.evaluate(env);
                    if !domain[*index].admits(value) {
                        return Step::Invalid;
                    }
                    next[*index] = value;
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
    transitions: Vec<Check>,
}

impl Compiled {
    /// Lower a model produced by rmodel's checked parser.
    pub fn new(model: &rmodel::Model) -> Result<Self, Unsupported> {
        compile::lower(model)
    }

    /// Same work budget as the reference evaluator: count configurations,
    /// initial candidates and reached states. Breadth-first order retains
    /// the shortest witness depth within the exploration.
    pub fn analyze(&self, max_states: usize) -> Report {
        let mut remaining = max_states;
        let mut complete = true;
        let mut initial = false;
        let mut broken = None;
        let mut witnesses = vec![None; self.checks.len()];
        let mut transition_witnesses = vec![None; self.transitions.len()];
        let mut enabled = vec![false; self.actions.len()];
        let mut next = vec![0; self.state.len()];
        let mut active: Vec<Vec<ActiveAction<'_>>> =
            self.actions.iter().map(|_| Vec::new()).collect();
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
            };
            if !self.facts.evaluate(env) {
                continue;
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
            for (group, active) in self.actions.iter().zip(&mut active) {
                if group.toggle.is_some() {
                    continue;
                }
                active.clear();
                for action in &group.instances {
                    if action.state_guard || action.guard.evaluate(env) {
                        active.push(match action.word {
                            Some(word) => ActiveAction::Word(word),
                            None => ActiveAction::General {
                                guard: action.state_guard.then_some(&action.guard),
                                assignments: &action.assignments,
                            },
                        });
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
                observe(&self.checks, &mut witnesses, depth, env);
                let mut enqueue = |next: &[u64]| {
                    observe(
                        &self.transitions,
                        &mut transition_witnesses,
                        depth + 1,
                        Environment { after: next, ..env },
                    );
                    frontier.push(next, depth + 1);
                };
                for ((group, instances), action_enabled) in
                    self.actions.iter().zip(&active).zip(&mut enabled)
                {
                    if let Some(toggle) = &group.toggle {
                        let state = state[0];
                        let mut eligible = toggle.mask & if toggle.set { !state } else { state };
                        *action_enabled |= eligible != 0;
                        while eligible != 0 {
                            let remaining_bits = eligible & (eligible - 1);
                            enqueue(&[state ^ (eligible ^ remaining_bits)]);
                            eligible = remaining_bits;
                        }
                        continue;
                    }
                    for action in instances {
                        match action.step(env, &self.state, &mut next) {
                            Step::Disabled => continue,
                            Step::Invalid => {
                                *action_enabled = true;
                                remember(&mut broken, depth + 1);
                                continue;
                            }
                            Step::Admitted => *action_enabled = true,
                        }
                        enqueue(&next);
                    }
                }
            }
        }

        let coverage = match (complete, initial) {
            (false, _) => Outcome::Unknown,
            (true, false) => Outcome::Empty,
            (true, true) => Outcome::Holds,
        };
        let verdicts = self
            .checks
            .iter()
            .chain(&self.transitions)
            .zip(witnesses.into_iter().chain(transition_witnesses))
            .map(|(check, depth)| {
                let outcome = match (depth, check.expects_true, coverage) {
                    (Some(depth), true, _) => Outcome::Reachable { depth },
                    (Some(depth), false, _) => Outcome::Violated { depth },
                    (None, true, Outcome::Holds) => Outcome::Unreachable,
                    _ => coverage,
                };
                Verdict {
                    check: check.name.clone(),
                    outcome,
                }
            })
            .collect();
        Report {
            verdicts,
            declarations: broken.map_or(coverage, |depth| Outcome::Violated { depth }),
            scope: coverage,
            enabled: self
                .actions
                .iter()
                .zip(enabled)
                .map(|(group, found)| {
                    (
                        group.name.clone(),
                        (found || (complete && initial)).then_some(found),
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

fn observe(checks: &[Check], witnesses: &mut [Option<usize>], depth: usize, env: Environment<'_>) {
    for (check, witness) in checks.iter().zip(witnesses) {
        if witness.is_none_or(|known| depth < known)
            && check.predicate.evaluate(env) == check.expects_true
        {
            *witness = Some(depth);
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
