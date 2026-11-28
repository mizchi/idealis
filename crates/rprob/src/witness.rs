use crate::{Unsupported, bits, compile, indexed, indexed_rows, sequences, sparse, values};
use idealis_semantics::reference::{
    Data, Next, Outcome, Runtime, SearchBudget, Snapshot, TraceCommand, Witness, WitnessResult,
};
use idealis_syntax::{Model, Shape, ValueType};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    State(usize),
    Transition(usize),
    Declarations,
    Scope,
}

#[derive(Default)]
pub(crate) struct Search {
    pub(crate) target: Option<Target>,
    pub(crate) parents: BTreeMap<Box<[u64]>, Link>,
    found: Option<Path>,
}

pub(crate) type Call = (usize, usize);

pub(crate) enum Edge<'a> {
    Encoded(Call, &'a [u64]),
    ScopeExit(Call),
}

enum FinalState {
    Encoded(Vec<u64>),
    ScopeExit,
}

pub(crate) struct Link {
    pub(crate) before: Box<[u64]>,
    pub(crate) call: Call,
}

struct Path {
    config: Vec<u64>,
    initial: Vec<u64>,
    commands: Vec<Call>,
    end: FinalState,
}

impl Search {
    pub(crate) fn record(
        &mut self,
        target: Target,
        depth: usize,
        config: &[u64],
        state: &[u64],
        edge: Option<Edge<'_>>,
    ) {
        if self.target != Some(target)
            || self
                .found
                .as_ref()
                .is_some_and(|path| path.commands.len() <= depth)
        {
            return;
        }
        let mut commands = Vec::with_capacity(depth);
        let end = match edge {
            Some(Edge::Encoded(call, next)) => {
                commands.push(call);
                FinalState::Encoded(next.to_vec())
            }
            Some(Edge::ScopeExit(call)) => {
                commands.push(call);
                FinalState::ScopeExit
            }
            None => FinalState::Encoded(state.to_vec()),
        };
        let mut initial = state;
        while let Some(link) = self.parents.get(initial) {
            commands.push(link.call);
            initial = &link.before;
        }
        commands.reverse();
        debug_assert_eq!(commands.len(), depth);
        self.found = Some(Path {
            config: config.to_vec(),
            initial: initial.to_vec(),
            commands,
            end,
        });
    }
}

pub(crate) struct Layout<'a> {
    pub(crate) refined: BTreeMap<&'a str, std::sync::Arc<[u64]>>,
    pub(crate) sorts: BTreeMap<&'a str, Vec<u64>>,
    pub(crate) sparse: Option<sparse::Tuples>,
    pub(crate) records: values::Records<'a>,
    pub(crate) cases: values::Cases<'a>,
    pub(crate) sequences: BTreeMap<&'a str, Arc<sequences::Layout>>,
    pub(crate) indexed: BTreeMap<&'a str, Arc<indexed::Layout>>,
    pub(crate) indexed_rows: BTreeMap<&'a str, Arc<indexed_rows::Layout>>,
}

