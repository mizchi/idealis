//! Find counterexamples to eventually and leads-to over the reachable graph.
use crate::{
    Unsupported, compile,
    compile::{Environment, Predicate},
    witness::{Call, Search},
};
use rmodel::{
    FairnessKind, Model,
    reference::{Outcome, SearchBudget, StopReason, Witness},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;

pub struct TemporalWitness {
    /// Only real operations are stored in the state witness. Each missing
    /// entry in the schedule represents a stuttering edge.
    pub witness: Witness,
    pub stutters: Vec<bool>,
    pub loop_start: usize,
}

pub enum TemporalOutcome {
    Empty,
    Holds,
    Unknown {
        scope: Outcome,
        declarations: Outcome,
        stop: Option<StopReason>,
    },
    Violated(TemporalWitness),
}

pub fn temporal_support(model: &Model, name: &str) -> Result<(), Unsupported> {
    lower(model, name).map(|_| ())
}

pub fn temporal_witness(
    model: &Model,
    name: &str,
    max_states: usize,
) -> Result<TemporalOutcome, Unsupported> {
    let (compiled, layout) = lower(model, name)?;
    let question = compiled
        .temporal
        .as_ref()
        .expect("lowered temporal question");
    let mut graphs = Graphs::default();
    let report = compiled.analyze_with::<false, true>(
        SearchBudget::for_states(max_states),
        &mut Search::default(),
        Some(&mut graphs),
    );
    match (report.scope, report.declarations) {
        (Outcome::Empty, Outcome::Empty) => return Ok(TemporalOutcome::Empty),
        (Outcome::Holds, Outcome::Holds) => {}
        _ => {
            return Ok(TemporalOutcome::Unknown {
                scope: report.scope,
                declarations: report.declarations,
                stop: report.stop,
            });
        }
    }
    for graph in &graphs.configurations {
        for goal in &question.goals {
            if let Some(path) = graph.counterexample(goal, &question.fairness) {
                return Ok(TemporalOutcome::Violated(
                    layout.decode_temporal(model, path)?,
                ));
            }
        }
    }
    Ok(TemporalOutcome::Holds)
}

fn lower<'a>(
    model: &'a Model,
    name: &str,
) -> Result<(crate::Compiled, crate::witness::Layout<'a>), Unsupported> {
    let question = model
        .temporal
        .iter()
        .find(|check| check.name == name)
        .ok_or_else(|| Unsupported::new(format!("no temporal question named {name}")))?;
    compile::lower_temporal(model, question)
}

pub(crate) struct Goal {
    pub(crate) trigger: Option<Predicate>,
    pub(crate) response: Predicate,
}

pub(crate) struct Question {
    pub(crate) goals: Vec<Goal>,
    pub(crate) fairness: Vec<Rule>,
}

pub(crate) struct Rule {
    pub(crate) kind: FairnessKind,
    pub(crate) group: usize,
    pub(crate) instance: Option<usize>,
}

impl Rule {
    fn accepts(&self, (group, instance): Call) -> bool {
        group == self.group && self.instance.is_none_or(|own| own == instance)
    }
}

#[derive(Default)]
pub(crate) struct Graphs {
    configurations: Vec<Graph>,
}

impl Graphs {
    pub(crate) fn start(&mut self, config: &[u64]) -> &mut Graph {
        self.configurations.push(Graph {
            config: config.into(),
            index: BTreeMap::new(),
            nodes: Vec::new(),
        });
        self.configurations.last_mut().expect("just inserted graph")
    }
}

#[derive(Clone, Copy)]
struct Edge {
    to: usize,
    call: Call,
}

struct FairComponent {
    nodes: BTreeSet<usize>,
    stops: Vec<(usize, Option<Edge>)>,
}

type Parent = Option<(usize, Call)>;

#[derive(Default)]
struct Node {
    parent: Parent,
    edges: Vec<Edge>,
    incoming: Vec<usize>,
}

pub(crate) struct Graph {
    config: Vec<u64>,
    index: BTreeMap<Rc<[u64]>, usize>,
    nodes: Vec<(Rc<[u64]>, Node)>,
}

pub(crate) struct Path {
    pub(crate) config: Vec<u64>,
    pub(crate) initial: Vec<u64>,
    pub(crate) commands: Vec<Option<Call>>,
    pub(crate) loop_start: usize,
    pub(crate) final_state: Vec<u64>,
}

impl Graph {
    fn insert(&mut self, state: &[u64], parent: Parent) -> usize {
        if let Some(&id) = self.index.get(state) {
            return id;
        }
        let id = self.nodes.len();
        let state = Rc::from(state);
        self.index.insert(Rc::clone(&state), id);
        self.nodes.push((
            state,
            Node {
                parent,
                ..Node::default()
            },
        ));
        id
    }

    pub(crate) fn initial(&mut self, state: &[u64]) {
        self.insert(state, None);
    }

    pub(crate) fn edge(&mut self, from: &[u64], to: &[u64], call: Call) {
        // The model's stutter edge always exists. Equivalent no-op actions
        // do not make a fairness rule enabled.
        if from == to {
            return;
        }
        let from = self.index[from];
        let to = self.insert(to, Some((from, call)));
        self.nodes[from].1.edges.push(Edge { to, call });
        self.nodes[to].1.incoming.push(from);
    }

