//! The reference evaluator: the semantics of a model, by exhaustive
//! enumeration within its scope (plan 0097). Every checker and lowering is
//! measured against it.
//!
//! - A signature has exactly its scope's atoms; enum members are one atom
//!   each; all atoms are distinct.
//! - A configuration assigns every `rel` a subset of its domain that meets
//!   its multiplicity, and satisfies every `fact`.
//! - A state assigns every `var` likewise; initial states also satisfy
//!   `init`. An action instance whose guards hold moves to the state its
//!   assignments give, evaluated in the current state; unassigned vars keep
//!   their value; a next state that breaks a type or multiplicity is not
//!   reached.
//! - `#` is a mathematical integer.
//! - With no initial state in any configuration every check is `empty`,
//!   never a vacuous pass.
//! - Depths are shortest transition counts from an initial state. A found
//!   violation or reached state is definite; `holds` and `unreachable`
//!   need the whole space within the budget, otherwise `unknown`.
use idealis_syntax::{
    Arith, Check, CheckKind, Compare, Expr, Formula, Int, IntCompare, Model, Mult, Quantifier,
    SetOp, Shape,
};
use idealis_syntax::{ValueDomain, ValueType};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::OnceLock;
mod guards;
mod history;
use guards::GuardPlan;
mod input;
mod lasso;
pub use history::{ObservationEvent, observation_history_pair};
pub use input::BoundCommand;
pub use lasso::LassoError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// No configuration has an initial state: nothing was checked, which is
    /// not a pass (the glossary's `empty`).
    Empty,
    Holds,
    Violated {
        depth: usize,
    },
    Reachable {
        depth: usize,
    },
    Unreachable,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub check: String,
    pub outcome: Outcome,
}

/// An element of a tuple: an atom of a signature or enum, or a value
/// (plan 0103). Values compare by their content.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Atom {
    Id(u16),
    Int(i64),
    /// The record's position among the model's records, and its fields.
    Record(u16, Box<[Atom]>),
    Sequence(Vec<Atom>),
    Variant(String, Option<Box<Atom>>),
}
type Tuple = Vec<Atom>;
type Value = BTreeSet<Tuple>;

fn reached_from(roots: impl Iterator<Item = Atom>, edges: &Value) -> BTreeSet<Atom> {
    let mut reached: BTreeSet<_> = roots.collect();
    let mut frontier: Vec<_> = reached.iter().cloned().collect();
    while let Some(node) = frontier.pop() {
        for edge in edges {
            if edge[0] == node && reached.insert(edge[1].clone()) {
                frontier.push(edge[1].clone());
            }
        }
    }
    reached
}

/// A domain larger than this many tuples is not enumerated: its subsets
/// alone would exhaust any budget.
const MAX_DOMAIN: usize = 20;

struct StructuredSort<'a> {
    domain: ValueDomain<'a>,
    atoms: OnceLock<Vec<Atom>>,
}

enum Candidates<'a> {
    Ready(Vec<Value>),
    Singleton {
        sort: &'a str,
        size: usize,
        optional: bool,
    },
}

impl Candidates<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Ready(values) => values.len(),
            Self::Singleton { size, optional, .. } => size.saturating_add(usize::from(*optional)),
        }
    }

    fn at(&self, index: usize, universe: &Universe<'_>) -> Value {
        match self {
            Self::Ready(values) => values[index].clone(),
            Self::Singleton { optional: true, .. } if index == 0 => Value::new(),
            Self::Singleton { sort, optional, .. } => {
                singleton(universe.atom_at(sort, index - usize::from(*optional)))
            }
        }
    }
}

struct Universe<'a> {
    /// Atoms of each signature and enum.
    sorts: BTreeMap<&'a str, Vec<Atom>>,
    structured: BTreeMap<&'a str, StructuredSort<'a>>,
    /// The atom of each enum member.
    members: BTreeMap<&'a str, Atom>,
    model: &'a Model,
}

impl<'a> Universe<'a> {
    fn argument_count(&self, params: &[(String, String)]) -> Option<usize> {
        let sizes = params.iter().map(|(_, sort)| self.size(sort));
        if sizes.clone().any(|size| size == 0) {
            return Some(0);
        }
        sizes.into_iter().try_fold(1usize, usize::checked_mul)
    }

    fn new(model: &'a Model) -> Self {
        let mut next: u16 = 0;
        let mut sorts = BTreeMap::new();
        let mut members = BTreeMap::new();
        for sig in &model.sigs {
            let size = model.scope.sizes[sig] as u16;
            sorts.insert(sig.as_str(), (next..next + size).map(Atom::Id).collect());
            next += size;
        }
        for (name, enum_members) in &model.enums {
            let mut atoms = Vec::new();
            for member in enum_members {
                members.insert(member.as_str(), Atom::Id(next));
                atoms.push(Atom::Id(next));
                next += 1;
            }
            sorts.insert(name.as_str(), atoms);
        }
        // A sum sort's atoms are its children's (plan 0100).
        for (name, children) in &model.sums {
            let atoms: Vec<Atom> = children
                .iter()
                .flat_map(|child| sorts[child.as_str()].clone())
                .collect();
            sorts.insert(name.as_str(), atoms);
        }
        // Values enumerate the scope: its integers, and records of every
        // combination of their fields (plan 0103).
        if let Some((low, high)) = model.scope.ints {
            sorts.insert(idealis_syntax::INT, (low..=high).map(Atom::Int).collect());
            sorts.insert(
                idealis_syntax::NAT,
                (low.max(0)..=high).map(Atom::Int).collect(),
            );
        }
        let mut universe = Self {
            sorts,
            structured: BTreeMap::new(),
            members,
            model,
        };
        for mut domain in model.value_domains().expect("checked value types") {
            let kind = domain.kind;
            domain.size = kind.size(|sort| universe.size(sort));
            let size = domain.size;
            universe.structured.insert(
                kind.name(),
                StructuredSort {
                    domain,
                    atoms: OnceLock::new(),
                },
            );
            if let ValueType::Record(record) = kind
                && !record.refinements.is_empty()
            {
                let atoms: Vec<_> = (0..size)
                    .map(|index| universe.atom_at(&record.name, index))
                    .filter(|atom| universe.satisfies_refinements(atom))
                    .collect();
                let filtered = universe.structured.get_mut(record.name.as_str()).unwrap();
                filtered.domain.size = atoms.len();
                filtered.atoms = OnceLock::from(atoms);
            }
        }
        universe
    }

    fn size(&self, sort: &str) -> usize {
        self.structured
            .get(sort)
            .map_or_else(|| self.sorts[sort].len(), |s| s.domain.size)
    }

    fn can_enumerate(&self, sort: &str) -> bool {
        self.structured
            .get(sort)
            .is_none_or(|s| s.domain.enumerable())
    }

    fn can_index(&self, sort: &str) -> bool {
        self.structured
            .get(sort)
            .is_none_or(|s| s.domain.indexable())
    }

    // Decode a rank in the same order as the materialized domain: the last
    // field varies fastest, sequences grow by length, variants follow cases.
    fn atom_at(&self, sort: &str, mut index: usize) -> Atom {
        let Some(s) = self.structured.get(sort) else {
            return self.sorts[sort][index].clone();
        };
        debug_assert!(s.domain.indexable() && index < s.domain.size);
        match s.domain.kind {
            ValueType::Record(record)
                if !record.refinements.is_empty() && s.atoms.get().is_some() =>
            {
                s.atoms.get().unwrap()[index].clone()
            }
            ValueType::Record(record) => Atom::Record(
                self.record_index(&record.name),
                self.ranked_fields(index, record.fields.iter().map(|(_, sort)| sort.as_str()))
                    .into(),
            ),
            ValueType::Sequence(seq) => {
                let size = self.size(&seq.element);
                for length in 0..=seq.max {
                    let block = size.saturating_pow(length as u32);
                    if index < block {
                        return Atom::Sequence(self.ranked_fields(
                            index,
                            std::iter::repeat_n(seq.element.as_str(), length),
                        ));
                    }
                    index -= block;
                }
                unreachable!("sequence rank is in its domain")
            }
            ValueType::Variant(variant) => {
                for case in &variant.cases {
                    let block = case.payload.as_deref().map_or(1, |sort| self.size(sort));
                    if index < block {
                        let payload = case
                            .payload
                            .as_deref()
                            .map(|sort| Box::new(self.atom_at(sort, index)));
                        return Atom::Variant(case.tag.clone(), payload);
                    }
                    index -= block;
                }
                unreachable!("variant rank is in its domain")
            }
        }
    }

