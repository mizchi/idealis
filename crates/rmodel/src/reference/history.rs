use super::{
    Call, Check, Formula, GuardPlan, Machine, Model, Next, ObservationField, ObservationPairResult,
    Outcome, RawEvent, RawWitness, Scene, Snapshot, bind_values, binding_indices,
    candidate_indices, candidate_values, encode_witness, observation_fields, observation_point,
};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, btree_map::Entry};

type StatePair = [Snapshot; 2];

struct Link<'a> {
    before: StatePair,
    calls: [Option<Call>; 2],
    event: Option<&'a str>,
}

struct Route<'a> {
    total: usize,
    link: Option<Link<'a>>,
}

#[derive(Default)]
struct Frontier<'a> {
    routes: BTreeMap<StatePair, Route<'a>>,
    queue: BinaryHeap<Reverse<(usize, StatePair)>>,
}

impl<'a> Frontier<'a> {
    fn push(&mut self, pair: StatePair, total: usize, link: Option<Link<'a>>) {
        match self.routes.entry(pair.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(Route { total, link });
            }
            Entry::Occupied(mut entry) if total < entry.get().total => {
                entry.insert(Route { total, link });
            }
            Entry::Occupied(_) => return,
        }
        self.queue.push(Reverse((total, pair)));
    }

    fn pop(&mut self) -> Option<(usize, StatePair)> {
        while let Some(Reverse((total, pair))) = self.queue.pop() {
            if self.routes[&pair].total == total {
                return Some((total, pair));
            }
        }
        None
    }
}

#[derive(Default)]
struct Moves<'a> {
    visible: Vec<(Snapshot, Call, Option<&'a str>)>,
    silent: Vec<(Snapshot, Call)>,
}

pub struct ObservationEvent<'a> {
    pub action: &'a str,
    pub event: &'a str,
    pub when: Option<&'a str>,
}

struct Event<'a> {
    name: &'a str,
    when: Option<Formula>,
}

struct Search<'a> {
    machine: &'a Machine<'a>,
    at: &'a Check,
    observe: Vec<ObservationField<'a>>,
    distinguish: Vec<ObservationField<'a>>,
    silent: BTreeSet<usize>,
    events: BTreeMap<usize, Event<'a>>,
    guard_plans: Vec<GuardPlan<'a>>,
    remaining: usize,
    escaped: bool,
    has_initial: bool,
    best: Option<[RawWitness; 2]>,
}

