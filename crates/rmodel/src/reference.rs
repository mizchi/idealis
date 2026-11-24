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
    Check, CheckKind, Compare, Expr, Formula, Int, IntCompare, Model, Mult, Quantifier, SetOp,
    Shape,
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

type Atom = u16;
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
}

impl<'a> Universe<'a> {
    fn new(model: &'a Model) -> Self {
        let mut next: Atom = 0;
        let mut sorts = BTreeMap::new();
        let mut members = BTreeMap::new();
        for sig in &model.sigs {
            let size = model.scope.sizes[sig];
            sorts.insert(sig.as_str(), (next..next + size as Atom).collect());
            next += size as Atom;
        }
        for (name, enum_members) in &model.enums {
            let mut atoms = Vec::new();
            for member in enum_members {
                members.insert(member.as_str(), next);
                atoms.push(next);
                next += 1;
            }
            sorts.insert(name.as_str(), atoms);
        }
        Self { sorts, members }
    }
    fn domain(&self, shape: &Shape) -> Vec<Tuple> {
        match shape {
            Shape::Unary { sig, .. } => self.sorts[sig.as_str()].iter().map(|&a| vec![a]).collect(),
            Shape::Binary { from, to, .. } => self.sorts[from.as_str()]
                .iter()
                .flat_map(|&a| self.sorts[to.as_str()].iter().map(move |&b| vec![a, b]))
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
                .all(|&a| fits(*mult, value.iter().filter(|t| t[0] == a).count())),
        }
    }
    /// Every value of a relation that meets its type and multiplicity, or
    /// `None` when the domain is too large to enumerate.
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

impl Universe<'_> {
    fn expression(&self, expression: &Expr, scene: &Scene<'_>) -> Value {
        match expression {
            Expr::None => Value::new(),
            Expr::Name(name) => {
                if let Some(&atom) = scene.bound.get(name) {
                    return [vec![atom]].into();
                }
                if let Some(value) = scene.relations.get(name.as_str()) {
                    return value.clone();
                }
                if let Some(&atom) = self.members.get(name.as_str()) {
                    return [vec![atom]].into();
                }
                self.sorts[name.as_str()].iter().map(|&a| vec![a]).collect()
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
                let test = |atom: Atom| {
                    let mut bound = scene.bound.clone();
                    bound.insert(variable.clone(), atom);
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
                    Quantifier::All => atoms.iter().all(|&a| test(a)),
                    Quantifier::Some => atoms.iter().any(|&a| test(a)),
                }
            }
        }
    }
}

/// Every combination of one value per relation.
fn product(choices: &[Vec<Value>]) -> Vec<Vec<Value>> {
    let mut all = vec![Vec::new()];
    for options in choices {
        all = all
            .into_iter()
            .flat_map(|prefix| {
                options.iter().map(move |value| {
                    let mut next = prefix.clone();
                    next.push(value.clone());
                    next
                })
            })
            .collect();
    }
    all
}

/// Every binding of a parameter list to atoms of its sorts.
fn bindings(universe: &Universe<'_>, params: &[(String, String)]) -> Vec<BTreeMap<String, Atom>> {
    let mut all = vec![BTreeMap::new()];
    for (name, sig) in params {
        all = all
            .into_iter()
            .flat_map(|prefix| {
                universe.sorts[sig.as_str()].iter().map(move |&atom| {
                    let mut next = prefix.clone();
                    next.insert(name.clone(), atom);
                    next
                })
            })
            .collect();
    }
    all
}

/// The verdict of every check, exploring at most `max_states` states over
/// all configurations.
pub fn evaluate(model: &Model, max_states: usize) -> Vec<Verdict> {
    let universe = Universe::new(model);
    let statics: Vec<_> = model.relations.iter().filter(|r| !r.var).collect();
    let vars: Vec<_> = model.relations.iter().filter(|r| r.var).collect();
    let mut found: Vec<Option<usize>> = vec![None; model.checks.len()];
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
        return unknown(model);
    };
    let mut explored = 0usize;
    let mut complete = true;
    let mut initial = false;
    let none = BTreeMap::new();
    'configurations: for configuration in product(&static_choices) {
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
            for action in &model.actions {
                for bound in bindings(&universe, &action.params) {
                    let scene = Scene {
                        relations: &relations,
                        bound: &bound,
                    };
                    if !action.guards.iter().all(|g| universe.formula(g, &scene)) {
                        continue;
                    }
                    let mut next = state.clone();
                    let mut admitted = true;
                    for (var, value) in &action.assignments {
                        let position = vars
                            .iter()
                            .position(|r| &r.name == var)
                            .expect("checked var");
                        let value = universe.expression(value, &scene);
                        let domain: Value =
                            universe.domain(&vars[position].shape).into_iter().collect();
                        if !value.is_subset(&domain)
                            || !universe.admits(&vars[position].shape, &value)
                        {
                            admitted = false;
                            break;
                        }
                        next[position] = value;
                    }
                    if admitted && seen.insert(next.clone()) {
                        queue.push_back((next, depth + 1));
                    }
                }
            }
        }
    }
    model
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
        .collect()
}