    fn ranked_fields<'s>(
        &self,
        mut index: usize,
        sorts: impl DoubleEndedIterator<Item = &'s str>,
    ) -> Vec<Atom> {
        let mut fields = Vec::new();
        for sort in sorts.rev() {
            let radix = self.size(sort);
            fields.push(self.atom_at(sort, index % radix));
            index /= radix;
        }
        fields.reverse();
        fields
    }

    fn atoms(&self, sort: &str) -> &[Atom] {
        let Some(s) = self.structured.get(sort) else {
            return &self.sorts[sort];
        };
        debug_assert!(s.domain.enumerable());
        s.atoms
            .get_or_init(|| (0..s.domain.size).map(|i| self.atom_at(sort, i)).collect())
    }

    fn record_index(&self, name: &str) -> u16 {
        self.model
            .records
            .iter()
            .position(|r| r.name == name)
            .expect("checked record") as u16
    }

    fn satisfies_refinements(&self, atom: &Atom) -> bool {
        let Atom::Record(index, _) = atom else {
            return true;
        };
        self.model.records[usize::from(*index)]
            .refinements
            .iter()
            .all(|clause| clause.holds(|path| self.refinement_field(atom, path)))
    }

    fn refinement_field(&self, atom: &Atom, path: &[String]) -> i64 {
        let mut value = atom;
        for field in path {
            let Atom::Record(record, fields) = value else {
                unreachable!("checked field path")
            };
            let index = self.model.records[usize::from(*record)]
                .field_index(field)
                .unwrap();
            value = &fields[index];
        }
        let Atom::Int(n) = value else {
            unreachable!("checked numeric field")
        };
        *n
    }

    fn valid_data(&self, atom: &Atom) -> bool {
        match atom {
            Atom::Record(index, fields) => {
                let record = &self.model.records[usize::from(*index)];
                self.satisfies_refinements(atom)
                    && record.fields.iter().zip(fields).all(|((_, sort), atom)| {
                        (!matches!((sort.as_str(), atom), (idealis_syntax::NAT, Atom::Int(n)) if *n < 0))
                            && self.valid_data(atom)
                    })
            }
            Atom::Sequence(items) => items.iter().all(|item| self.valid_data(item)),
            Atom::Variant(_, Some(payload)) => self.valid_data(payload),
            _ => true,
        }
    }

    fn contains(&self, sort: &str, atom: &Atom) -> bool {
        self.contains_value(sort, atom, true)
    }

    fn contains_value(&self, sort: &str, atom: &Atom, refined: bool) -> bool {
        if let Some(values) = self.sorts.get(sort) {
            return values.contains(atom);
        }
        match (self.structured[sort].domain.kind, atom) {
            (ValueType::Record(record), Atom::Record(position, fields)) => {
                *position == self.record_index(&record.name)
                    && record.fields.len() == fields.len()
                    && record
                        .fields
                        .iter()
                        .zip(fields)
                        .all(|((_, sort), atom)| self.contains_value(sort, atom, refined))
                    && (!refined || self.satisfies_refinements(atom))
            }
            (ValueType::Sequence(seq), Atom::Sequence(items)) => {
                items.len() <= seq.max
                    && items
                        .iter()
                        .all(|atom| self.contains_value(&seq.element, atom, refined))
            }
            (ValueType::Variant(variant), Atom::Variant(tag, payload)) => variant
                .cases
                .iter()
                .find(|case| &case.tag == tag)
                .is_some_and(|case| match (&case.payload, payload) {
                    (None, None) => true,
                    (Some(sort), Some(atom)) => self.contains_value(sort, atom, refined),
                    _ => false,
                }),
            _ => false,
        }
    }

    fn shape_enumerable(&self, shape: &Shape) -> bool {
        match shape {
            Shape::Unary { sig, .. } => self.can_enumerate(sig),
            Shape::Binary { from, to, .. } => self.can_enumerate(from) && self.can_enumerate(to),
        }
    }

    fn large_parameter(&self, materialize: bool) -> Option<(&str, &str)> {
        self.model
            .actions
            .iter()
            .filter(|action| action.params.iter().all(|(_, sort)| self.size(sort) != 0))
            .find_map(|action| {
                action
                    .params
                    .iter()
                    .find(|(_, sort)| {
                        !self.can_index(sort) || materialize && !self.can_enumerate(sort)
                    })
                    .map(|(_, sort)| (action.name.as_str(), sort.as_str()))
            })
    }

    fn domain(&self, shape: &Shape) -> Vec<Tuple> {
        match shape {
            Shape::Unary { sig, .. } => self.atoms(sig).iter().map(|a| vec![a.clone()]).collect(),
            Shape::Binary { from, to, .. } => self
                .atoms(from)
                .iter()
                .flat_map(|a| {
                    self.atoms(to)
                        .iter()
                        .map(move |b| vec![a.clone(), b.clone()])
                })
                .collect(),
        }
    }
    fn admits(&self, shape: &Shape, value: &Value) -> bool {
        let fits = |mult: Mult, count: usize| match mult {
            Mult::Set => true,
            Mult::Lone => count <= 1,
            Mult::One => count == 1,
            Mult::Some => count >= 1,
        };
        match shape {
            Shape::Unary { mult, .. } => fits(*mult, value.len()),
            Shape::Binary { from, mult, .. } => self
                .atoms(from)
                .iter()
                .all(|a| fits(*mult, value.iter().filter(|t| &t[0] == a).count())),
        }
    }
    fn admits_update(&self, shape: &Shape, value: &Value) -> bool {
        self.admits(shape, value) && (!self.model.has_refinements() || self.in_domain(shape, value))
    }

    /// Whether every stored value lies in its scope, including nested fields.
    fn in_scope(&self, shape: &Shape, value: &Value) -> bool {
        let target = match shape {
            Shape::Unary { sig, .. } => sig,
            Shape::Binary { to, .. } => to,
        };
        !self.model.is_value(target)
            || value
                .iter()
                .all(|tuple| self.contains_value(target, tuple.last().expect("a target"), false))
    }
    /// The only integer of a set the checker proved singular.
    fn int(&self, value: &Value) -> i64 {
        match value.iter().next().map(|tuple| &tuple[0]) {
            Some(Atom::Int(value)) => *value,
            other => unreachable!("one integer by the checker, got {other:?}"),
        }
    }
    fn domain_size(&self, shape: &Shape) -> usize {
        match shape {
            Shape::Unary { sig, .. } => self.size(sig),
            Shape::Binary { from, to, .. } => self.size(from).saturating_mul(self.size(to)),
        }
    }

    fn in_domain(&self, shape: &Shape, value: &Value) -> bool {
        value.iter().all(|tuple| match (shape, tuple.as_slice()) {
            (Shape::Unary { sig, .. }, [atom]) => self.contains(sig, atom),
            (Shape::Binary { from, to, .. }, [key, atom]) => {
                self.contains(from, key) && self.contains(to, atom)
            }
            _ => false,
        })
    }

    fn candidates<'s>(
        &self,
        shape: &'s Shape,
        fixed: Option<&Expr>,
        prefer_fixed: bool,
    ) -> Option<Candidates<'s>> {
        if let Some(expression) = fixed
            && (prefer_fixed
                || self.domain_size(shape) > MAX_DOMAIN
                || !self.shape_enumerable(shape))
        {
            let value = self.expression(
                expression,
                &Scene {
                    relations: &BTreeMap::new(),
                    after: None,
                    bound: &BTreeMap::new(),
                },
            );
            let values = (self.in_domain(shape, &value) && self.admits(shape, &value))
                .then_some(value)
                .into_iter()
                .collect();
            return Some(Candidates::Ready(values));
        }
        if let Shape::Unary {
            sig,
            mult: mult @ (Mult::One | Mult::Lone),
        } = shape
        {
            return self.can_index(sig).then(|| Candidates::Singleton {
                sort: sig,
                size: self.size(sig),
                optional: *mult == Mult::Lone,
            });
        }
        if !self.shape_enumerable(shape) || self.domain_size(shape) > MAX_DOMAIN {
            return None;
        }
        let domain = self.domain(shape);
        Some(Candidates::Ready(
            (0u32..1 << domain.len())
                .map(|mask| {
                    domain
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| mask & (1 << index) != 0)
                        .map(|(_, tuple)| tuple.clone())
                        .collect::<Value>()
                })
                .filter(|value| self.admits(shape, value))
                .collect(),
        ))
    }
}

/// The relations' values and the atoms bound by quantifiers and parameters.
struct Scene<'s> {
    relations: &'s BTreeMap<&'s str, Value>,
    after: Option<&'s BTreeMap<&'s str, Value>>,
    bound: &'s BTreeMap<String, Atom>,
}

fn singleton(atom: Atom) -> Value {
    [vec![atom]].into()
}