impl Layout<'_> {
    pub(crate) fn decode_temporal(
        &self,
        model: &Model,
        path: crate::temporal::Path,
    ) -> Result<crate::TemporalWitness, Unsupported> {
        let stutters = path.commands.iter().map(Option::is_none).collect();
        let witness = self.decode(
            model,
            Path {
                config: path.config,
                initial: path.initial,
                commands: path.commands.into_iter().flatten().collect(),
                end: FinalState::Encoded(path.final_state),
            },
        )?;
        Ok(crate::TemporalWitness {
            witness,
            stutters,
            loop_start: path.loop_start,
        })
    }

    fn decode(&self, model: &Model, path: Path) -> Result<Witness, Unsupported> {
        let mut atoms: Vec<_> = model
            .sigs
            .iter()
            .flat_map(|sig| (0..model.scope.sizes[sig]).map(move |index| format!("{sig}{index}")))
            .chain(
                model
                    .enums
                    .iter()
                    .flat_map(|(_, members)| members.iter().cloned()),
            )
            .map(Data::Atom)
            .chain(
                model
                    .scope
                    .ints
                    .into_iter()
                    .flat_map(|(low, high)| (low..=high).map(Data::Int)),
            )
            .collect();
        for ty in model
            .value_types_in_dependency_order()
            .map_err(|error| Unsupported::new(error.message))?
        {
            match ty {
                ValueType::Record(spec) => {
                    if self.indexed.contains_key(spec.name.as_str()) {
                        continue;
                    }
                    let (_, layout) = &self.records[spec.name.as_str()];
                    for rank in 0..layout.values.len() {
                        let fields = spec
                            .fields
                            .iter()
                            .enumerate()
                            .map(|(index, (name, _))| {
                                let value = layout.field(rank, index);
                                (name.clone(), atoms[value.trailing_zeros() as usize].clone())
                            })
                            .collect();
                        atoms.push(Data::Record(spec.name.clone(), fields));
                    }
                }
                ValueType::Variant(spec) => {
                    for case in &spec.cases {
                        if case.payload.is_none() {
                            atoms.push(Data::Variant(case.tag.clone(), None));
                        } else {
                            for &(_, payload) in &self.cases[case.tag.as_str()].payloads {
                                let payload = atoms[payload.trailing_zeros() as usize].clone();
                                atoms
                                    .push(Data::Variant(case.tag.clone(), Some(Box::new(payload))));
                            }
                        }
                    }
                }
                ValueType::Sequence(spec) => {
                    if self.indexed.contains_key(spec.name.as_str()) {
                        continue;
                    }
                    let layout = &self.sequences[spec.name.as_str()];
                    for &bit in &layout.values {
                        let items = layout
                            .decode(bit)
                            .into_iter()
                            .map(|item| atoms[item.trailing_zeros() as usize].clone())
                            .collect();
                        atoms.push(Data::Sequence(items));
                    }
                }
            }
        }
        let atom = |mask: u64| atoms[mask.trailing_zeros() as usize].clone();
        let relations = |is_var, words: &[u64]| {
            model
                .relations
                .iter()
                .filter(|r| r.var == is_var)
                .zip(words)
                .map(|(relation, &word)| {
                    if let Shape::Binary { to, .. } = &relation.shape
                        && let Some(rows) = self.indexed_rows.get(relation.name.as_str())
                    {
                        let layout = &self.indexed[to.as_str()];
                        let mut tuples: Vec<_> = rows
                            .values(word)
                            .map(|(key, value)| {
                                vec![atom(key), layout.decode((value - 1) as usize, &atoms)]
                            })
                            .collect();
                        tuples.sort();
                        return (relation.name.clone(), tuples);
                    }
                    if let Shape::Unary { sig, .. } = &relation.shape
                        && let Some(layout) = self.indexed.get(sig.as_str())
                    {
                        let tuples = word
                            .checked_sub(1)
                            .map(|rank| vec![layout.decode(rank as usize, &atoms)])
                            .into_iter()
                            .collect();
                        return (relation.name.clone(), tuples);
                    }
                    let mut tuples = bits::indices(word)
                        .map(|bit| match relation.shape {
                            Shape::Unary { .. } => vec![atoms[bit].clone()],
                            Shape::Binary { .. } => {
                                let (from, to) = self.sparse.as_ref().map_or_else(
                                    || (1 << (bit / bits::ROW), 1 << (bit % bits::ROW)),
                                    |tuples| tuples.pair(bit),
                                );
                                vec![atom(from), atom(to)]
                            }
                        })
                        .collect::<Vec<_>>();
                    tuples.sort();
                    (relation.name.clone(), tuples)
                })
                .collect()
        };
        let commands = path
            .commands
            .into_iter()
            .map(|(action, mut instance)| {
                let action = &model.actions[action];
                let mut args = action
                    .params
                    .iter()
                    .rev()
                    .map(|(param, sort)| {
                        let (size, value) = if let Some(layout) = self.indexed.get(sort.as_str()) {
                            let allowed = self.refined.get(sort.as_str());
                            let size = allowed.map_or(layout.count, |codes| codes.len());
                            let ordinal = instance % size;
                            let rank =
                                allowed.map_or(ordinal, |codes| (codes[ordinal] - 1) as usize);
                            (size, layout.decode(rank, &atoms))
                        } else {
                            let domain = self.refined.get(sort.as_str()).map_or_else(
                                || self.sorts[sort.as_str()].as_slice(),
                                AsRef::as_ref,
                            );
                            (domain.len(), atom(domain[instance % domain.len()]))
                        };
                        instance /= size;
                        (param.clone(), value)
                    })
                    .collect::<Vec<_>>();
                args.reverse();
                TraceCommand {
                    action: action.name.clone(),
                    args,
                }
            })
            .collect();
        let mut witness = Witness {
            config: relations(false, &path.config),
            initial: relations(true, &path.initial),
            commands,
            final_state: match &path.end {
                FinalState::Encoded(state) => relations(true, state),
                FinalState::ScopeExit => Vec::new(),
            },
            events: None,
        };
        if matches!(path.end, FinalState::ScopeExit) {
            recover_scope_exit(model, &mut witness)?;
        }
        Ok(witness)
    }
}

