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
use crate::syntax::{
    Arith, Check, CheckKind, Compare, Expr, Formula, Int, IntCompare, Model, Mult, Quantifier,
    SetOp, Shape,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

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
}
type Tuple = Vec<Atom>;
type Value = BTreeSet<Tuple>;

/// A domain larger than this many tuples is not enumerated: its subsets
/// alone would exhaust any budget.
const MAX_DOMAIN: usize = 20;

struct Universe<'a> {
    /// Atoms of each signature and enum.
    sorts: BTreeMap<&'a str, Vec<Atom>>,
    /// The atom of each enum member.
    members: BTreeMap<&'a str, Atom>,
    /// The scope's integers, `lo..=hi`.
    ints: Option<(i64, i64)>,
    model: &'a Model,
}

impl<'a> Universe<'a> {
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
            sorts.insert(crate::syntax::INT, (low..=high).map(Atom::Int).collect());
        }
        for (position, record) in model.records.iter().enumerate() {
            let mut all: Vec<Vec<Atom>> = vec![Vec::new()];
            for (_, sort) in &record.fields {
                all = all
                    .into_iter()
                    .flat_map(|prefix| {
                        sorts[sort.as_str()].iter().map(move |atom: &Atom| {
                            let mut next = prefix.clone();
                            next.push(atom.clone());
                            next
                        })
                    })
                    .collect();
            }
            sorts.insert(
                record.name.as_str(),
                all.into_iter()
                    .map(|fields| Atom::Record(position as u16, fields.into()))
                    .collect(),
            );
        }
        Self {
            sorts,
            members,
            ints: model.scope.ints,
            model,
        }
    }
    fn domain(&self, shape: &Shape) -> Vec<Tuple> {
        match shape {
            Shape::Unary { sig, .. } => self.sorts[sig.as_str()]
                .iter()
                .map(|a| vec![a.clone()])
                .collect(),
            Shape::Binary { from, to, .. } => self.sorts[from.as_str()]
                .iter()
                .flat_map(|a| {
                    self.sorts[to.as_str()]
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
            Shape::Binary { from, mult, .. } => self.sorts[from.as_str()]
                .iter()
                .all(|a| fits(*mult, value.iter().filter(|t| &t[0] == a).count())),
        }
    }
    /// Every value of a relation that meets its type and multiplicity, or
    /// `None` when the domain is too large to enumerate.
    /// Whether every integer in a value lies in the scope (plan 0103).
    fn in_scope(&self, value: &Value) -> bool {
        fn inside(ints: Option<(i64, i64)>, atom: &Atom) -> bool {
            match atom {
                Atom::Id(_) => true,
                Atom::Int(value) => ints.is_some_and(|(low, high)| (low..=high).contains(value)),
                Atom::Record(_, fields) => fields.iter().all(|field| inside(ints, field)),
            }
        }
        value
            .iter()
            .all(|tuple| tuple.iter().all(|a| inside(self.ints, a)))
    }
    /// The only integer of a set the checker proved singular.
    fn int(&self, value: &Value) -> i64 {
        match value.iter().next().map(|tuple| &tuple[0]) {
            Some(Atom::Int(value)) => *value,
            other => unreachable!("one integer by the checker, got {other:?}"),
        }
    }
    fn candidates(&self, shape: &Shape) -> Option<Vec<Value>> {
        let domain = self.domain(shape);
        if domain.len() > MAX_DOMAIN {
            return None;
        }
        Some(
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
        )
    }
}

/// The relations' values and the atoms bound by quantifiers and parameters.
struct Scene<'s> {
    relations: &'s BTreeMap<&'s str, Value>,
    bound: &'s BTreeMap<String, Atom>,
}

fn singleton(atom: Atom) -> Value {
    [vec![atom]].into()
}