impl Universe<'_> {
    fn scalar(&self, expr: &Expr, scene: &Scene<'_>) -> Atom {
        self.expression(expr, scene)
            .into_iter()
            .next()
            .expect("one value by the checker")
            .into_iter()
            .next()
            .expect("a unary value")
    }

    fn expression(&self, expression: &Expr, scene: &Scene<'_>) -> Value {
        match expression {
            Expr::At(phase, expr) => {
                let relations = match phase {
                    idealis_syntax::Phase::Before => scene.relations,
                    idealis_syntax::Phase::After => {
                        scene.after.expect("a checked transition property")
                    }
                };
                self.expression(
                    expr,
                    &Scene {
                        relations,
                        after: None,
                        bound: scene.bound,
                    },
                )
            }
            Expr::None => Value::new(),
            Expr::Keys(relation) => self
                .expression(relation, scene)
                .into_iter()
                .map(|mut tuple| {
                    tuple.truncate(1);
                    tuple
                })
                .collect(),
            Expr::ReachableFrom(roots, edges) => {
                let roots = self.expression(roots, scene);
                let edges = self.expression(edges, scene);
                reached_from(roots.into_iter().map(|root| root[0].clone()), &edges)
                    .into_iter()
                    .map(|node| vec![node])
                    .collect()
            }
            Expr::Closure(edges) => {
                let edges = self.expression(edges, scene);
                let sources: BTreeSet<_> = edges.iter().map(|edge| &edge[0]).collect();
                sources
                    .into_iter()
                    .flat_map(|source| {
                        let successors = edges
                            .iter()
                            .filter(|edge| &edge[0] == source)
                            .map(|edge| edge[1].clone());
                        reached_from(successors, &edges)
                            .into_iter()
                            .map(move |target| vec![source.clone(), target])
                    })
                    .collect()
            }
            Expr::Name(name) => {
                if let Some(atom) = scene.bound.get(name) {
                    return singleton(atom.clone());
                }
                if let Some(value) = scene.relations.get(name.as_str()) {
                    return value.clone();
                }
                if let Some(atom) = self.members.get(name.as_str()) {
                    return singleton(atom.clone());
                }
                self.sorts[name.as_str()]
                    .iter()
                    .map(|a| vec![a.clone()])
                    .collect()
            }
            Expr::Select(name, sort, predicate) => self
                .atoms(sort)
                .iter()
                .filter(|atom| self.test_atom(predicate, scene, name, atom))
                .map(|atom| vec![atom.clone()])
                .collect(),
            Expr::If(condition, yes, no) => {
                let branch = if self.formula(condition, scene) {
                    yes
                } else {
                    no
                };
                self.expression(branch, scene)
            }
            Expr::Literal(value) => singleton(Atom::Int(*value)),
            Expr::Count(counted) => {
                singleton(Atom::Int(self.expression(counted, scene).len() as i64))
            }
            Expr::Arith(operator, left, right) => {
                let (left, right) = (
                    self.int(&self.expression(left, scene)),
                    self.int(&self.expression(right, scene)),
                );
                // The checked model bounds every intermediate operation.
                singleton(Atom::Int(match operator {
                    Arith::Add => left + right,
                    Arith::Sub => left - right,
                    Arith::Mul => left * right,
                }))
            }
            Expr::Sequence(_, items) => singleton(Atom::Sequence(
                items.iter().map(|item| self.scalar(item, scene)).collect(),
            )),
            Expr::Append(seq, item) => {
                let Atom::Sequence(mut elements) = self.scalar(seq, scene) else {
                    unreachable!("typed sequence")
                };
                elements.push(self.scalar(item, scene));
                singleton(Atom::Sequence(elements))
            }
            Expr::Tail(seq) => {
                let Atom::Sequence(elements) = self.scalar(seq, scene) else {
                    unreachable!("typed sequence")
                };
                singleton(Atom::Sequence(elements.into_iter().skip(1).collect()))
            }
            Expr::Take(seq, count) => {
                let Atom::Sequence(mut elements) = self.scalar(seq, scene) else {
                    unreachable!("typed sequence")
                };
                let count = self
                    .int(&self.expression(count, scene))
                    .clamp(0, elements.len() as i64);
                elements.truncate(count as usize);
                singleton(Atom::Sequence(elements))
            }
            Expr::RemoveAt(seq, index) => {
                let Atom::Sequence(mut elements) = self.scalar(seq, scene) else {
                    unreachable!("typed sequence")
                };
                if let Ok(index) = usize::try_from(self.int(&self.expression(index, scene)))
                    && index < elements.len()
                {
                    elements.remove(index);
                }
                singleton(Atom::Sequence(elements))
            }
            Expr::RemoveAll(seq, excluded) => {
                let Atom::Sequence(mut elements) = self.scalar(seq, scene) else {
                    unreachable!("typed sequence")
                };
                let excluded = self.expression(excluded, scene);
                elements.retain(|item| !excluded.contains(std::slice::from_ref(item)));
                singleton(Atom::Sequence(elements))
            }
            Expr::Elements(seq) => {
                let Atom::Sequence(elements) = self.scalar(seq, scene) else {
                    unreachable!("typed sequence")
                };
                elements.into_iter().map(|element| vec![element]).collect()
            }
            Expr::Length(seq) => {
                let Atom::Sequence(elements) = self.scalar(seq, scene) else {
                    unreachable!("typed sequence")
                };
                singleton(Atom::Int(elements.len() as i64))
            }
            Expr::Index(seq, index) => {
                let Atom::Sequence(elements) = self.scalar(seq, scene) else {
                    unreachable!("typed sequence")
                };
                let index = self.int(&self.expression(index, scene));
                usize::try_from(index)
                    .ok()
                    .and_then(|i| elements.get(i))
                    .cloned()
                    .map_or_else(Value::new, singleton)
            }
            Expr::Variant(tag, value) => singleton(Atom::Variant(
                tag.clone(),
                value.as_ref().map(|v| Box::new(self.scalar(v, scene))),
            )),
            Expr::Payload(expr, tag) => {
                let Atom::Variant(actual, value) = self.scalar(expr, scene) else {
                    unreachable!("typed variant")
                };
                if &actual == tag
                    && let Some(value) = value
                {
                    singleton(*value)
                } else {
                    Value::new()
                }
            }
            Expr::Record(name, values) => {
                let fields: Vec<Atom> = values
                    .iter()
                    .map(|value| {
                        let value = self.expression(value, scene);
                        value.into_iter().next().expect("one value by the checker")[0].clone()
                    })
                    .collect();
                singleton(Atom::Record(self.record_index(name), fields.into()))
            }
            Expr::Field(receiver, field) => self
                .expression(receiver, scene)
                .into_iter()
                .map(|mut tuple| {
                    let value = tuple.last_mut().expect("a nonempty tuple");
                    let Atom::Record(record, fields) = value else {
                        unreachable!("a record by the checker, got {value:?}");
                    };
                    let index = self.model.records[usize::from(*record)]
                        .field_index(field)
                        .expect("a field of the record");
                    *value = fields[index].clone();
                    tuple
                })
                .collect(),
            Expr::Binary(operator, left, right) => {
                let (left, right) = (self.expression(left, scene), self.expression(right, scene));
                match operator {
                    SetOp::Union | SetOp::SetUnion => left.union(&right).cloned().collect(),
                    SetOp::Override => {
                        let keys: BTreeSet<_> = right.iter().map(|tuple| &tuple[0]).collect();
                        left.iter()
                            .filter(|tuple| !keys.contains(&tuple[0]))
                            .chain(&right)
                            .cloned()
                            .collect()
                    }
                    SetOp::Difference | SetOp::SetDifference => {
                        left.difference(&right).cloned().collect()
                    }
                    SetOp::WithoutKeys => left
                        .into_iter()
                        .filter(|row| !right.contains(&row[..1]))
                        .collect(),
                    SetOp::Intersection => left.intersection(&right).cloned().collect(),
                    SetOp::Product => left
                        .iter()
                        .flat_map(|l| right.iter().map(move |r| [l.as_slice(), r].concat()))
                        .collect(),
                    SetOp::Join => left
                        .iter()
                        .flat_map(|l| {
                            right
                                .iter()
                                .filter(move |r| l.last() == r.first())
                                .map(move |r| [&l[..l.len() - 1], &r[1..]].concat())
                        })
                        .collect(),
                    SetOp::Multiply => unreachable!("the checker rewrites *"),
                }
            }
        }
    }
    fn formula(&self, formula: &Formula, scene: &Scene<'_>) -> bool {
        match formula {
            Formula::Compare(operator, left, right) => {
                let (left, right) = (self.expression(left, scene), self.expression(right, scene));
                match operator {
                    Compare::In => left.is_subset(&right),
                    Compare::NotIn => !left.is_subset(&right),
                    Compare::Equal => left == right,
                    Compare::NotEqual => left != right,
                }
            }
            Formula::Ints(operator, left, right) => {
                let int = |int: &Int| -> i64 {
                    match int {
                        Int::Count(counted) => self.expression(counted, scene).len() as i64,
                        Int::Literal(value) => *value,
                        Int::Value(value) | Int::LocatedValue(value, _) => {
                            self.int(&self.expression(value, scene))
                        }
                    }
                };
                let (left, right) = (int(left), int(right));
                match operator {
                    IntCompare::Less => left < right,
                    IntCompare::LessEqual => left <= right,
                    IntCompare::Greater => left > right,
                    IntCompare::GreaterEqual => left >= right,
                    IntCompare::Equal => left == right,
                    IntCompare::NotEqual => left != right,
                }
            }
            Formula::Mult(mult, counted) => {
                let count = self.expression(counted, scene).len();
                match mult {
                    Mult::Some => count > 0,
                    Mult::One => count == 1,
                    Mult::Lone => count <= 1,
                    Mult::Set => true,
                }
            }
            Formula::HasTag(expr, tag) => {
                matches!(self.scalar(expr, scene), Atom::Variant(actual, _) if &actual == tag)
            }
            Formula::Empty(counted) => self.expression(counted, scene).is_empty(),
            Formula::Not(inner) => !self.formula(inner, scene),
            Formula::And(parts) => parts.iter().all(|part| self.formula(part, scene)),
            Formula::Or(left, right) => self.formula(left, scene) || self.formula(right, scene),
            Formula::Implies(left, right) => {
                !self.formula(left, scene) || self.formula(right, scene)
            }
            Formula::Quantified(quantifier, variable, sig, body) => {
                let test = |atom: &Atom| self.test_atom(body, scene, variable, atom);
                let atoms = self.atoms(sig);
                match quantifier {
                    Quantifier::All => atoms.iter().all(test),
                    Quantifier::Some => atoms.iter().any(test),
                }
            }
        }
    }
    fn test_atom(&self, predicate: &Formula, scene: &Scene<'_>, name: &str, atom: &Atom) -> bool {
        let mut bound = scene.bound.clone();
        bound.insert(name.to_owned(), atom.clone());
        self.formula(
            predicate,
            &Scene {
                relations: scene.relations,
                after: scene.after,
                bound: &bound,
            },
        )
    }
}

// Enumerate ranks separately so callers can charge the search budget
// before constructing the corresponding candidate values.
fn advance_indices(indices: &mut [usize], lengths: &[usize]) -> bool {
    for (index, length) in indices.iter_mut().zip(lengths).rev() {
        *index += 1;
        if *index < *length {
            return true;
        }
        *index = 0;
    }
    false
}

fn index_product(lengths: Vec<usize>) -> impl Iterator<Item = Vec<usize>> {
    let mut indices = vec![0usize; lengths.len()];
    let mut pending = !lengths.contains(&0);
    std::iter::from_fn(move || {
        if !pending {
            return None;
        }
        let item = indices.clone();
        pending = advance_indices(&mut indices, &lengths);
        Some(item)
    })
}

fn candidate_indices(choices: &[Candidates<'_>]) -> impl Iterator<Item = Vec<usize>> {
    index_product(choices.iter().map(Candidates::len).collect())
}

fn candidate_values(
    universe: &Universe<'_>,
    choices: &[Candidates<'_>],
    indices: Vec<usize>,
) -> Vec<Value> {
    choices
        .iter()
        .zip(indices)
        .map(|(choice, index)| choice.at(index, universe))
        .collect()
}

fn binding_indices(
    universe: &Universe<'_>,
    params: &[(String, String)],
) -> impl Iterator<Item = Vec<usize>> {
    index_product(params.iter().map(|(_, sort)| universe.size(sort)).collect())
}

fn bind_values(
    universe: &Universe<'_>,
    params: &[(String, String)],
    indices: Vec<usize>,
) -> BTreeMap<String, Atom> {
    params
        .iter()
        .zip(indices)
        .map(|((name, sort), index)| (name.clone(), universe.atom_at(sort, index)))
        .collect()
}

/// Every concrete binding, in declaration and domain order.
fn bindings<'a>(
    universe: &'a Universe<'_>,
    params: &'a [(String, String)],
) -> impl Iterator<Item = BTreeMap<String, Atom>> + 'a {
    binding_indices(universe, params).map(move |indices| bind_values(universe, params, indices))
}

/// The verdict of every check, examining at most `max_states` configurations,
/// initial candidates and states in all.
pub fn evaluate(model: &Model, max_states: usize) -> Vec<Verdict> {
    analyze(model, max_states).verdicts
}