    fn counterexample(&self, goal: &Goal, rules: &[Rule]) -> Option<Path> {
        let mut allowed = vec![false; self.nodes.len()];
        let mut starts = Vec::new();
        for (id, (state, node)) in self.nodes.iter().enumerate() {
            let env = Environment {
                config: &self.config,
                before: state,
                after: &[],
                arguments: &[],
            };
            allowed[id] = !goal.response.evaluate(env);
            if allowed[id]
                && goal
                    .trigger
                    .as_ref()
                    .map_or_else(|| node.parent.is_none(), |trigger| trigger.evaluate(env))
            {
                starts.push(id);
            }
        }
        if starts.is_empty() {
            return None;
        }
        let (reachable, suffix) = self.routes(&starts, |id| allowed[id]);
        let FairComponent {
            nodes: component,
            stops,
        } = self.fair_component(&reachable, rules)?;
        let entry = *component.first().expect("non-empty SCC");
        let (trigger, suffix) = route(entry, |id| suffix[id]);
        let (initial, prefix) = route(trigger, |id| self.nodes[id].1.parent);
        let mut commands: Vec<_> = prefix.into_iter().chain(suffix).map(Some).collect();
        let loop_start = commands.len();
        let mut at = entry;
        for (from, edge) in stops {
            self.extend_route(&mut commands, at, from, &component);
            at = from;
            if let Some(edge) = edge {
                commands.push(Some(edge.call));
                at = edge.to;
            }
        }
        self.extend_route(&mut commands, at, entry, &component);
        if commands.len() == loop_start {
            commands.push(None);
        }
        Some(Path {
            config: self.config.clone(),
            initial: self.nodes[initial].0.to_vec(),
            commands,
            loop_start,
            final_state: self.nodes[entry].0.to_vec(),
        })
    }

    fn extend_route(
        &self,
        commands: &mut Vec<Option<Call>>,
        from: usize,
        to: usize,
        component: &BTreeSet<usize>,
    ) {
        if from != to {
            let (_, parents) = self.routes(&[from], |id| component.contains(&id));
            let (root, calls) = route(to, |id| parents[id]);
            assert_eq!(root, from, "strongly connected route");
            commands.extend(calls.into_iter().map(Some));
        }
    }

    fn routes(
        &self,
        starts: &[usize],
        allowed: impl Fn(usize) -> bool,
    ) -> (Vec<bool>, Vec<Parent>) {
        let mut seen = vec![false; self.nodes.len()];
        let mut parents = vec![None; self.nodes.len()];
        let mut queue = VecDeque::from(starts.to_vec());
        for &start in starts {
            seen[start] = true;
        }
        while let Some(from) = queue.pop_front() {
            for edge in &self.nodes[from].1.edges {
                if !seen[edge.to] && allowed(edge.to) {
                    seen[edge.to] = true;
                    parents[edge.to] = Some((from, edge.call));
                    queue.push_back(edge.to);
                }
            }
        }
        (seen, parents)
    }

    fn fair_component(&self, reachable: &[bool], rules: &[Rule]) -> Option<FairComponent> {
        let mut components = self.components(|id| reachable[id]);
        'component: while let Some(component) = components.pop() {
            let mut stops = Vec::new();
            for rule in rules {
                let enabled = |id: usize| {
                    self.nodes[id]
                        .1
                        .edges
                        .iter()
                        .any(|edge| rule.accepts(edge.call))
                };
                let taken = component.iter().find_map(|&id| {
                    self.nodes[id]
                        .1
                        .edges
                        .iter()
                        .find(|edge| component.contains(&edge.to) && rule.accepts(edge.call))
                        .map(|&edge| (id, Some(edge)))
                });
                if let Some(stop) = taken {
                    stops.push(stop);
                } else if rule.kind == FairnessKind::Weak {
                    if let Some(&disabled) = component.iter().find(|&&id| !enabled(id)) {
                        stops.push((disabled, None));
                    } else {
                        continue 'component;
                    }
                } else if component.iter().any(|&id| enabled(id)) {
                    // A fair subcycle cannot visit any of these enabled
                    // states: this component contains no matching edge.
                    components
                        .extend(self.components(|id| component.contains(&id) && !enabled(id)));
                    continue 'component;
                }
            }
            return Some(FairComponent {
                nodes: component,
                stops,
            });
        }
        None
    }

    // Iterative Kosaraju, with each retained singleton admitting stutter.
    fn components(&self, allowed: impl Fn(usize) -> bool) -> Vec<BTreeSet<usize>> {
        let mut seen = vec![false; self.nodes.len()];
        let mut order = Vec::new();
        for root in 0..self.nodes.len() {
            if seen[root] || !allowed(root) {
                continue;
            }
            seen[root] = true;
            let mut stack = vec![(root, self.nodes[root].1.edges.iter())];
            while let Some((from, edges)) = stack.last_mut() {
                match edges.next() {
                    Some(edge) if !seen[edge.to] && allowed(edge.to) => {
                        seen[edge.to] = true;
                        stack.push((edge.to, self.nodes[edge.to].1.edges.iter()));
                    }
                    Some(_) => {}
                    None => {
                        order.push(*from);
                        stack.pop();
                    }
                }
            }
        }
        let mut components = Vec::new();
        for root in order.into_iter().rev() {
            if !seen[root] {
                continue;
            }
            seen[root] = false;
            let mut component = BTreeSet::from([root]);
            let mut stack = vec![root];
            while let Some(to) = stack.pop() {
                for &from in &self.nodes[to].1.incoming {
                    if seen[from] {
                        seen[from] = false;
                        component.insert(from);
                        stack.push(from);
                    }
                }
            }
            components.push(component);
        }
        components
    }
}

fn route(mut end: usize, parent: impl Fn(usize) -> Parent) -> (usize, Vec<Call>) {
    let mut calls = Vec::new();
    while let Some((previous, call)) = parent(end) {
        calls.push(call);
        end = previous;
    }
    calls.reverse();
    (end, calls)
}