impl Universe<'_> {
    fn expression(&self, expression: &Expr, scene: &Scene<'_>) -> Value {
        match expression {
            Expr::None => Value::new(),
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
            Expr::Literal(value) => singleton(Atom::Int(*value)),
            Expr::Count(counted) => {
                singleton(Atom::Int(self.expression(counted, scene).len() as i64))
            }
            Expr::Arith(operator, left, right) => {
                let (left, right) = (
                    self.int(&self.expression(left, scene)),
                    self.int(&self.expression(right, scene)),
                );
                // Mathematical integers: an i64 overflow is far outside any
                // scope, so it saturates and leaves the scope.
                singleton(Atom::Int(match operator {
                    Arith::Add => left.saturating_add(right),
                    Arith::Sub => left.saturating_sub(right),
                    Arith::Mul => left.saturating_mul(right),
                }))
            }
            Expr::Record(name, values) => {
                let position = self
                    .model
                    .records
                    .iter()
                    .position(|record| &record.name == name)
                    .expect("a declared record") as u16;
                let fields: Vec<Atom> = values
                    .iter()
                    .map(|value| {
                        let value = self.expression(value, scene);
                        value.into_iter().next().expect("one value by the checker")[0].clone()
                    })
                    .collect();
                singleton(Atom::Record(position, fields.into()))
            }
            Expr::Binary(SetOp::Join, left, right)
                if matches!(right.as_ref(), Expr::Name(name)
                    if !scene.bound.contains_key(name) && self.model.field(name).is_some()) =>
            {
                let Expr::Name(field) = right.as_ref() else {
                    unreachable!()
                };
                let (_, index) = self.model.field(field).expect("a field");
                self.expression(left, scene)
                    .into_iter()
                    .map(|tuple| match &tuple[0] {
                        Atom::Record(_, fields) => vec![fields[index].clone()],
                        other => unreachable!("a record by the checker, got {other:?}"),
                    })
                    .collect()
            }
            Expr::Binary(operator, left, right) => {
                let (left, right) = (self.expression(left, scene), self.expression(right, scene));
                match operator {
                    SetOp::Union => left.union(&right).cloned().collect(),
                    SetOp::Difference => left.difference(&right).cloned().collect(),
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
                        Int::Value(value) => self.int(&self.expression(value, scene)),
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
            Formula::Empty(counted) => self.expression(counted, scene).is_empty(),
            Formula::Not(inner) => !self.formula(inner, scene),
            Formula::And(parts) => parts.iter().all(|part| self.formula(part, scene)),
            Formula::Or(left, right) => self.formula(left, scene) || self.formula(right, scene),
            Formula::Implies(left, right) => {
                !self.formula(left, scene) || self.formula(right, scene)
            }
            Formula::Quantified(quantifier, variable, sig, body) => {
                let test = |atom: &Atom| {
                    let mut bound = scene.bound.clone();
                    bound.insert(variable.clone(), atom.clone());
                    self.formula(
                        body,
                        &Scene {
                            relations: scene.relations,
                            bound: &bound,
                        },
                    )
                };
                let atoms = &self.sorts[sig.as_str()];
                match quantifier {
                    Quantifier::All => atoms.iter().all(test),
                    Quantifier::Some => atoms.iter().any(test),
                }
            }
        }
    }
}

/// Every combination of one value per relation, produced one at a time:
/// materializing them exhausted memory before the budget applied (seed
/// 5141, plan 0101).
fn product(choices: &[Vec<Value>]) -> impl Iterator<Item = Vec<Value>> + '_ {
    let mut indices = vec![0usize; choices.len()];
    let mut done = choices.iter().any(Vec::is_empty);
    std::iter::from_fn(move || {
        if done {
            return None;
        }
        let item = indices
            .iter()
            .zip(choices)
            .map(|(&index, options)| options[index].clone())
            .collect();
        // Advance the last position fastest, as the nested loops did.
        done = true;
        for position in (0..choices.len()).rev() {
            indices[position] += 1;
            if indices[position] < choices[position].len() {
                done = false;
                break;
            }
            indices[position] = 0;
        }
        Some(item)
    })
}

/// Every binding of a parameter list to atoms of its sorts.
fn bindings(universe: &Universe<'_>, params: &[(String, String)]) -> Vec<BTreeMap<String, Atom>> {
    let mut all = vec![BTreeMap::new()];
    for (name, sig) in params {
        all = all
            .into_iter()
            .flat_map(|prefix| {
                universe.sorts[sig.as_str()].iter().map(move |atom| {
                    let mut next = prefix.clone();
                    next.insert(name.clone(), atom.clone());
                    next
                })
            })
            .collect();
    }
    all
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
    /// Whether a transition breaks a declared multiplicity (plan 0099):
    /// `Violated` with the shortest depth of the breaking state, `Holds`,
    /// `Empty` or `Unknown`.
    pub declarations: Outcome,
    /// Whether a state leaves the scope's integers (plan 0103): `Violated`
    /// with the shortest depth of such a state. Leaving makes the search
    /// incomplete, so checks without a witness are then `Unknown`.
    pub scope: Outcome,
    pub enabled: Vec<(String, Option<bool>)>,
}