/// Verdicts, and whether each action has a transition from some reachable
/// state (plan 0098); `enabled` is `None` for an action never seen enabled
/// in an incomplete or empty search.
pub struct Report {
    pub verdicts: Vec<Verdict>,
    /// Whether a transition breaks a declared multiplicity or data refinement:
    /// `Violated` with the shortest depth of the breaking state, `Holds`,
    /// `Empty` or `Unknown`.
    pub declarations: Outcome,
    /// Whether a state leaves the scope's integers (plan 0103): `Violated`
    /// with the shortest depth of such a state. Leaving makes the search
    /// incomplete, so checks without a witness are then `Unknown`.
    pub scope: Outcome,
    /// Why the search stopped, independently of any scope exit it observed.
    pub stop: Option<StopReason>,
    pub enabled: Vec<(String, Option<bool>)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopReason {
    Budget,
    ArgumentBudget,
    EnumerationLimit { relation: String },
    ArgumentEnumerationLimit { action: String, sort: String },
}

#[derive(Clone, Copy, Debug)]
pub struct SearchBudget {
    pub states: usize,
    /// Counts every operation binding tried, including disabled commands.
    pub arguments: u64,
}

impl SearchBudget {
    pub fn for_states(states: usize) -> Self {
        Self {
            states,
            arguments: 2_000_000,
        }
    }
}

/// Whether every relation has enumerable initial choices or a fixed value.
pub fn enumerable(model: &Model) -> Result<(), String> {
    check_enumerable(model, &Universe::new(model), false)
}

fn check_enumerable(
    model: &Model,
    universe: &Universe<'_>,
    collect_initial: bool,
) -> Result<(), String> {
    if let Some((action, sort)) = universe.large_parameter(collect_initial) {
        return Err(format!(
            "action {action}: {sort} exceeds the value materialization limit"
        ));
    }
    let fixed = crate::initial::fixed_initial_values(model);
    for relation in &model.relations {
        if fixed.contains_key(relation.name.as_str()) {
            continue;
        }
        if let Shape::Unary {
            sig,
            mult: Mult::One | Mult::Lone,
        } = &relation.shape
            && model.is_value(sig)
        {
            if !universe.can_index(sig) || collect_initial && !universe.can_enumerate(sig) {
                return Err(format!(
                    "{} exceeds the value materialization limit",
                    relation.name
                ));
            }
            continue;
        }
        if !universe.shape_enumerable(&relation.shape) {
            return Err(format!(
                "{} exceeds the value materialization limit",
                relation.name
            ));
        }
        let size = universe.domain_size(&relation.shape);
        if size > MAX_DOMAIN {
            return Err(format!(
                "{} has {size} possible tuples, more than {MAX_DOMAIN}",
                relation.name
            ));
        }
    }
    Ok(())
}

pub fn analyze(model: &Model, max_states: usize) -> Report {
    analyze_with_budget(model, SearchBudget::for_states(max_states))
}

pub fn analyze_with_budget(model: &Model, budget: SearchBudget) -> Report {
    explore(
        &Universe::new(model),
        budget,
        InitialCandidates::Enumerate,
        None,
        None,
    )
}

/// Action reachability using any fixed values in the initial constraints.
pub fn analyze_actions(model: &Model, max_states: usize) -> Report {
    analyze_actions_with_budget(model, SearchBudget::for_states(max_states))
}

pub fn analyze_actions_with_budget(model: &Model, budget: SearchBudget) -> Report {
    explore(
        &Universe::new(model),
        budget,
        InitialCandidates::UseFixed,
        None,
        None,
    )
}

/// Guard reachability for each concrete call to the requested action.
/// Counts configurations and states as in `analyze_actions`; `max_instances`
/// bounds the size of the returned call catalogue.
pub fn analyze_instances(
    model: &Model,
    action: &str,
    max_states: usize,
    max_instances: usize,
) -> Result<(Report, Vec<InstanceReachability>), String> {
    analyze_instances_with_budget(
        model,
        action,
        SearchBudget::for_states(max_states),
        max_instances,
    )
}

pub fn analyze_instances_with_budget(
    model: &Model,
    action: &str,
    budget: SearchBudget,
    max_instances: usize,
) -> Result<(Report, Vec<InstanceReachability>), String> {
    let index = model
        .actions
        .iter()
        .position(|candidate| candidate.name == action)
        .ok_or_else(|| format!("no action {action}"))?;
    let universe = Universe::new(model);
    let count = instance_count(&universe, &model.actions[index], max_instances)?;
    let mut reached = BTreeMap::from([(index, vec![false; count])]);
    let report = explore(
        &universe,
        budget,
        InitialCandidates::UseFixed,
        None,
        Some(&mut reached),
    );
    let instances = describe_instances(
        &universe,
        &model.actions[index],
        &reached[&index],
        &atom_names(model),
        report.scope,
    );
    Ok((report, instances))
}

fn instance_count(
    universe: &Universe<'_>,
    action: &idealis_syntax::Action,
    limit: usize,
) -> Result<usize, String> {
    let params = &action.params;
    let count = universe
        .argument_count(params)
        .filter(|&count| count <= limit)
        .ok_or_else(|| format!("action {} exceeds the {limit} instance limit", action.name))?;
    if count != 0 && params.iter().any(|(_, sort)| !universe.can_index(sort)) {
        return Err(format!(
            "action {} exceeds the value materialization limit",
            action.name
        ));
    }
    Ok(count)
}

fn describe_instances(
    universe: &Universe<'_>,
    action: &idealis_syntax::Action,
    reached: &[bool],
    names: &BTreeMap<u16, String>,
    scope: Outcome,
) -> Vec<InstanceReachability> {
    if reached.is_empty() {
        return Vec::new();
    }
    bindings(universe, &action.params)
        .zip(reached.iter().copied())
        .map(|(bound, reached)| InstanceReachability {
            call: TraceCommand {
                action: action.name.clone(),
                args: action
                    .params
                    .iter()
                    .map(|(name, _)| (name.clone(), atom_data(universe.model, names, &bound[name])))
                    .collect(),
            },
            enabled: (reached || scope == Outcome::Holds).then_some(reached),
        })
        .collect()
}

pub struct InstanceReachability {
    pub call: TraceCommand,
    pub enabled: Option<bool>,
}

type Call = (usize, BTreeMap<String, Atom>);

struct PathLink {
    before: Vec<Value>,
    call: Call,
}

struct RawWitness {
    config: Snapshot,
    initial: Snapshot,
    commands: Vec<Call>,
    final_state: Snapshot,
    events: Option<Vec<RawEvent>>,
}

struct RawEvent {
    step: usize,
    event: String,
    observation: Vec<(String, Value)>,
}

struct WitnessSearch<'a> {
    target: WitnessTarget<'a>,
    parents: BTreeMap<Vec<Value>, PathLink>,
    found: Option<RawWitness>,
    pair: Option<PairSearch<'a>>,
}

enum WitnessTarget<'a> {
    Question(&'a str),
    Deadlock {
        mode: DeadlockMode,
        unless: Option<&'a Formula>,
        depth: Option<usize>,
    },
    UpdateBranches(Vec<Vec<BranchSearch<'a>>>),
}

struct BranchSearch<'a> {
    sites: Vec<BranchSite<'a>>,
    found: [Option<RawWitness>; 2],
}

struct BranchSite<'a> {
    ancestors: Vec<(&'a Formula, bool)>,
    condition: &'a Formula,
}

impl<'a> BranchSearch<'a> {
    fn new(
        branch: &idealis_syntax::UpdateBranch,
        action: &'a idealis_syntax::Action,
    ) -> Option<Self> {
        let sites = if branch.value_sites().is_empty() {
            let (assignee, ancestors) = branch.selector();
            let path: Vec<_> = ancestors.iter().map(|&yes| usize::from(!yes)).collect();
            vec![BranchSite::resolve(action, assignee, &path, false)?]
        } else {
            branch
                .value_sites()
                .iter()
                .map(|(assignment, path)| BranchSite::resolve(action, assignment, path, true))
                .collect::<Option<_>>()?
        };
        Some(Self {
            sites,
            found: [None, None],
        })
    }
}

impl<'a> BranchSite<'a> {
    fn resolve(
        action: &'a idealis_syntax::Action,
        assignment: &str,
        path: &[usize],
        expanded: bool,
    ) -> Option<Self> {
        let (_, value) = action
            .assignments
            .iter()
            .find(|(name, _)| name == assignment)?;
        let mut current = value;
        let mut ancestors = Vec::new();
        for &index in path {
            if let Expr::If(condition, ..) = current {
                ancestors.push((condition.as_ref(), index == 0));
            } else if !expanded {
                return None;
            }
            current = current.coverage_child(index)?;
        }
        let Expr::If(condition, ..) = current else {
            return None;
        };
        Some(Self {
            ancestors,
            condition,
        })
    }
}

fn record_path(
    parents: &BTreeMap<Vec<Value>, PathLink>,
    config: &[Value],
    state: &[Value],
    final_step: Option<(&[Value], Call)>,
) -> RawWitness {
    let mut commands = Vec::new();
    let mut cursor = state;
    if let Some((before, call)) = final_step {
        commands.push(call);
        cursor = before;
    }
    while let Some(previous) = parents.get(cursor) {
        commands.push(previous.call.clone());
        cursor = &previous.before;
    }
    commands.reverse();
    RawWitness {
        config: Snapshot(config.to_vec()),
        initial: Snapshot(cursor.to_vec()),
        commands,
        final_state: Snapshot(state.to_vec()),
        events: None,
    }
}

impl WitnessSearch<'_> {
    fn targets_question(&self, name: &str) -> bool {
        matches!(self.target, WitnessTarget::Question(question) if question == name)
    }

    fn record(&mut self, config: &[Value], state: &[Value], final_step: Option<(&[Value], Call)>) {
        self.found = Some(record_path(&self.parents, config, state, final_step));
    }
}

struct PairSearch<'a> {
    observe: Vec<ObservationField<'a>>,
    distinguish: Vec<ObservationField<'a>>,
    groups: BTreeMap<Vec<Value>, ObservationState>,
    best: Option<[RawWitness; 2]>,
}

struct ObservationState {
    internal: Vec<Value>,
    state: Vec<Value>,
    depth: usize,
}

impl PairSearch<'_> {
    fn visit(
        &mut self,
        parents: &BTreeMap<Vec<Value>, PathLink>,
        config: &[Value],
        state: &[Value],
        depth: usize,
        universe: &Universe<'_>,
        scene: &Scene<'_>,
    ) {
        let project = |fields: &[ObservationField<'_>]| {
            fields
                .iter()
                .map(|field| field.project(state, universe, scene))
                .collect::<Vec<_>>()
        };
        let observation = project(&self.observe);
        let internal = project(&self.distinguish);
        match self.groups.entry(observation) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(ObservationState {
                    internal,
                    state: state.to_vec(),
                    depth,
                });
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                let first = entry.get();
                // Within one configuration the states arrive in breadth-first order.
                if internal != first.internal
                    && self.best.as_ref().is_none_or(|pair| {
                        first.depth + depth < pair.iter().map(|trace| trace.commands.len()).sum()
                    })
                {
                    self.best = Some([
                        record_path(parents, config, &first.state, None),
                        record_path(parents, config, state, None),
                    ]);
                }
            }
        }
    }
}

/// One reachable path from an initial state, including the final operation on violations.
pub struct Witness {
    pub config: RelationData,
    pub initial: RelationData,
    pub commands: Vec<TraceCommand>,
    pub final_state: RelationData,
    pub events: Option<Vec<WitnessEvent>>,
}

/// A named observation emitted after a command in a history witness.
pub struct WitnessEvent {
    pub step: usize,
    pub event: String,
    pub observation: RelationData,
}

type RelationData = Vec<(String, Vec<Vec<Data>>)>;

pub struct TraceCommand {
    pub action: String,
    pub args: Vec<(String, Data)>,
}

pub struct WitnessResult {
    pub outcome: Outcome,
    /// The search exhausted the model scope and did not encounter an escaped state.
    pub complete: bool,
    pub scope: Outcome,
    pub declarations: Outcome,
    pub stop: Option<StopReason>,
    pub witness: Option<Witness>,
}

pub struct BranchWitnesses {
    pub complete: bool,
    pub scope: Outcome,
    pub declarations: Outcome,
    pub stop: Option<StopReason>,
    /// The witness for each source group's then and else arm, in that order.
    pub branches: Vec<[Option<Witness>; 2]>,
}

/// Find witnesses for every source-level conditional update arm in a single search.
pub fn update_branch_witnesses(
    model: &Model,
    action: &str,
    branches: &[idealis_syntax::UpdateBranch],
    budget: SearchBudget,
) -> Result<BranchWitnesses, String> {
    let index = model
        .actions
        .iter()
        .position(|candidate| candidate.name == action)
        .ok_or_else(|| format!("no action named {action}"))?;
    let (report, mut paths) =
        explore_update_branches(&Universe::new(model), [(index, branches)], budget, None)?;
    Ok(BranchWitnesses {
        complete: matches!(report.scope, Outcome::Holds | Outcome::Empty)
            && !matches!(report.declarations, Outcome::Violated { .. }),
        scope: report.scope,
        declarations: report.declarations,
        stop: report.stop,
        branches: std::mem::take(&mut paths[index]),
    })
}

/// Per-action witnesses for each source update group's then and else arm.
pub type UpdatePaths = Vec<Vec<[Option<Witness>; 2]>>;

/// Explore once for both operation reachability and all source update groups.
pub fn update_coverage(
    model: &Model,
    branches: &BTreeMap<String, Vec<idealis_syntax::UpdateBranch>>,
    budget: SearchBudget,
) -> Result<(Report, UpdatePaths), String> {
    all_update_coverage(&Universe::new(model), branches, budget, None)
}

pub struct CoverageWithInstances {
    pub report: Report,
    pub paths: UpdatePaths,
    /// A catalogue or an enumeration diagnostic per action, in model order.
    pub instances: Vec<Result<Vec<InstanceReachability>, String>>,
}

