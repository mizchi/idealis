use crate::compile::{Environment, Number, Predicate, Term};
use crate::{indexed, indexed_sets};
use rmodel::{Compare, Expr, Formula, Int, Model, Mult, Shape};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) enum Encoding {
    Bits(Arc<Layout>),
    Ordinal(Arc<indexed::Layout>),
}

impl Encoding {
    pub(crate) fn max(&self) -> usize {
        match self {
            Self::Bits(layout) => layout.max,
            Self::Ordinal(layout) => layout.sequence_element().expect("sequence layout").1,
        }
    }

    pub(crate) fn encode(&self, items: &[u64]) -> Option<u64> {
        match self {
            Self::Bits(layout) => layout.encode(items),
            Self::Ordinal(layout) => layout.encode(items.iter().copied().map(Some)),
        }
    }

    fn decode(&self, code: u64) -> Vec<u64> {
        match self {
            Self::Bits(layout) => layout.decode(code),
            Self::Ordinal(layout) => layout.sequence_codes(code),
        }
    }

    pub(crate) fn element_mask(&self) -> Option<u64> {
        match self {
            Self::Bits(layout) => Some(layout.elements.iter().fold(0, |mask, bit| mask | bit)),
            Self::Ordinal(layout) => layout
                .sequence_element()
                .expect("sequence layout")
                .0
                .bit_mask(),
        }
    }
}

/// Bits are ordered by length, then lexicographically by the element domain.
pub(crate) struct Layout {
    pub(crate) values: Vec<u64>,
    pub(crate) elements: Vec<u64>,
    pub(crate) max: usize,
    pub(crate) literals: Option<Vec<Vec<u64>>>,
}

impl Layout {
    pub(crate) fn count(elements: usize, max: usize) -> usize {
        (0..=max).map(|length| elements.pow(length as u32)).sum()
    }

    pub(crate) fn decode(&self, bit: u64) -> Vec<u64> {
        let mut rank = (bit.trailing_zeros() - self.values[0].trailing_zeros()) as usize;
        if let Some(literals) = &self.literals {
            return literals[rank].clone();
        }
        let base = self.elements.len();
        let length = (0..=self.max)
            .find(|&length| {
                let block = base.pow(length as u32);
                if rank < block {
                    true
                } else {
                    rank -= block;
                    false
                }
            })
            .expect("stored bit is a sequence in its domain");
        let mut items = vec![0; length];
        for element in items.iter_mut().rev() {
            *element = self.elements[rank % base];
            rank /= base;
        }
        items
    }

    pub(crate) fn encode(&self, elements: &[u64]) -> Option<u64> {
        if elements.len() > self.max {
            return None;
        }
        if let Some(literals) = &self.literals {
            return literals
                .iter()
                .position(|value| value == elements)
                .map(|index| self.values[index]);
        }
        let base = self.elements.len();
        let rank = elements.iter().try_fold(0, |rank, element| {
            let index = self.elements.iter().position(|item| item == element)?;
            Some(rank * base + index)
        })?;
        self.values
            .get(
                rank + (0..elements.len())
                    .map(|n| base.pow(n as u32))
                    .sum::<usize>(),
            )
            .copied()
    }

    pub(crate) fn mask(&self) -> u64 {
        self.values.iter().fold(0, |mask, bit| mask | bit)
    }
}

/// Prove that all stored values of the sort come from a closed set of literals.
/// The validated model has one namespace and cannot name a value sort as a set.
pub(crate) fn closed_literals(
    model: &Model,
    sort: &str,
    members: &BTreeMap<&str, u64>,
    elements: &[u64],
) -> Option<Vec<Vec<u64>>> {
    let mut slots = BTreeSet::new();
    for relation in &model.relations {
        match &relation.shape {
            Shape::Unary { sig, mult } if sig == sort => {
                if !relation.var || *mult != Mult::One {
                    return None;
                }
                slots.insert(relation.name.as_str());
            }
            Shape::Binary { from, to, .. } if from == sort || to == sort => return None,
            _ => {}
        }
    }
    if slots.is_empty()
        || model
            .records
            .iter()
            .any(|record| record.fields.iter().any(|(_, ty)| ty == sort))
        || model.variants.iter().any(|variant| {
            variant
                .cases
                .iter()
                .any(|case| case.payload.as_deref() == Some(sort))
        })
        || model
            .sequences
            .iter()
            .any(|sequence| sequence.element == sort)
        || !slots.iter().all(|slot| {
            model
                .init
                .iter()
                .any(|part| initial_literal(part, slot, sort))
        })
    {
        return None;
    }
    let mut domain = LiteralDomain {
        sort,
        members,
        elements,
        values: BTreeSet::new(),
    };
    for action in &model.actions {
        if action.params.iter().any(|(_, ty)| ty == sort)
            || !action.guards.iter().all(|guard| domain.formula(guard))
            || !action.assignments.iter().all(|(slot, value)| {
                (!slots.contains(slot.as_str()) || closed_write(value, &slots, sort))
                    && domain.expression(value)
            })
        {
            return None;
        }
    }
    let properties = model
        .facts
        .iter()
        .chain(&model.init)
        .chain(model.checks.iter().map(|check| &check.formula))
        .chain(model.transitions.iter().map(|check| &check.formula))
        .chain(
            model
                .temporal
                .iter()
                .flat_map(|check| std::iter::once(&check.formula).chain(&check.trigger)),
        );
    if model
        .temporal
        .iter()
        .any(|check| check.params.iter().any(|(_, ty)| ty == sort))
        || !properties
            .into_iter()
            .all(|formula| domain.formula(formula))
        || !model
            .derivations
            .iter()
            .all(|(_, value)| domain.expression(value))
    {
        return None;
    }
    let mut values: Vec<_> = domain.values.into_iter().collect();
    values.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
    Some(values)
}