/// Compare the initial observations and the observations after each non-silent operation.
/// The shortest pair minimizes the total number of operations, including silent operations.
pub fn observation_history_pair(
    model: &Model,
    at: &str,
    observe: &[String],
    distinguish: &[String],
    silent: &[String],
    events: &[ObservationEvent<'_>],
    budget: usize,
) -> Result<ObservationPairResult, String> {
    let at = observation_point(model, at)?;
    let observe = observation_fields(model, observe)?;
    let distinguish = observation_fields(model, distinguish)?;
    let silent: BTreeSet<usize> = silent
        .iter()
        .map(|name| {
            model
                .actions
                .iter()
                .position(|action| action.name == *name)
                .ok_or_else(|| format!("unknown silent operation {name}"))
        })
        .collect::<Result<_, _>>()?;
    let mut rules = BTreeMap::new();
    for rule in events {
        let (index, action) = model
            .actions
            .iter()
            .enumerate()
            .find(|(_, action)| action.name == rule.action)
            .ok_or_else(|| format!("unknown emitting operation {}", rule.action))?;
        if silent.contains(&index) || rules.contains_key(&index) || rule.event.trim().is_empty() {
            return Err(format!(
                "operation {} needs one nonempty event rule and cannot also be silent",
                rule.action
            ));
        }
        let when = rule
            .when
            .map(|source| crate::syntax::transition_predicate(model, action, source))
            .transpose()
            .map_err(|error| format!("{} event predicate: {error}", rule.action))?;
        rules.insert(
            index,
            Event {
                name: rule.event,
                when,
            },
        );
    }
    let Ok(machine) = Machine::for_search(model) else {
        return Ok(ObservationPairResult {
            outcome: Outcome::Unknown,
            complete: false,
            pair: None,
        });
    };
    let mut search = Search {
        machine: &machine,
        at,
        observe,
        distinguish,
        silent,
        events: rules,
        guard_plans: model.actions.iter().map(GuardPlan::new).collect(),
        remaining: budget,
        escaped: false,
        has_initial: false,
        best: None,
    };
    let complete = search.walk().is_ok() && !search.escaped;
    let outcome = match &search.best {
        Some(pair) => Outcome::Reachable {
            depth: pair.iter().map(|trace| trace.commands.len()).sum(),
        },
        None if !complete => Outcome::Unknown,
        None if !search.has_initial => Outcome::Empty,
        None => Outcome::Unreachable,
    };
    let pair = search
        .best
        .map(|pair| pair.map(|trace| encode_witness(&machine, trace)));
    Ok(ObservationPairResult {
        outcome,
        complete,
        pair,
    })
}

fn spend(remaining: &mut usize) -> Result<(), ()> {
    *remaining = remaining.checked_sub(1).ok_or(())?;
    Ok(())
}

fn holds(
    machine: &Machine<'_>,
    config: &Snapshot,
    state: &[super::Value],
    formulas: &[Formula],
) -> bool {
    let relations = machine.relations(&config.0, state);
    let scene = Scene {
        relations: &relations,
        after: None,
        bound: &BTreeMap::new(),
    };
    formulas
        .iter()
        .all(|formula| machine.universe.formula(formula, &scene))
}

impl<'a> Search<'a> {
    fn walk(&mut self) -> Result<(), ()> {
        for indices in candidate_indices(&self.machine.static_choices) {
            spend(&mut self.remaining)?;
            let config = Snapshot(candidate_values(
                &self.machine.universe,
                &self.machine.static_choices,
                indices,
            ));
            if !holds(self.machine, &config, &[], &self.machine.model.facts) {
                continue;
            }
            let mut initial = Vec::new();
            for indices in candidate_indices(&self.machine.var_choices) {
                spend(&mut self.remaining)?;
                let state =
                    candidate_values(&self.machine.universe, &self.machine.var_choices, indices);
                if holds(self.machine, &config, &state, &self.machine.model.init) {
                    initial.push(Snapshot(state));
                }
            }
            self.has_initial |= !initial.is_empty();
            self.walk_configuration(&config, &initial)?;
        }
        Ok(())
    }

    fn walk_configuration(&mut self, config: &Snapshot, initial: &[Snapshot]) -> Result<(), ()> {
        let mut frontier = Frontier::default();
        for left in initial {
            for right in initial {
                spend(&mut self.remaining)?;
                if same(self.machine, config, left, right, &self.observe) {
                    frontier.push([left.clone(), right.clone()], 0, None);
                }
            }
        }
        while let Some((total, pair)) = frontier.pop() {
            if self
                .best
                .as_ref()
                .is_none_or(|best| total < best.iter().map(|trace| trace.commands.len()).sum())
                && !same(self.machine, config, &pair[0], &pair[1], &self.distinguish)
                && pair.iter().all(|state| {
                    holds(
                        self.machine,
                        config,
                        &state.0,
                        std::slice::from_ref(&self.at.formula),
                    )
                })
            {
                self.best = Some(record_pair(
                    self.machine,
                    config,
                    &pair,
                    &frontier.routes,
                    &self.observe,
                ));
            }
            let left = self.moves(config, &pair[0])?;
            let right = self.moves(config, &pair[1])?;
            for (side, moves) in [left.silent, right.silent].into_iter().enumerate() {
                for (next, call) in moves {
                    spend(&mut self.remaining)?;
                    let mut successor = pair.clone();
                    successor[side] = next;
                    let mut calls = [None, None];
                    calls[side] = Some(call);
                    frontier.push(
                        successor,
                        total + 1,
                        Some(Link {
                            before: pair.clone(),
                            calls,
                            event: None,
                        }),
                    );
                }
            }
            for (left, left_call, left_event) in &left.visible {
                for (right, right_call, right_event) in &right.visible {
                    spend(&mut self.remaining)?;
                    if left_event != right_event
                        || !same(self.machine, config, left, right, &self.observe)
                    {
                        continue;
                    }
                    // The observations emitted so far agree. Only the current pair
                    // determines the future, even after silent changes to a field.
                    frontier.push(
                        [left.clone(), right.clone()],
                        total + 2,
                        Some(Link {
                            before: pair.clone(),
                            calls: [Some(left_call.clone()), Some(right_call.clone())],
                            event: *left_event,
                        }),
                    );
                }
            }
        }
        Ok(())
    }

    fn moves(&mut self, config: &Snapshot, state: &Snapshot) -> Result<Moves<'a>, ()> {
        let relations = self.machine.relations(&config.0, &state.0);
        let mut moves = Moves::default();
        let universe = &self.machine.universe;
        let before = Scene {
            relations: &relations,
            after: None,
            bound: &BTreeMap::new(),
        };
        for (index, (action, guards)) in self
            .machine
            .model
            .actions
            .iter()
            .zip(&self.guard_plans)
            .enumerate()
        {
            if !guards.before_binding(universe, &before) {
                continue;
            }
            for indices in binding_indices(universe, &action.params) {
                spend(&mut self.remaining)?;
                let bound = bind_values(&self.machine.universe, &action.params, indices);
                let scene = Scene {
                    relations: &relations,
                    after: None,
                    bound: &bound,
                };
                if !guards.after_binding(universe, &scene) {
                    continue;
                }
                match self.machine.advance(action, &scene, state) {
                    Next::State(next) => {
                        let event = self.events.get(&index);
                        let hidden = self.silent.contains(&index)
                            || event
                                .and_then(|event| event.when.as_ref())
                                .is_some_and(|when| {
                                    let after = self.machine.relations(&config.0, &next.0);
                                    !self.machine.universe.formula(
                                        when,
                                        &Scene {
                                            after: Some(&after),
                                            ..scene
                                        },
                                    )
                                });
                        if hidden {
                            moves.silent.push((next, (index, bound)));
                        } else {
                            moves.visible.push((
                                next,
                                (index, bound),
                                event.map(|event| event.name),
                            ));
                        }
                    }
                    Next::Outside(_) => self.escaped = true,
                    Next::Broken(_) => {}
                }
            }
        }
        Ok(moves)
    }
}