/// Explore the model once for actions, argument bindings and update groups.
pub fn update_coverage_with_instances(
    model: &Model,
    branches: &BTreeMap<String, Vec<idealis_syntax::UpdateBranch>>,
    budget: SearchBudget,
    max_instances: usize,
) -> Result<CoverageWithInstances, String> {
    let universe = Universe::new(model);
    let mut remaining = max_instances;
    let mut reached = BTreeMap::new();
    let catalogues = model
        .actions
        .iter()
        .enumerate()
        .map(|(index, action)| {
            let count = instance_count(&universe, action, max_instances)?;
            remaining = remaining.checked_sub(count).ok_or_else(|| {
                format!("the model exceeds the {max_instances} total instance limit")
            })?;
            reached.insert(index, vec![false; count]);
            Ok(index)
        })
        .collect::<Vec<_>>();
    let (report, paths) = all_update_coverage(&universe, branches, budget, Some(&mut reached))?;
    let names = atom_names(model);
    let instances = catalogues
        .into_iter()
        .map(|catalogue| {
            catalogue.map(|index| {
                describe_instances(
                    &universe,
                    &model.actions[index],
                    &reached[&index],
                    &names,
                    report.scope,
                )
            })
        })
        .collect();
    Ok(CoverageWithInstances {
        report,
        paths,
        instances,
    })
}

fn all_update_coverage<'a>(
    universe: &Universe<'a>,
    branches: &'a BTreeMap<String, Vec<idealis_syntax::UpdateBranch>>,
    budget: SearchBudget,
    instances: Option<&mut BTreeMap<usize, Vec<bool>>>,
) -> Result<(Report, UpdatePaths), String> {
    let model = universe.model;
    let selected = model
        .actions
        .iter()
        .enumerate()
        .filter_map(|(index, action)| {
            branches
                .get(&action.name)
                .map(|groups| (index, groups.as_slice()))
        });
    explore_update_branches(universe, selected, budget, instances)
}

fn explore_update_branches<'a>(
    universe: &Universe<'a>,
    selected: impl IntoIterator<Item = (usize, &'a [idealis_syntax::UpdateBranch])>,
    budget: SearchBudget,
    instances: Option<&mut BTreeMap<usize, Vec<bool>>>,
) -> Result<(Report, UpdatePaths), String> {
    let model = universe.model;
    let mut by_action = (0..model.actions.len())
        .map(|_| Vec::new())
        .collect::<Vec<_>>();
    for (index, branches) in selected {
        let action = &model.actions[index];
        by_action[index] = branches
            .iter()
            .map(|branch| {
                BranchSearch::new(branch, action)
                    .ok_or_else(|| format!("branch metadata does not match {}", action.name))
            })
            .collect::<Result<Vec<_>, _>>()?;
    }
    if by_action.iter().all(Vec::is_empty) {
        return Ok((
            explore(
                universe,
                budget,
                InitialCandidates::UseFixed,
                None,
                instances,
            ),
            by_action.into_iter().map(|_| Vec::new()).collect(),
        ));
    }
    let mut search = WitnessSearch {
        target: WitnessTarget::UpdateBranches(by_action),
        parents: BTreeMap::new(),
        found: None,
        pair: None,
    };
    let report = explore(
        universe,
        budget,
        InitialCandidates::UseFixed,
        Some(&mut search),
        instances,
    );
    let WitnessTarget::UpdateBranches(branches) = search.target else {
        unreachable!("update branch query");
    };
    let machine = branches
        .iter()
        .flatten()
        .any(|branch| branch.found.iter().any(Option::is_some))
        .then(|| Machine::for_search(model))
        .transpose()?;
    let paths = branches
        .into_iter()
        .map(|groups| {
            groups
                .into_iter()
                .map(|branch| {
                    branch.found.map(|raw| {
                        raw.map(|raw| {
                            encode_witness(machine.as_ref().expect("witness encoder"), raw)
                        })
                    })
                })
                .collect()
        })
        .collect();
    Ok((report, paths))
}

pub struct ObservationPairResult {
    pub outcome: Outcome,
    pub complete: bool,
    pub pair: Option<[Witness; 2]>,
}

fn observation_point<'a>(model: &'a Model, at: &str) -> Result<&'a Check, String> {
    model
        .checks
        .iter()
        .find(|check| check.name == at && check.kind == CheckKind::Reachable)
        .ok_or_else(|| format!("no reachable observation point named {at}"))
}

#[derive(Clone, Copy)]
enum ObservationField<'a> {
    State(usize),
    Derived(&'a str, &'a Expr),
}

impl ObservationField<'_> {
    fn project(self, state: &[Value], universe: &Universe<'_>, scene: &Scene<'_>) -> Value {
        match self {
            Self::State(index) => state[index].clone(),
            Self::Derived(_, expression) => universe.expression(expression, scene),
        }
    }
}

fn observation_fields<'a>(
    model: &'a Model,
    names: &[String],
) -> Result<Vec<ObservationField<'a>>, String> {
    if names.is_empty() || names.iter().collect::<BTreeSet<_>>().len() != names.len() {
        return Err("field lists must be nonempty and contain distinct names".into());
    }
    names
        .iter()
        .map(|name| {
            model
                .vars()
                .position(|relation| relation.name == *name)
                .map(ObservationField::State)
                .or_else(|| {
                    model
                        .derivations
                        .iter()
                        .find(|(derived, _)| derived == name)
                        .map(|(name, expression)| ObservationField::Derived(name, expression))
                })
                .ok_or_else(|| format!("unknown state field {name}"))
        })
        .collect()
}

/// Compare reachable states at one named observation point within each configuration.
pub fn observation_pair(
    model: &Model,
    at: &str,
    observe: &[String],
    distinguish: &[String],
    max_states: usize,
) -> Result<ObservationPairResult, String> {
    observation_point(model, at)?;
    let mut search = WitnessSearch {
        target: WitnessTarget::Question(at),
        parents: BTreeMap::new(),
        found: None,
        pair: Some(PairSearch {
            observe: observation_fields(model, observe)?,
            distinguish: observation_fields(model, distinguish)?,
            groups: BTreeMap::new(),
            best: None,
        }),
    };
    let report = explore(
        &Universe::new(model),
        SearchBudget::for_states(max_states),
        InitialCandidates::UseFixed,
        Some(&mut search),
        None,
    );
    let complete = matches!(report.scope, Outcome::Holds | Outcome::Empty);
    let pair = search.pair.expect("pair search").best;
    let outcome = if let Some(pair) = &pair {
        Outcome::Reachable {
            depth: pair.iter().map(|trace| trace.commands.len()).sum(),
        }
    } else if !complete {
        Outcome::Unknown
    } else if report.scope == Outcome::Empty {
        Outcome::Empty
    } else {
        Outcome::Unreachable
    };
    let pair = match pair {
        Some(pair) => {
            let machine = Machine::for_search(model)?;
            Some(pair.map(|trace| encode_witness(&machine, trace)))
        }
        None => None,
    };
    Ok(ObservationPairResult {
        outcome,
        complete,
        pair,
    })
}

/// Save a path for one finite question, counting initial candidates after fixed constraints.
pub fn witness(model: &Model, question: &str, max_states: usize) -> Result<WitnessResult, String> {
    witness_with_budget(model, question, SearchBudget::for_states(max_states))
}

pub fn witness_with_budget(
    model: &Model,
    question: &str,
    budget: SearchBudget,
) -> Result<WitnessResult, String> {
    if !matches!(
        question,
        idealis_syntax::DECLARATIONS | idealis_syntax::SCOPE
    ) && !model.checks.iter().any(|check| check.name == question)
        && !model.transitions.iter().any(|check| check.name == question)
    {
        return Err(format!(
            "no finite state or transition question named {question}"
        ));
    }
    let mut search = WitnessSearch {
        target: WitnessTarget::Question(question),
        parents: BTreeMap::new(),
        found: None,
        pair: None,
    };
    let report = explore(
        &Universe::new(model),
        budget,
        InitialCandidates::UseFixed,
        Some(&mut search),
        None,
    );
    let outcome = match question {
        idealis_syntax::DECLARATIONS => report.declarations,
        idealis_syntax::SCOPE => report.scope,
        _ => {
            report
                .verdicts
                .iter()
                .find(|answer| answer.check == question)
                .expect("known question")
                .outcome
        }
    };
    encode_witness_result(model, outcome, report, search.found)
}

/// Find a reachable state with no enabled operation outside an optional completion predicate.
pub fn deadlock_witness(
    model: &Model,
    unless: Option<&str>,
    budget: SearchBudget,
) -> Result<WitnessResult, String> {
    deadlock_witness_with_mode(model, unless, budget, DeadlockMode::Guards)
}

#[derive(Clone, Copy)]
pub enum DeadlockMode {
    /// All enabled guards count as exits.
    Guards,
    /// In-scope, well-formed self-loops do not count as exits.
    IgnoreStutter,
}

/// Find a reachable state without an exit under the selected definition.
pub fn deadlock_witness_with_mode(
    model: &Model,
    unless: Option<&str>,
    budget: SearchBudget,
    mode: DeadlockMode,
) -> Result<WitnessResult, String> {
    let unless = unless
        .map(|name| {
            observation_point(model, name)
                .map(|check| &check.formula)
                .map_err(|_| format!("no reachable completion condition named {name}"))
        })
        .transpose()?;
    let mut search = WitnessSearch {
        target: WitnessTarget::Deadlock {
            mode,
            unless,
            depth: None,
        },
        parents: BTreeMap::new(),
        found: None,
        pair: None,
    };
    let report = explore(
        &Universe::new(model),
        budget,
        InitialCandidates::UseFixed,
        Some(&mut search),
        None,
    );
    let WitnessTarget::Deadlock { depth, .. } = search.target else {
        unreachable!("deadlock query")
    };
    let outcome = match (depth, report.scope, report.declarations) {
        (Some(depth), _, _) => Outcome::Reachable { depth },
        (None, Outcome::Holds, Outcome::Holds) => Outcome::Unreachable,
        (None, Outcome::Empty, _) => Outcome::Empty,
        _ => Outcome::Unknown,
    };
    let mut result = encode_witness_result(model, outcome, report, search.found)?;
    result.complete &= !matches!(result.declarations, Outcome::Violated { .. });
    Ok(result)
}

fn encode_witness_result(
    model: &Model,
    outcome: Outcome,
    report: Report,
    found: Option<RawWitness>,
) -> Result<WitnessResult, String> {
    let witness = if let Some(raw) = found {
        let machine = Machine::for_search(model)?;
        Some(encode_witness(&machine, raw))
    } else {
        None
    };
    Ok(WitnessResult {
        outcome,
        complete: matches!(report.scope, Outcome::Holds | Outcome::Empty),
        scope: report.scope,
        declarations: report.declarations,
        stop: report.stop,
        witness,
    })
}

/// Find a legal invocation of an action whose prestate and poststate match a predicate.
pub fn transition_witness(
    model: &Model,
    action: &str,
    predicate: &str,
    max_states: usize,
) -> Result<WitnessResult, String> {
    transition_witness_with_budget(
        model,
        action,
        predicate,
        SearchBudget::for_states(max_states),
    )
}