/// Whether every relation's values can be enumerated, or the first that
/// cannot (more than `MAX_DOMAIN` tuples in its domain).
pub fn enumerable(model: &Model) -> Result<(), String> {
    let universe = Universe::new(model);
    for relation in &model.relations {
        let size = universe.domain(&relation.shape).len();
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
    let universe = Universe::new(model);
    let statics: Vec<_> = model.relations.iter().filter(|r| !r.var).collect();
    let vars: Vec<_> = model.relations.iter().filter(|r| r.var).collect();
    let mut found: Vec<Option<usize>> = vec![None; model.checks.len()];
    let mut enabled = vec![false; model.actions.len()];
    let mut broken: Option<usize> = None;
    let mut escaped: Option<usize> = None;
    let unknown = |model: &Model| {
        model
            .checks
            .iter()
            .map(|check| Verdict {
                check: check.name.clone(),
                outcome: Outcome::Unknown,
            })
            .collect::<Vec<_>>()
    };
    let (Some(static_choices), Some(var_choices)) = (
        statics
            .iter()
            .map(|r| universe.candidates(&r.shape))
            .collect::<Option<Vec<_>>>(),
        vars.iter()
            .map(|r| universe.candidates(&r.shape))
            .collect::<Option<Vec<_>>>(),
    ) else {
        return Report {
            verdicts: unknown(model),
            declarations: Outcome::Unknown,
            scope: Outcome::Unknown,
            enabled: model
                .actions
                .iter()
                .map(|a| (a.name.clone(), None))
                .collect(),
        };
    };
    let mut explored = 0usize;
    let mut complete = true;
    let mut initial = false;
    let none = BTreeMap::new();
    // The budget counts every configuration and initial candidate examined,
    // not only reached states: their product can be far larger than the
    // state space (seed 6130, plan 0101).
    'configurations: for configuration in product(&static_choices) {
        explored += 1;
        if explored > max_states {
            complete = false;
            break 'configurations;
        }
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
                    bound: &none,
                },
            )
        }) {
            continue;
        }
        let mut seen: BTreeSet<Vec<Value>> = BTreeSet::new();
        let mut queue: VecDeque<(Vec<Value>, usize)> = VecDeque::new();
        for state in product(&var_choices) {
            explored += 1;
            if explored > max_states {
                complete = false;
                break 'configurations;
            }
            for (r, value) in vars.iter().zip(&state) {
                relations.insert(r.name.as_str(), value.clone());
            }
            let scene = Scene {
                relations: &relations,
                bound: &none,
            };
            if model.init.iter().all(|f| universe.formula(f, &scene)) && seen.insert(state.clone())
            {
                initial = true;
                queue.push_back((state, 0));
            }
        }
        while let Some((state, depth)) = queue.pop_front() {
            explored += 1;
            if explored > max_states {
                complete = false;
                break 'configurations;
            }
            for (r, value) in vars.iter().zip(&state) {
                relations.insert(r.name.as_str(), value.clone());
            }
            let scene = Scene {
                relations: &relations,
                bound: &none,
            };
            for (index, check) in model.checks.iter().enumerate() {
                let holds = universe.formula(&check.formula, &scene);
                let witness = match check.kind {
                    CheckKind::Invariant => !holds,
                    CheckKind::Reachable => holds,
                };
                if witness && found[index].is_none_or(|known| depth < known) {
                    found[index] = Some(depth);
                }
            }
            for (action_index, action) in model.actions.iter().enumerate() {
                for bound in bindings(&universe, &action.params) {
                    let scene = Scene {
                        relations: &relations,
                        bound: &bound,
                    };
                    if !action.guards.iter().all(|g| universe.formula(g, &scene)) {
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
                        if !universe.in_scope(&value) {
                            inside = false;
                        }
                        if !universe.admits(&vars[position].shape, &value) {
                            valid = false;
                        }
                        next[position] = value;
                    }
                    enabled[action_index] = true;
                    // A state with an integer outside the scope is not
                    // explored: the search is no longer complete (plan 0103).
                    if !inside {
                        if escaped.is_none_or(|known| depth + 1 < known) {
                            escaped = Some(depth + 1);
                        }
                        continue;
                    }
                    if !valid {
                        if broken.is_none_or(|known| depth + 1 < known) {
                            broken = Some(depth + 1);
                        }
                        continue;
                    }
                    if seen.insert(next.clone()) {
                        queue.push_back((next, depth + 1));
                    }
                }
            }
        }
    }
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
    model: &'a Model,
    universe: Universe<'a>,
    statics: Vec<&'a crate::syntax::Relation>,
    vars: Vec<&'a crate::syntax::Relation>,
    static_choices: Vec<Vec<Value>>,
    var_choices: Vec<Vec<Value>>,
    names: BTreeMap<u16, String>,
}