fn same(
    machine: &Machine<'_>,
    config: &Snapshot,
    left: &Snapshot,
    right: &Snapshot,
    fields: &[ObservationField<'_>],
) -> bool {
    let mut relations = None;
    let bound = BTreeMap::new();
    fields.iter().all(|field| match *field {
        ObservationField::State(index) => left.0[index] == right.0[index],
        ObservationField::Derived(_, expression) => {
            let (left, right) = relations.get_or_insert_with(|| {
                (
                    machine.relations(&config.0, &left.0),
                    machine.relations(&config.0, &right.0),
                )
            });
            let value = |relations| {
                machine.universe.expression(
                    expression,
                    &Scene {
                        relations,
                        after: None,
                        bound: &bound,
                    },
                )
            };
            value(left) == value(right)
        }
    })
}

fn record_pair(
    machine: &Machine<'_>,
    config: &Snapshot,
    pair: &StatePair,
    routes: &BTreeMap<StatePair, Route<'_>>,
    observe: &[ObservationField<'_>],
) -> [RawWitness; 2] {
    let mut calls = [Vec::new(), Vec::new()];
    let mut events = [Vec::new(), Vec::new()];
    let mut cursor = pair;
    while let Some(link) = &routes[cursor].link {
        for (side, call) in link.calls.iter().enumerate() {
            if let Some(call) = call {
                calls[side].push(call.clone());
                if let Some(event) = link.event {
                    let state = &cursor[side].0;
                    let mut relations = None;
                    let bound = BTreeMap::new();
                    events[side].push(RawEvent {
                        step: calls[side].len(),
                        event: event.to_owned(),
                        observation: observe
                            .iter()
                            .map(|field| match *field {
                                ObservationField::State(index) => {
                                    (machine.vars[index].name.clone(), state[index].clone())
                                }
                                ObservationField::Derived(name, expression) => {
                                    let relations = relations
                                        .get_or_insert_with(|| machine.relations(&config.0, state));
                                    let value = machine.universe.expression(
                                        expression,
                                        &Scene {
                                            relations,
                                            after: None,
                                            bound: &bound,
                                        },
                                    );
                                    (name.to_owned(), value)
                                }
                            })
                            .collect(),
                    });
                }
            }
        }
        cursor = &link.before;
    }
    [0, 1].map(|index| {
        let mut commands = std::mem::take(&mut calls[index]);
        commands.reverse();
        let mut emitted = std::mem::take(&mut events[index]);
        emitted.reverse();
        for event in &mut emitted {
            event.step = commands.len() - event.step + 1;
        }
        RawWitness {
            config: config.clone(),
            initial: cursor[index].clone(),
            commands,
            final_state: pair[index].clone(),
            events: Some(emitted),
        }
    })
}