pub fn transition_witness_with_budget(
    model: &Model,
    action: &str,
    predicate: &str,
    budget: SearchBudget,
) -> Result<WitnessResult, String> {
    let selected = model
        .actions
        .iter()
        .find(|candidate| candidate.name == action)
        .ok_or_else(|| format!("no action named {action}"))?;
    let formula = idealis_syntax::transition_predicate(model, selected, predicate)
        .map_err(|error| error.to_string())?;
    let used: BTreeSet<_> = model
        .checks
        .iter()
        .map(|check| check.name.as_str())
        .chain(model.transitions.iter().map(|check| check.name.as_str()))
        .collect();
    let name = (0..)
        .map(|index| format!("WitnessStep{index}"))
        .find(|candidate| !used.contains(candidate.as_str()))
        .expect("a fresh name for the query");

    // A counterexample to the negated predicate ends with a matching step.
    let mut queried = model.clone();
    queried.transitions.push(idealis_syntax::TransitionCheck {
        name: name.clone(),
        action: Some(action.to_owned()),
        formula: Formula::Not(Box::new(formula)),
    });
    let mut result = witness_with_budget(&queried, &name, budget)?;
    result.outcome = match result.outcome {
        Outcome::Violated { depth } => Outcome::Reachable { depth },
        Outcome::Holds => Outcome::Unreachable,
        other => other,
    };
    Ok(result)
}

fn encode_witness(machine: &Machine<'_>, raw: RawWitness) -> Witness {
    let commands = raw
        .commands
        .into_iter()
        .map(|(action, args)| {
            let action = &machine.model.actions[action];
            let params = action
                .params
                .iter()
                .map(|(name, _)| (name.clone(), machine.data(&args[name])))
                .collect();
            TraceCommand {
                action: action.name.clone(),
                args: params,
            }
        })
        .collect();
    Witness {
        config: machine.configuration_data(&raw.config),
        initial: machine.state_data(&raw.initial),
        commands,
        final_state: machine.state_data(&raw.final_state),
        events: raw.events.map(|events| {
            events
                .into_iter()
                .map(|event| WitnessEvent {
                    step: event.step,
                    event: event.event,
                    observation: event
                        .observation
                        .into_iter()
                        .map(|(name, value)| (name, machine.describe_value(&value)))
                        .collect(),
                })
                .collect()
        }),
    }
}

enum InitialCandidates {
    // Keep checker comparison budgets comparable to rprob.
    Enumerate,
    UseFixed,
}

fn explore<'model>(
    universe: &Universe<'model>,
    budget: SearchBudget,
    initial_candidates: InitialCandidates,
    mut search: Option<&mut WitnessSearch<'_>>,
    mut instances: Option<&mut BTreeMap<usize, Vec<bool>>>,
) -> Report {
    let model = universe.model;
    let ignore_stutter = search.as_ref().is_some_and(|search| {
        matches!(
            search.target,
            WitnessTarget::Deadlock {
                mode: DeadlockMode::IgnoreStutter,
                ..
            }
        )
    });
    let statics: Vec<_> = model.relations.iter().filter(|r| !r.var).collect();
    let vars: Vec<_> = model.relations.iter().filter(|r| r.var).collect();
    let mut found: Vec<Option<usize>> = vec![None; model.checks.len()];
    let mut transition_found = vec![None; model.transitions.len()];
    let mut enabled = vec![false; model.actions.len()];
    let mut broken: Option<usize> = None;
    let mut escaped: Option<usize> = None;
    let unknown = |model: &Model| {
        model
            .checks
            .iter()
            .map(|check| &check.name)
            .chain(model.transitions.iter().map(|check| &check.name))
            .map(|name| Verdict {
                check: name.clone(),
                outcome: Outcome::Unknown,
            })
            .collect::<Vec<_>>()
    };
    let stopped = |stop| Report {
        verdicts: unknown(model),
        declarations: Outcome::Unknown,
        scope: Outcome::Unknown,
        stop: Some(stop),
        enabled: model
            .actions
            .iter()
            .map(|a| (a.name.clone(), None))
            .collect(),
    };
    if let Some((action, sort)) = universe.large_parameter(false) {
        return stopped(StopReason::ArgumentEnumerationLimit {
            action: action.to_owned(),
            sort: sort.to_owned(),
        });
    }
    let fixed = crate::initial::fixed_initial_values(model);
    let candidates = |&relation: &&'model idealis_syntax::Relation| {
        universe
            .candidates(
                &relation.shape,
                fixed.get(relation.name.as_str()).copied(),
                matches!(initial_candidates, InitialCandidates::UseFixed),
            )
            .ok_or_else(|| relation.name.clone())
    };
    let (static_choices, var_choices) = match statics
        .iter()
        .map(candidates)
        .collect::<Result<Vec<_>, _>>()
        .and_then(|static_choices| {
            vars.iter()
                .map(candidates)
                .collect::<Result<Vec<_>, _>>()
                .map(|var_choices| (static_choices, var_choices))
        }) {
        Ok(choices) => choices,
        Err(relation) => return stopped(StopReason::EnumerationLimit { relation }),
    };
    let guard_plans: Vec<_> = model.actions.iter().map(GuardPlan::new).collect();
    let mut explored = 0usize;
    let mut stop = None;
    let mut arguments_left = budget.arguments;
    let mut initial = false;
    let none = BTreeMap::new();
    // The budget counts every configuration and initial candidate examined,
    // not only reached states: their product can be far larger than the
    // state space (seed 6130, plan 0101).
    'configurations: for indices in candidate_indices(&static_choices) {
        if explored == budget.states {
            stop = Some(StopReason::Budget);
            break 'configurations;
        }
        explored += 1;
        let configuration = candidate_values(universe, &static_choices, indices);
        let mut relations: BTreeMap<&str, Value> = statics
            .iter()
            .zip(&configuration)
            .map(|(r, value)| (r.name.as_str(), value.clone()))
            .collect();
        if !model.facts.iter().all(|fact| {
            universe.formula(
                fact,
                &Scene {
                    relations: &relations,
                    after: None,
                    bound: &none,
                },
            )
        }) {
            continue;
        }
        let mut seen: BTreeSet<Vec<Value>> = BTreeSet::new();
        let mut queue: VecDeque<(Vec<Value>, usize)> = VecDeque::new();
        if let Some(search) = search.as_deref_mut() {
            search.parents.clear();
            if let Some(pair) = &mut search.pair {
                pair.groups.clear();
            }
        }
        for indices in candidate_indices(&var_choices) {
            if explored == budget.states {
                stop = Some(StopReason::Budget);
                break 'configurations;
            }
            explored += 1;
            let state = candidate_values(universe, &var_choices, indices);
            for (r, value) in vars.iter().zip(&state) {
                relations.insert(r.name.as_str(), value.clone());
            }
            let scene = Scene {
                relations: &relations,
                after: None,
                bound: &none,
            };
            if model.init.iter().all(|f| universe.formula(f, &scene)) && seen.insert(state.clone())
            {
                initial = true;
                queue.push_back((state, 0));
            }
        }
        while let Some((state, depth)) = queue.pop_front() {
            if explored == budget.states {
                stop = Some(StopReason::Budget);
                break 'configurations;
            }
            explored += 1;
            for (r, value) in vars.iter().zip(&state) {
                relations.insert(r.name.as_str(), value.clone());
            }
            let scene = Scene {
                relations: &relations,
                after: None,
                bound: &none,
            };
            for (index, check) in model.checks.iter().enumerate() {
                let holds = universe.formula(&check.formula, &scene);
                let witness = match check.kind {
                    CheckKind::Invariant => !holds,
                    CheckKind::Reachable => holds,
                };
                if witness {
                    let shorter = found[index].is_none_or(|known| depth < known);
                    if shorter {
                        found[index] = Some(depth);
                    }
                    if let Some(search) = search.as_deref_mut()
                        && search.targets_question(&check.name)
                    {
                        if let Some(pair) = &mut search.pair {
                            pair.visit(
                                &search.parents,
                                &configuration,
                                &state,
                                depth,
                                universe,
                                &scene,
                            );
                        } else if shorter {
                            search.record(&configuration, &state, None);
                        }
                    }
                }
            }
            let mut has_exit = false;
            for (action_index, (action, guards)) in
                model.actions.iter().zip(&guard_plans).enumerate()
            {
                if !guards.before_binding(universe, &scene) {
                    continue;
                }
                let mut reached = instances
                    .as_deref_mut()
                    .and_then(|calls| calls.get_mut(&action_index));
                for (instance, indices) in binding_indices(universe, &action.params).enumerate() {
                    if arguments_left == 0 {
                        stop = Some(StopReason::ArgumentBudget);
                        break 'configurations;
                    }
                    arguments_left -= 1;
                    let bound = bind_values(universe, &action.params, indices);
                    let scene = Scene {
                        relations: &relations,
                        after: None,
                        bound: &bound,
                    };
                    if !guards.after_binding(universe, &scene) {
                        continue;
                    }
                    // Sorted expressions stay in their domains; a broken
                    // multiplicity is a reported violation, not a disabled
                    // transition, and the broken state has no successors
                    // (plan 0099).
                    let mut next = state.clone();
                    let mut valid = true;
                    let mut inside = true;
                    for (var, value) in &action.assignments {
                        let position = vars
                            .iter()
                            .position(|r| &r.name == var)
                            .expect("checked var");
                        let value = universe.expression(value, &scene);
                        if !universe.in_scope(&vars[position].shape, &value) {
                            inside = false;
                        }
                        if !universe.admits_update(&vars[position].shape, &value) {
                            valid = false;
                        }
                        next[position] = value;
                    }
                    // A malformed successor is diagnostic evidence, not a proven stall.
                    has_exit = has_exit || !ignore_stutter || !inside || !valid || next != state;
                    enabled[action_index] = true;
                    if let Some(reached) = &mut reached {
                        // The catalogue uses the same binding order.
                        reached[instance] = true;
                    }
                    // A state with an integer outside the scope is not
                    // explored: the search is no longer complete (plan 0103).
                    if !inside {
                        if escaped.is_none_or(|known| depth + 1 < known) {
                            escaped = Some(depth + 1);
                            if let Some(search) = search.as_deref_mut()
                                && search.targets_question(idealis_syntax::SCOPE)
                            {
                                search.record(
                                    &configuration,
                                    &next,
                                    Some((&state, (action_index, bound.clone()))),
                                );
                            }
                        }
                        continue;
                    }
                    if !valid {
                        if broken.is_none_or(|known| depth + 1 < known) {
                            broken = Some(depth + 1);
                            if let Some(search) = search.as_deref_mut()
                                && search.targets_question(idealis_syntax::DECLARATIONS)
                            {
                                search.record(
                                    &configuration,
                                    &next,
                                    Some((&state, (action_index, bound.clone()))),
                                );
                            }
                        }
                        continue;
                    }
                    if let Some(search) = search.as_deref_mut()
                        && let WitnessTarget::UpdateBranches(branches) = &mut search.target
                    {
                        for branch in &mut branches[action_index] {
                            for site in &branch.sites {
                                if !site.ancestors.iter().all(|(condition, selected)| {
                                    universe.formula(condition, &scene) == *selected
                                }) {
                                    continue;
                                }
                                let arm = usize::from(!universe.formula(site.condition, &scene));
                                let found = &mut branch.found[arm];
                                if found
                                    .as_ref()
                                    .is_none_or(|known| depth + 1 < known.commands.len())
                                {
                                    *found = Some(record_path(
                                        &search.parents,
                                        &configuration,
                                        &next,
                                        Some((&state, (action_index, bound.clone()))),
                                    ));
                                }
                            }
                        }
                    }
                    if !model.transitions.is_empty() {
                        let mut after = relations.clone();
                        for (var, val) in vars.iter().zip(&next) {
                            after.insert(var.name.as_str(), val.clone());
                        }
                        let step = Scene {
                            relations: &relations,
                            after: Some(&after),
                            bound: &bound,
                        };
                        for (check, found) in model.transitions.iter().zip(&mut transition_found) {
                            if check
                                .action
                                .as_ref()
                                .is_none_or(|name| name == &action.name)
                                && found.is_none_or(|known| depth + 1 < known)
                                && !universe.formula(&check.formula, &step)
                            {
                                *found = Some(depth + 1);
                                if let Some(search) = search.as_deref_mut()
                                    && search.targets_question(&check.name)
                                {
                                    search.record(
                                        &configuration,
                                        &next,
                                        Some((&state, (action_index, bound.clone()))),
                                    );
                                }
                            }
                        }
                    }
                    if seen.insert(next.clone()) {
                        if let Some(search) = search.as_deref_mut() {
                            search.parents.insert(
                                next.clone(),
                                PathLink {
                                    before: state.clone(),
                                    call: (action_index, bound.clone()),
                                },
                            );
                        }
                        queue.push_back((next, depth + 1));
                    }
                }
            }
            if !has_exit
                && let Some(search) = search.as_deref_mut()
                && let WitnessTarget::Deadlock {
                    unless,
                    depth: found,
                    ..
                } = &mut search.target
                && found.is_none_or(|known| depth < known)
                && !unless.is_some_and(|condition| universe.formula(condition, &scene))
            {
                *found = Some(depth);
                search.record(&configuration, &state, None);
            }
        }
    }
    let complete = stop.is_none();
    let scope = match (escaped, complete) {
        (Some(depth), _) => Outcome::Violated { depth },
        (None, true) if !initial => Outcome::Empty,
        (None, true) => Outcome::Holds,
        (None, false) => Outcome::Unknown,
    };
    let complete = complete && escaped.is_none();
    let verdicts = model
        .checks
        .iter()
        .zip(found)
        .map(|(check, found): (&Check, Option<usize>)| Verdict {
            check: check.name.clone(),
            outcome: match (check.kind, found, complete) {
                (_, None, true) if !initial => Outcome::Empty,
                (CheckKind::Invariant, Some(depth), _) => Outcome::Violated { depth },
                (CheckKind::Reachable, Some(depth), _) => Outcome::Reachable { depth },
                (_, None, false) => Outcome::Unknown,
                (CheckKind::Invariant, None, true) => Outcome::Holds,
                (CheckKind::Reachable, None, true) => Outcome::Unreachable,
            },
        })
        .chain(
            model
                .transitions
                .iter()
                .zip(transition_found)
                .map(|(check, found)| Verdict {
                    check: check.name.clone(),
                    outcome: match (found, complete) {
                        (Some(depth), _) => Outcome::Violated { depth },
                        (None, false) => Outcome::Unknown,
                        (None, true) if !initial => Outcome::Empty,
                        (None, true) => Outcome::Holds,
                    },
                }),
        )
        .collect();
    Report {
        verdicts,
        declarations: match (broken, complete) {
            (Some(depth), _) => Outcome::Violated { depth },
            (None, true) if !initial => Outcome::Empty,
            (None, true) => Outcome::Holds,
            (None, false) => Outcome::Unknown,
        },
        scope,
        stop,
        enabled: model
            .actions
            .iter()
            .zip(enabled)
            .map(|(action, enabled)| {
                (
                    action.name.clone(),
                    (enabled || (complete && initial)).then_some(enabled),
                )
            })
            .collect(),
    }
}