fn initial_literal(formula: &Formula, slot: &str, sort: &str) -> bool {
    let binds = |name: &Expr, value: &Expr| {
        matches!(name, Expr::Name(name) if name == slot)
            && matches!(value, Expr::Sequence(ty, _) if ty == sort)
    };
    match formula {
        Formula::Compare(Compare::Equal, left, right) => binds(left, right) || binds(right, left),
        Formula::And(parts) => parts.iter().any(|part| initial_literal(part, slot, sort)),
        _ => false,
    }
}

fn closed_write(value: &Expr, slots: &BTreeSet<&str>, sort: &str) -> bool {
    match value {
        Expr::Name(name) => slots.contains(name.as_str()),
        Expr::Sequence(ty, _) => ty == sort,
        Expr::If(_, yes, no) => closed_write(yes, slots, sort) && closed_write(no, slots, sort),
        _ => false,
    }
}

struct LiteralDomain<'a> {
    sort: &'a str,
    members: &'a BTreeMap<&'a str, u64>,
    elements: &'a [u64],
    values: BTreeSet<Vec<u64>>,
}

impl LiteralDomain<'_> {
    fn expression(&mut self, expr: &Expr) -> bool {
        match expr {
            Expr::Name(name) => name != self.sort,
            Expr::None | Expr::Literal(_) => true,
            Expr::Sequence(ty, items) if ty == self.sort => {
                let Some(value) = items
                    .iter()
                    .map(|item| {
                        let Expr::Name(name) = item else { return None };
                        self.members
                            .get(name.as_str())
                            .copied()
                            .filter(|bit| self.elements.contains(bit))
                    })
                    .collect::<Option<Vec<_>>>()
                else {
                    return false;
                };
                self.values.insert(value);
                self.values.len() <= 64
            }
            Expr::Sequence(_, items) | Expr::Record(_, items) => {
                items.iter().all(|item| self.expression(item))
            }
            Expr::Binary(_, a, b)
            | Expr::Arith(_, a, b)
            | Expr::ReachableFrom(a, b)
            | Expr::Index(a, b) => self.expression(a) && self.expression(b),
            Expr::If(condition, yes, no) => {
                self.formula(condition) && self.expression(yes) && self.expression(no)
            }
            Expr::Select(_, ty, condition) => ty != self.sort && self.formula(condition),
            Expr::Count(value)
            | Expr::Field(value, _)
            | Expr::Keys(value)
            | Expr::Closure(value)
            | Expr::Length(value)
            | Expr::Elements(value)
            | Expr::Payload(value, _)
            | Expr::At(_, value) => self.expression(value),
            Expr::Variant(_, value) => value.as_deref().is_none_or(|value| self.expression(value)),
            // These operations can construct values absent from the literal domain.
            Expr::Append(..)
            | Expr::Tail(..)
            | Expr::Take(..)
            | Expr::RemoveAt(..)
            | Expr::RemoveAll(..) => false,
        }
    }

    fn formula(&mut self, formula: &Formula) -> bool {
        match formula {
            Formula::Compare(_, a, b) => self.expression(a) && self.expression(b),
            Formula::Ints(_, a, b) => [a, b].into_iter().all(|operand| match operand {
                Int::Literal(_) => true,
                Int::Count(value) | Int::Value(value) | Int::LocatedValue(value, _) => {
                    self.expression(value)
                }
            }),
            Formula::Mult(_, value) | Formula::Empty(value) | Formula::HasTag(value, _) => {
                self.expression(value)
            }
            Formula::Not(inner) => self.formula(inner),
            Formula::And(parts) => parts.iter().all(|part| self.formula(part)),
            Formula::Or(a, b) | Formula::Implies(a, b) => self.formula(a) && self.formula(b),
            Formula::Quantified(_, _, ty, body) => ty != self.sort && self.formula(body),
        }
    }
}

