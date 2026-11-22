mod cache;
mod parser;
mod protocol;
mod script;
mod session;
mod solver;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use cache::{CacheOptions, CacheStats, CheckCache, IncrementalCheck, IncrementalScript};
use num_bigint::BigInt;
use num_traits::Zero;
pub use protocol::Engine;
pub use script::Script;
use serde::{Deserialize, Serialize};
pub use solver::Solver;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug, Serialize)]
pub struct Error {
    pub code: String,
    pub message: String,
}
impl Error {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(output, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
struct Edge {
    from: usize,
    to: usize,
    bound: BigInt,
}
#[derive(Clone)]
enum Formula {
    Constant(bool),
    Boolean(usize),
    Edge(Edge),
    Not(Arc<Self>),
    And(Vec<Arc<Self>>),
    Or(Vec<Arc<Self>>),
    Iff(Arc<Self>, Arc<Self>),
}
#[derive(Clone, Copy)]
enum Variable {
    Int(usize),
    Bool(usize),
}
pub struct Problem {
    variables: Arc<parser::Variables>,
    root: Arc<Formula>,
    anchors: Vec<Edge>,
    integers: usize,
    booleans: usize,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Assumption {
    pub symbol: String,
    pub positive: bool,
}
#[derive(Default, Serialize)]
pub struct Stats {
    work: u32,
    branches: u64,
    relaxations: u32,
}
impl Stats {
    fn tick(&mut self, limit: u32) -> std::result::Result<(), ()> {
        if self.work == limit {
            return Err(());
        }
        self.work += 1;
        Ok(())
    }
}
#[derive(Serialize)]
pub struct Solution {
    pub status: String,
    pub values: BTreeMap<String, String>,
    pub booleans: BTreeMap<String, bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub stats: Stats,
}
#[derive(Clone)]
struct State {
    todo: Vec<(Arc<Formula>, bool)>,
    edges: Vec<Edge>,
    booleans: Vec<Option<bool>>,
}

fn distances(
    nodes: usize,
    edges: &[Edge],
    stats: &mut Stats,
    limit: u32,
) -> std::result::Result<Option<Vec<BigInt>>, ()> {
    let mut values = vec![BigInt::zero(); nodes];
    for pass in 0..nodes {
        let mut changed = false;
        for edge in edges {
            stats.tick(limit)?;
            stats.relaxations += 1;
            let candidate = &values[edge.from] + &edge.bound;
            if candidate < values[edge.to] {
                values[edge.to] = candidate;
                changed = true;
            }
        }
        if !changed {
            return Ok(Some(values));
        }
        if pass + 1 == nodes {
            return Ok(None);
        }
    }
    Ok(Some(values))
}

impl Problem {
    pub fn parse(source: &str) -> Result<Self> {
        parser::compile(source)
    }
    pub fn solve(&self, max_work: u32) -> Solution {
        self.solve_with(max_work, vec![(self.root.clone(), true)])
    }
    pub fn solve_assuming(&self, assumptions: &[Assumption], max_work: u32) -> Result<Solution> {
        Ok(self.solve_with(max_work, self.assumption_todo(assumptions)?))
    }
    fn assumption_todo(&self, assumptions: &[Assumption]) -> Result<Vec<(Arc<Formula>, bool)>> {
        if assumptions.len() > 256 {
            return Err(Error::new("resource", "assumptions exceed 256"));
        }
        let mut todo = vec![(self.root.clone(), true)];
        for assumption in assumptions {
            match self.variables.get(&assumption.symbol) {
                Some((Variable::Bool(slot), _)) => {
                    todo.push((Arc::new(Formula::Boolean(*slot)), assumption.positive))
                }
                Some(_) => return Err(Error::new("type", "assumption needs a Bool symbol")),
                None => return Err(Error::new("type", "undeclared assumption symbol")),
            }
        }
        Ok(todo)
    }
    fn solve_with(&self, max_work: u32, todo: Vec<(Arc<Formula>, bool)>) -> Solution {
        let mut stats = Stats::default();
        let mut stack = vec![State {
            todo,
            edges: self.anchors.clone(),
            booleans: vec![None; self.booleans],
        }];
        'states: while let Some(mut state) = stack.pop() {
            while let Some((formula, positive)) = state.todo.pop() {
                if stats.tick(max_work).is_err() {
                    return Self::finished("unknown", stats, Some("work"));
                }
                match formula.as_ref() {
                    Formula::Constant(value) => {
                        if *value != positive {
                            continue 'states;
                        }
                    }
                    Formula::Boolean(slot) => {
                        if state.booleans[*slot].is_some_and(|value| value != positive) {
                            continue 'states;
                        }
                        state.booleans[*slot] = Some(positive);
                    }
                    Formula::Edge(edge) => state.edges.push(if positive {
                        edge.clone()
                    } else {
                        Edge {
                            from: edge.to,
                            to: edge.from,
                            bound: -&edge.bound - 1,
                        }
                    }),
                    Formula::Not(inner) => state.todo.push((inner.clone(), !positive)),
                    Formula::And(children) | Formula::Or(children) => {
                        let conjunction = matches!(formula.as_ref(), Formula::And(_)) == positive;
                        if conjunction {
                            for child in children.iter().rev() {
                                state.todo.push((child.clone(), positive));
                            }
                        } else {
                            let Some(first) = children.first() else {
                                continue 'states;
                            };
                            for child in children[1..].iter().rev() {
                                let mut alternative = state.clone();
                                alternative.todo.push((child.clone(), positive));
                                stack.push(alternative);
                                stats.branches += 1;
                            }
                            state.todo.push((first.clone(), positive));
                        }
                    }
                    Formula::Iff(left, right) => {
                        let mut alternative = state.clone();
                        alternative
                            .todo
                            .extend([(right.clone(), !positive), (left.clone(), false)]);
                        stack.push(alternative);
                        stats.branches += 1;
                        state
                            .todo
                            .extend([(right.clone(), positive), (left.clone(), true)]);
                    }
                }
            }
            let values = match distances(self.integers + 1, &state.edges, &mut stats, max_work) {
                Err(()) => return Self::finished("unknown", stats, Some("work")),
                Ok(None) => continue,
                Ok(Some(values)) => values,
            };
            let mut solution = Self::finished("sat", stats, None);
            for (name, (variable, _)) in self.variables.iter() {
                match variable {
                    Variable::Int(slot) => {
                        solution
                            .values
                            .insert(name.clone(), (&values[*slot] - &values[0]).to_string());
                    }
                    Variable::Bool(slot) => {
                        solution
                            .booleans
                            .insert(name.clone(), state.booleans[*slot].unwrap_or(false));
                    }
                }
            }
            return solution;
        }
        Self::finished("unsat", stats, None)
    }
    fn finished(status: &str, stats: Stats, reason: Option<&str>) -> Solution {
        Solution {
            status: status.into(),
            values: BTreeMap::new(),
            booleans: BTreeMap::new(),
            reason: reason.map(str::to_owned),
            stats,
        }
    }
}