/// A value in portable form, for drivers of implementations (plan 0102):
/// an atom by name (`User0`, an enum member such as `JPY`, `true`), an
/// integer, or a record with its fields in declaration order.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Data {
    Atom(String),
    Int(i64),
    Record(String, Vec<(String, Data)>),
    Sequence(Vec<Data>),
    Variant(String, Option<Box<Data>>),
}

/// The relations' values of a configuration or a state, opaque outside.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Snapshot(Vec<Value>);

/// Where an action instance leads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Next {
    /// A state within the multiplicities and the integer scope.
    State(Snapshot),
    /// A state that breaks a var's multiplicity (the Declarations check).
    Broken(Snapshot),
    /// A state with an integer outside the scope (the Scope check).
    Outside(Snapshot),
}

/// One action instance from a state: its arguments in parameter order, and
/// the next state when the guards hold.
#[derive(Clone, Debug)]
pub struct Step {
    pub action: String,
    pub args: Vec<(String, Data)>,
    pub next: Option<Next>,
}

/// The reference semantics as a machine to drive step by step: the
/// conformance check replays an implementation along it (plan 0102).
pub struct Machine<'a> {
    runtime: Runtime<'a>,
    static_choices: Vec<Candidates<'a>>,
    var_choices: Vec<Candidates<'a>>,
}

/// Reference evaluation for supplied snapshots and commands.
pub struct Runtime<'a> {
    model: &'a Model,
    universe: Universe<'a>,
    statics: Vec<&'a idealis_syntax::Relation>,
    vars: Vec<&'a idealis_syntax::Relation>,
    names: BTreeMap<u16, String>,
}

impl<'a> std::ops::Deref for Machine<'a> {
    type Target = Runtime<'a>;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

impl<'a> Machine<'a> {
    pub fn new(model: &'a Model) -> Result<Self, String> {
        Self::build(model, true)
    }

    pub fn configuration_data(&self, configuration: &Snapshot) -> RelationData {
        self.runtime.configuration_data(configuration)
    }

    pub fn state_data(&self, state: &Snapshot) -> RelationData {
        self.runtime.state_data(state)
    }

    pub fn violated_invariant(&self, configuration: &Snapshot, state: &Snapshot) -> Option<&str> {
        self.runtime.violated_invariant(configuration, state)
    }

    pub fn violated_transition(
        &self,
        configuration: &Snapshot,
        before: &Snapshot,
        after: &Snapshot,
    ) -> Option<&str> {
        self.runtime
            .violated_transition(configuration, before, after)
    }

    fn for_search(model: &'a Model) -> Result<Self, String> {
        Self::build(model, false)
    }

    fn build(model: &'a Model, collect_initial: bool) -> Result<Self, String> {
        let runtime = Runtime::new(model);
        check_enumerable(model, &runtime.universe, collect_initial)?;
        let fixed = crate::initial::fixed_initial_values(model);
        let choices = |relations: &[&'a idealis_syntax::Relation]| {
            relations
                .iter()
                .map(|r| {
                    runtime
                        .universe
                        .candidates(&r.shape, fixed.get(r.name.as_str()).copied(), true)
                        .expect("enumerable")
                })
                .collect::<Vec<_>>()
        };
        let (static_choices, var_choices) = (choices(&runtime.statics), choices(&runtime.vars));
        Ok(Self {
            runtime,
            static_choices,
            var_choices,
        })
    }
    /// Every configuration that satisfies the facts, one at a time.
    pub fn configurations(&self) -> impl Iterator<Item = Snapshot> + '_ {
        candidate_indices(&self.static_choices).filter_map(move |indices| {
            let values = candidate_values(&self.universe, &self.static_choices, indices);
            self.accept_configuration(Snapshot(values))
        })
    }

    /// The initial states of a configuration.
    pub fn initial(&self, configuration: &Snapshot) -> Vec<Snapshot> {
        candidate_indices(&self.var_choices)
            .filter_map(|indices| {
                let values = candidate_values(&self.universe, &self.var_choices, indices);
                self.accept_initial(configuration, Snapshot(values))
            })
            .collect()
    }
    /// Select one configuration by its relation values and verify its facts.
    /// This does not traverse the product of all possible configurations.
    pub fn match_configuration(
        &self,
        matches: impl FnMut(&str, Vec<Vec<Data>>) -> bool,
    ) -> Option<Snapshot> {
        let configuration = self.match_snapshot(&self.statics, &self.static_choices, matches)?;
        self.accept_configuration(configuration)
    }

    /// Select one supplied initial state and check its initial predicates.
    pub fn match_initial(
        &self,
        configuration: &Snapshot,
        matches: impl FnMut(&str, Vec<Vec<Data>>) -> bool,
    ) -> Option<Snapshot> {
        let state = self.match_snapshot(&self.vars, &self.var_choices, matches)?;
        self.accept_initial(configuration, state)
    }

    fn match_snapshot(
        &self,
        relations: &[&idealis_syntax::Relation],
        choices: &[Candidates<'_>],
        mut matches: impl FnMut(&str, Vec<Vec<Data>>) -> bool,
    ) -> Option<Snapshot> {
        relations
            .iter()
            .zip(choices)
            .map(|(relation, candidates)| {
                (0..candidates.len())
                    .map(|index| candidates.at(index, &self.universe))
                    .find(|value| {
                        let tuples = value
                            .iter()
                            .map(|tuple| tuple.iter().map(|atom| self.data(atom)).collect())
                            .collect();
                        matches(&relation.name, tuples)
                    })
            })
            .collect::<Option<Vec<_>>>()
            .map(Snapshot)
    }

    /// Check transition properties with the identity and arguments of an accepted step.
    pub fn violated_action_transition(
        &self,
        configuration: &Snapshot,
        before: &Snapshot,
        after: &Snapshot,
        action: &str,
        matches: impl FnMut(&str, Data) -> bool,
    ) -> Option<&str> {
        if self
            .model
            .transitions
            .iter()
            .all(|check| check.action.is_none())
        {
            return self.violated_transition(configuration, before, after);
        }
        let (_, bound) = self.bind_action(action, matches)?;
        self.violated_transition_for(configuration, before, after, Some(action), &bound)
    }

    /// Every action instance from a state, in declaration and argument order.
    pub fn steps(&self, configuration: &Snapshot, state: &Snapshot) -> Vec<Step> {
        let relations = self.relations(&configuration.0, &state.0);
        let mut steps = Vec::new();
        for action in &self.model.actions {
            for bound in bindings(&self.universe, &action.params) {
                let scene = Scene {
                    relations: &relations,
                    after: None,
                    bound: &bound,
                };
                let args = action
                    .params
                    .iter()
                    .map(|(param, _)| (param.clone(), self.data(&bound[param])))
                    .collect();
                let next = self.next(action, &scene, state);
                steps.push(Step {
                    action: action.name.clone(),
                    args,
                    next,
                });
            }
        }
        steps
    }
    /// Enumerate named argument bindings within the remaining grounding budget.
    pub fn action_arguments(
        &self,
        action: &idealis_syntax::Action,
        limit: usize,
    ) -> Option<Vec<Vec<(String, Data)>>> {
        let size = self.universe.argument_count(&action.params)?;
        if size > limit {
            return None;
        }
        if size == 0 {
            return Some(Vec::new());
        }
        Some(
            bindings(&self.universe, &action.params)
                .map(|bound| {
                    action
                        .params
                        .iter()
                        .map(|(name, _)| (name.clone(), self.data(&bound[name])))
                        .collect()
                })
                .collect(),
        )
    }
    /// Resolve just the supplied action and arguments. Unlike `steps`,
    /// this does not enumerate the product of action bindings. The outer
    /// option matches the request; the inner option indicates guard acceptance.
    pub fn match_step(
        &self,
        configuration: &Snapshot,
        state: &Snapshot,
        name: &str,
        matches: impl FnMut(&str, Data) -> bool,
    ) -> Option<Option<Next>> {
        let (action, bound) = self.bind_action(name, matches)?;
        let relations = self.relations(&configuration.0, &state.0);
        let scene = Scene {
            relations: &relations,
            after: None,
            bound: &bound,
        };
        Some(self.next(action, &scene, state))
    }

    fn bind_action(
        &self,
        name: &str,
        mut matches: impl FnMut(&str, Data) -> bool,
    ) -> Option<(&idealis_syntax::Action, BTreeMap<String, Atom>)> {
        let action = self
            .model
            .actions
            .iter()
            .find(|action| action.name == name)?;
        if self.universe.argument_count(&action.params) == Some(0) {
            return None;
        }
        let mut bound = BTreeMap::new();
        for (param, sort) in &action.params {
            let atom = self
                .universe
                .atoms(sort)
                .iter()
                .find(|atom| matches(param, self.data(atom)))?;
            bound.insert(param.clone(), atom.clone());
        }
        Some((action, bound))
    }
}

impl<'a> Runtime<'a> {
    pub fn new(model: &'a Model) -> Self {
        Self {
            model,
            universe: Universe::new(model),
            statics: model.relations.iter().filter(|r| !r.var).collect(),
            vars: model.vars().collect(),
            names: atom_names(model),
        }
    }