pub(crate) enum Expression {
    Stored(Term),
    Literal(Vec<Term>),
    Append(Box<Self>, Term),
    Tail(Box<Self>),
    Take(Box<Self>, Number),
    RemoveAt(Box<Self>, Number),
    RemoveAll(Box<Self>, Term),
    RemoveIndexed(Box<Self>, Box<indexed_sets::Expression>),
    If(Predicate, Box<Self>, Box<Self>),
}

impl Expression {
    pub(crate) fn reads_state(&self) -> bool {
        match self {
            Self::Stored(value) => value.reads_state(),
            Self::Literal(items) => items.iter().any(Term::reads_state),
            Self::Append(seq, value) | Self::RemoveAll(seq, value) => {
                seq.reads_state() || value.reads_state()
            }
            Self::RemoveIndexed(seq, values) => seq.reads_state() || values.reads_state(),
            Self::Tail(seq) => seq.reads_state(),
            Self::Take(seq, n) | Self::RemoveAt(seq, n) => seq.reads_state() || n.reads_state(),
            Self::If(condition, yes, no) => {
                condition.reads_state() || yes.reads_state() || no.reads_state()
            }
        }
    }

    pub(crate) fn evaluate(&self, env: Environment<'_>, layout: &Encoding) -> Vec<u64> {
        match self {
            Self::Stored(value) => layout.decode(value.evaluate(env)),
            Self::Literal(items) => items.iter().map(|item| item.evaluate(env)).collect(),
            Self::Append(seq, value) => {
                let mut items = seq.evaluate(env, layout);
                items.push(value.evaluate(env));
                items
            }
            Self::Tail(seq) => {
                let mut items = seq.evaluate(env, layout);
                if !items.is_empty() {
                    items.remove(0);
                }
                items
            }
            Self::Take(seq, n) => {
                let mut items = seq.evaluate(env, layout);
                items.truncate(n.evaluate(env).clamp(0, items.len() as i64) as usize);
                items
            }
            Self::RemoveAt(seq, n) => {
                let mut items = seq.evaluate(env, layout);
                if let Ok(index) = usize::try_from(n.evaluate(env))
                    && index < items.len()
                {
                    items.remove(index);
                }
                items
            }
            Self::RemoveAll(seq, excluded) => {
                let mut items = seq.evaluate(env, layout);
                let excluded = excluded.evaluate(env);
                items.retain(|item| item & excluded == 0);
                items
            }
            Self::RemoveIndexed(seq, excluded) => {
                let mut items = seq.evaluate(env, layout);
                let excluded = excluded.evaluate(env);
                items.retain(|item| excluded.binary_search(item).is_err());
                items
            }
            Self::If(condition, yes, no) => {
                let branch = if condition.evaluate(env) { yes } else { no };
                branch.evaluate(env, layout)
            }
        }
    }
}

pub(crate) enum Operand {
    Encoded(Term),
    Indexed(Term),
    Computed(Encoding, Box<Expression>),
}

enum Value {
    Encoded(u64),
    Indexed(u64),
    Oversized(Vec<u64>),
}

impl Operand {
    pub(crate) fn reads_state(&self) -> bool {
        match self {
            Self::Encoded(term) | Self::Indexed(term) => term.reads_state(),
            Self::Computed(_, seq) => seq.reads_state(),
        }
    }

    fn evaluate(&self, env: Environment<'_>) -> Value {
        match self {
            Self::Encoded(term) => Value::Encoded(term.evaluate(env)),
            Self::Indexed(term) => Value::Indexed(term.evaluate(env)),
            Self::Computed(layout, seq) => {
                let items = seq.evaluate(env, layout);
                match layout.encode(&items) {
                    Some(code) => match layout {
                        Encoding::Bits(_) => Value::Encoded(code),
                        Encoding::Ordinal(_) => Value::Indexed(code),
                    },
                    None => Value::Oversized(items),
                }
            }
        }
    }

    pub(crate) fn compare(&self, op: rmodel::Compare, right: &Self, env: Environment<'_>) -> bool {
        use rmodel::Compare::{In, NotEqual, NotIn};
        let subset = matches!(op, In | NotIn);
        let holds = match (self.evaluate(env), right.evaluate(env)) {
            (Value::Encoded(left), Value::Encoded(right)) if subset => left & !right == 0,
            (Value::Encoded(left), Value::Encoded(right)) => left == right,
            (Value::Indexed(left), Value::Indexed(right)) => left == right || (subset && left == 0),
            (Value::Oversized(left), Value::Oversized(right)) => left == right,
            (Value::Encoded(0) | Value::Indexed(0), Value::Oversized(_)) => subset,
            (Value::Encoded(0), Value::Indexed(right)) => subset || right == 0,
            (Value::Indexed(left), Value::Encoded(0)) => left == 0,
            _ => false,
        };
        holds != matches!(op, NotIn | NotEqual)
    }
}