impl<'a> Machine<'a> {
    pub fn new(model: &'a Model) -> Result<Self, String> {
        enumerable(model)?;
        let universe = Universe::new(model);
        let statics: Vec<_> = model.relations.iter().filter(|r| !r.var).collect();
        let vars: Vec<_> = model.relations.iter().filter(|r| r.var).collect();
        let choices = |relations: &[&crate::syntax::Relation]| {
            relations
                .iter()
                .map(|r| universe.candidates(&r.shape).expect("enumerable"))
                .collect::<Vec<_>>()
        };
        let (static_choices, var_choices) = (choices(&statics), choices(&vars));
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
        Ok(Self {
            model,
            universe,
            statics,
            vars,
            static_choices,
            var_choices,
            names,
        })
    }
    /// Every configuration that satisfies the facts, one at a time.
    pub fn configurations(&self) -> impl Iterator<Item = Snapshot> + '_ {
        let none = BTreeMap::new();
        product(&self.static_choices).filter_map(move |configuration| {
            let relations = self.relations(&configuration, &[]);
            let scene = Scene {
                relations: &relations,
                bound: &none,
            };
            self.model
                .facts
                .iter()
                .all(|fact| self.universe.formula(fact, &scene))
                .then_some(Snapshot(configuration))
        })
    }
    /// The initial states of a configuration.
    pub fn initial(&self, configuration: &Snapshot) -> Vec<Snapshot> {
        let none = BTreeMap::new();
        product(&self.var_choices)
            .filter(|state| {
                let relations = self.relations(&configuration.0, state);
                let scene = Scene {
                    relations: &relations,
                    bound: &none,
                };
                self.model
                    .init
                    .iter()
                    .all(|formula| self.universe.formula(formula, &scene))
            })
            .map(Snapshot)
            .collect()
    }
    /// Every action instance from a state, in declaration and argument
    /// order.
    pub fn steps(&self, configuration: &Snapshot, state: &Snapshot) -> Vec<Step> {
        let relations = self.relations(&configuration.0, &state.0);
        let mut steps = Vec::new();
        for action in &self.model.actions {
            for bound in bindings(&self.universe, &action.params) {
                let scene = Scene {
                    relations: &relations,
                    bound: &bound,
                };
                let args = action
                    .params
                    .iter()
                    .map(|(param, _)| (param.clone(), self.data(&bound[param])))
                    .collect();
                let next = action
                    .guards
                    .iter()
                    .all(|guard| self.universe.formula(guard, &scene))
                    .then(|| {
                        let mut next = state.0.clone();
                        let (mut valid, mut inside) = (true, true);
                        for (var, value) in &action.assignments {
                            let position = self
                                .vars
                                .iter()
                                .position(|r| &r.name == var)
                                .expect("checked var");
                            let value = self.universe.expression(value, &scene);
                            inside &= self.universe.in_scope(&value);
                            valid &= self.universe.admits(&self.vars[position].shape, &value);
                            next[position] = value;
                        }
                        match (inside, valid) {
                            (false, _) => Next::Outside(Snapshot(next)),
                            (true, false) => Next::Broken(Snapshot(next)),
                            (true, true) => Next::State(Snapshot(next)),
                        }
                    });
                steps.push(Step {
                    action: action.name.clone(),
                    args,
                    next,
                });
            }
        }
        steps
    }
    /// The static relations' values, by name, as sets of tuples.
    pub fn configuration_data(&self, configuration: &Snapshot) -> Vec<(String, Vec<Vec<Data>>)> {
        self.describe(&self.statics, &configuration.0)
    }
    /// The vars' values, by name, as sets of tuples.
    pub fn state_data(&self, state: &Snapshot) -> Vec<(String, Vec<Vec<Data>>)> {
        self.describe(&self.vars, &state.0)
    }
    fn describe(
        &self,
        relations: &[&crate::syntax::Relation],
        values: &[Value],
    ) -> Vec<(String, Vec<Vec<Data>>)> {
        relations
            .iter()
            .zip(values)
            .map(|(relation, value)| {
                let tuples = value
                    .iter()
                    .map(|tuple| tuple.iter().map(|atom| self.data(atom)).collect())
                    .collect();
                (relation.name.clone(), tuples)
            })
            .collect()
    }
    fn data(&self, atom: &Atom) -> Data {
        match atom {
            Atom::Id(id) => Data::Atom(self.names[id].clone()),
            Atom::Int(value) => Data::Int(*value),
            Atom::Record(position, fields) => {
                let record = &self.model.records[*position as usize];
                Data::Record(
                    record.name.clone(),
                    record
                        .fields
                        .iter()
                        .zip(fields.iter())
                        .map(|((field, _), value)| (field.clone(), self.data(value)))
                        .collect(),
                )
            }
        }
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