    fn accept_configuration(&self, configuration: Snapshot) -> Option<Snapshot> {
        let relations = self.relations(&configuration.0, &[]);
        let scene = Scene {
            relations: &relations,
            after: None,
            bound: &BTreeMap::new(),
        };
        self.model
            .facts
            .iter()
            .all(|fact| self.universe.formula(fact, &scene))
            .then_some(configuration)
    }

    fn accept_initial(&self, configuration: &Snapshot, state: Snapshot) -> Option<Snapshot> {
        let relations = self.relations(&configuration.0, &state.0);
        let scene = Scene {
            relations: &relations,
            after: None,
            bound: &BTreeMap::new(),
        };
        self.model
            .init
            .iter()
            .all(|formula| self.universe.formula(formula, &scene))
            .then_some(state)
    }

    /// The first false invariant of a given valid state.
    pub fn violated_invariant(&self, configuration: &Snapshot, state: &Snapshot) -> Option<&str> {
        let relations = self.relations(&configuration.0, &state.0);
        let scene = Scene {
            relations: &relations,
            after: None,
            bound: &BTreeMap::new(),
        };
        self.model
            .checks
            .iter()
            .find(|check| {
                check.kind == CheckKind::Invariant && !self.universe.formula(&check.formula, &scene)
            })
            .map(|check| check.name.as_str())
    }

    /// The first false unscoped property of an accepted, well-formed step.
    pub fn violated_transition(
        &self,
        configuration: &Snapshot,
        before: &Snapshot,
        after: &Snapshot,
    ) -> Option<&str> {
        self.violated_transition_for(configuration, before, after, None, &BTreeMap::new())
    }

    fn violated_transition_for(
        &self,
        configuration: &Snapshot,
        before: &Snapshot,
        after: &Snapshot,
        action: Option<&str>,
        bound: &BTreeMap<String, Atom>,
    ) -> Option<&str> {
        if self.model.transitions.is_empty() {
            return None;
        }
        let old = self.relations(&configuration.0, &before.0);
        let new = self.relations(&configuration.0, &after.0);
        let scene = Scene {
            relations: &old,
            after: Some(&new),
            bound,
        };
        self.model
            .transitions
            .iter()
            .find(|check| {
                check
                    .action
                    .as_deref()
                    .is_none_or(|name| Some(name) == action)
                    && !self.universe.formula(&check.formula, &scene)
            })
            .map(|check| check.name.as_str())
    }

    fn next(
        &self,
        action: &idealis_syntax::Action,
        scene: &Scene<'_>,
        state: &Snapshot,
    ) -> Option<Next> {
        if !action
            .guards
            .iter()
            .all(|guard| self.universe.formula(guard, scene))
        {
            return None;
        }
        Some(self.advance(action, scene, state))
    }

    fn advance(
        &self,
        action: &idealis_syntax::Action,
        scene: &Scene<'_>,
        state: &Snapshot,
    ) -> Next {
        let mut next = state.0.clone();
        let (mut valid, mut inside) = (true, true);
        for (var, value) in &action.assignments {
            let position = self
                .vars
                .iter()
                .position(|r| &r.name == var)
                .expect("checked var");
            let value = self.universe.expression(value, scene);
            inside &= self.universe.in_scope(&self.vars[position].shape, &value);
            valid &= self
                .universe
                .admits_update(&self.vars[position].shape, &value);
            next[position] = value;
        }
        match (inside, valid) {
            (false, _) => Next::Outside(Snapshot(next)),
            (true, false) => Next::Broken(Snapshot(next)),
            (true, true) => Next::State(Snapshot(next)),
        }
    }

    /// The static relations' values, by name, as sets of tuples.
    pub fn configuration_data(&self, configuration: &Snapshot) -> RelationData {
        self.describe(&self.statics, &configuration.0)
    }
    /// The vars' values, by name, as sets of tuples.
    pub fn state_data(&self, state: &Snapshot) -> RelationData {
        self.describe(&self.vars, &state.0)
    }
    /// Evaluate named derivations at an accepted snapshot.
    pub fn derived_data(
        &self,
        configuration: &Snapshot,
        state: &Snapshot,
        names: &[String],
    ) -> Result<RelationData, String> {
        if names.iter().collect::<BTreeSet<_>>().len() != names.len() {
            return Err("derive names must be distinct".into());
        }
        let relations = self.relations(&configuration.0, &state.0);
        let scene = Scene {
            relations: &relations,
            after: None,
            bound: &BTreeMap::new(),
        };
        names
            .iter()
            .map(|name| {
                let (_, expression) = self
                    .model
                    .derivations
                    .iter()
                    .find(|(derived, _)| derived == name)
                    .ok_or_else(|| format!("unknown derivation {name}"))?;
                Ok((
                    name.clone(),
                    self.describe_value(&self.universe.expression(expression, &scene)),
                ))
            })
            .collect()
    }
    fn describe(&self, relations: &[&idealis_syntax::Relation], values: &[Value]) -> RelationData {
        relations
            .iter()
            .zip(values)
            .map(|(relation, value)| self.describe_relation(relation, value))
            .collect()
    }
    fn describe_relation(
        &self,
        relation: &idealis_syntax::Relation,
        value: &Value,
    ) -> (String, Vec<Vec<Data>>) {
        (relation.name.clone(), self.describe_value(value))
    }
    fn describe_value(&self, value: &Value) -> Vec<Vec<Data>> {
        value
            .iter()
            .map(|tuple| tuple.iter().map(|atom| self.data(atom)).collect())
            .collect()
    }
    fn data(&self, atom: &Atom) -> Data {
        atom_data(self.model, &self.names, atom)
    }
    fn relations(&self, configuration: &[Value], state: &[Value]) -> BTreeMap<&'a str, Value> {
        self.statics
            .iter()
            .zip(configuration)
            .chain(self.vars.iter().zip(state))
            .map(|(r, value)| (r.name.as_str(), value.clone()))
            .collect()
    }
}

/// One candidate for a generated input decoder, independent of the search scope.
pub struct DataBoundary {
    pub sort: String,
    pub input: Data,
    pub valid: bool,
}

/// Probe just below, at and just above each written comparison's boundary.
/// A valid value from the finite domain provides the other fields.
pub fn data_boundaries(model: &Model) -> Vec<DataBoundary> {
    let universe = Universe::new(model);
    let names = atom_names(model);
    let mut cases = Vec::new();
    for record in model.records.iter().filter(|r| !r.refinements.is_empty()) {
        let seeds = universe.atoms(&record.name);
        for clause in &record.refinements {
            for (vary, bound) in [(&clause.left, &clause.right), (&clause.right, &clause.left)] {
                let idealis_syntax::RefinementOperand::Field(path) = vary else {
                    continue;
                };
                let candidates = |seed: &Atom| {
                    let pivot = match bound {
                        idealis_syntax::RefinementOperand::Literal(n) => *n,
                        idealis_syntax::RefinementOperand::Field(path) => {
                            universe.refinement_field(seed, path)
                        }
                    };
                    let mut probes = Vec::new();
                    for delta in [-1i64, 0, 1] {
                        let Some(n) = pivot.checked_add(delta) else {
                            continue;
                        };
                        if n.unsigned_abs() > 9_007_199_254_740_991 {
                            continue;
                        }
                        let mut candidate = seed.clone();
                        let mut value = &mut candidate;
                        for field in path {
                            let Atom::Record(record, fields) = value else {
                                unreachable!("checked field path")
                            };
                            let index = model.records[usize::from(*record)]
                                .field_index(field)
                                .unwrap();
                            value = &mut fields[index];
                        }
                        *value = Atom::Int(n);
                        let valid = universe.valid_data(&candidate);
                        probes.push((candidate, valid));
                    }
                    probes
                };
                // Fixed boundary predicates need just one seed. For dependent
                // bounds try the first few values to avoid a non-discriminating
                // example (for example, a condition already implied by Nat).
                for seed in seeds.iter().take(20).skip(1).chain(seeds.first()) {
                    let probes = candidates(seed);
                    if probes.iter().any(|(_, valid)| *valid)
                        && probes.iter().any(|(_, valid)| !*valid)
                    {
                        cases.extend(probes.into_iter().map(|(atom, valid)| DataBoundary {
                            sort: record.name.clone(),
                            input: atom_data(model, &names, &atom),
                            valid,
                        }));
                        break;
                    }
                }
            }
        }
    }
    cases
}

fn atom_names(model: &Model) -> BTreeMap<u16, String> {
    // Atom ids follow Universe::new: signatures, then enum members.
    let mut names = BTreeMap::new();
    let mut next = 0u16;
    for sig in &model.sigs {
        for index in 0..model.scope.sizes[sig] {
            names.insert(next, format!("{sig}{index}"));
            next += 1;
        }
    }
    for (_, members) in &model.enums {
        for member in members {
            names.insert(next, member.clone());
            next += 1;
        }
    }
    names
}

fn atom_data(model: &Model, names: &BTreeMap<u16, String>, atom: &Atom) -> Data {
    match atom {
        Atom::Id(id) => Data::Atom(names[id].clone()),
        Atom::Int(value) => Data::Int(*value),
        Atom::Sequence(items) => Data::Sequence(
            items
                .iter()
                .map(|item| atom_data(model, names, item))
                .collect(),
        ),
        Atom::Variant(tag, value) => Data::Variant(
            tag.clone(),
            value.as_ref().map(|v| Box::new(atom_data(model, names, v))),
        ),
        Atom::Record(position, fields) => {
            let record = &model.records[*position as usize];
            Data::Record(
                record.name.clone(),
                record
                    .fields
                    .iter()
                    .zip(fields.iter())
                    .map(|((field, _), value)| (field.clone(), atom_data(model, names, value)))
                    .collect(),
            )
        }
    }
}