// The search fixes the path. Replay it without enumerating any alternative
// state, because the escaped successor cannot be encoded in the bitset.
fn recover_scope_exit(model: &Model, witness: &mut Witness) -> Result<(), Unsupported> {
    let runtime = Runtime::new(model);
    let read = |rows: &[(String, Vec<Vec<Data>>)], name: &str| {
        rows.iter()
            .find(|(key, _)| key == name)
            .map(|(_, tuples)| tuples.clone())
    };
    let config = runtime
        .read_configuration(|relation| read(&witness.config, &relation.name))
        .ok_or_else(|| Unsupported::new("rprob scope witness configuration failed replay"))?;
    let mut state = runtime
        .read_initial(&config, |relation| read(&witness.initial, &relation.name))
        .ok_or_else(|| Unsupported::new("rprob scope witness initial state failed replay"))?;
    let execute = |state: &Snapshot, step: &TraceCommand| {
        let command = runtime.read_command(&step.action, |param, _| {
            step.args
                .iter()
                .find(|(name, _)| name == param)
                .map(|(_, value)| value.clone())
        })?;
        runtime.execute(&config, state, &command)
    };
    let (last, prefix) = witness
        .commands
        .split_last()
        .ok_or_else(|| Unsupported::new("rprob scope witness has no final operation"))?;
    for step in prefix {
        let Some(Next::State(next)) = execute(&state, step) else {
            return Err(Unsupported::new(format!(
                "rprob scope witness prefix failed replay at {}",
                step.action
            )));
        };
        state = next;
    }
    let Some(Next::Outside(outside)) = execute(&state, last) else {
        return Err(Unsupported::new(format!(
            "rprob scope witness final operation {} did not leave scope in replay",
            last.action
        )));
    };
    witness.final_state = runtime.state_data(&outside);
    Ok(())
}

/// Check whether the model can be prepared for finite witness search.
/// This lowers the model without exploring its reachable states.
pub fn witness_support(model: &Model) -> Result<(), Unsupported> {
    compile::lower_for_witness(model, false).map(|_| ())
}

/// Find a finite state, transition, or declaration/scope violation witness.
/// Temporal questions and fairness do not restrict this safety exploration.
pub fn witness(
    model: &Model,
    question: &str,
    max_states: usize,
) -> Result<WitnessResult, Unsupported> {
    search_witness(model, question, SearchBudget::for_states(max_states), false)
}

/// Find a finite witness with explicit state and argument trial budgets.
pub fn witness_with_budget(
    model: &Model,
    question: &str,
    budget: SearchBudget,
) -> Result<WitnessResult, Unsupported> {
    search_witness(model, question, budget, true)
}

fn search_witness(
    model: &Model,
    question: &str,
    budget: SearchBudget,
    force_argument_search: bool,
) -> Result<WitnessResult, Unsupported> {
    let target = if question == idealis_syntax::DECLARATIONS {
        Target::Declarations
    } else if question == idealis_syntax::SCOPE {
        Target::Scope
    } else if let Some(index) = model.checks.iter().position(|check| check.name == question) {
        Target::State(index)
    } else if let Some(index) = model
        .transitions
        .iter()
        .position(|check| check.name == question)
    {
        Target::Transition(index)
    } else {
        return Err(Unsupported::new(format!(
            "no finite state or transition question named {question}"
        )));
    };
    let (compiled, layout) = compile::lower_for_witness(model, force_argument_search)?;
    let mut search = Search {
        target: Some(target),
        ..Search::default()
    };
    let report = compiled.analyze_with::<true, false>(budget, &mut search, None);
    let outcome = match target {
        Target::State(index) => report.verdicts[index].outcome,
        Target::Transition(index) => report.verdicts[model.checks.len() + index].outcome,
        Target::Declarations => report.declarations,
        Target::Scope => report.scope,
    };
    Ok(WitnessResult {
        outcome,
        complete: matches!(report.scope, Outcome::Holds | Outcome::Empty),
        scope: report.scope,
        declarations: report.declarations,
        stop: report.stop,
        witness: search
            .found
            .map(|path| layout.decode(model, path))
            .transpose()?,
    })
}
