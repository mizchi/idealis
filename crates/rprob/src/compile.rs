use crate::{
    Action, ActionGroup, BitmapLayout, Check, Compiled, Initial, InitialSearch, Toggle,
    Unsupported, WordTest, WordUpdate, bits,
};
use rmodel::{
    Compare, Expr, Formula, Int, IntCompare, Model, Mult, Phase, Quantifier, SetOp, Shape,
};
use std::{cell::Cell, collections::BTreeMap};

const MAX_TUPLES: usize = 20;
const MAX_ACTION_PARAMETERS: usize = 128;
const MAX_LOWERED_NODES: usize = 1_000_000;

#[derive(Clone, Copy)]
enum Width {
    Empty,
    Unary,
    Binary,
}

#[derive(Clone, Copy)]
pub(crate) enum Slot {
    Config(usize),
    State(usize),
}

#[derive(Clone, Copy)]
pub(crate) struct Environment<'a> {
    pub(crate) config: &'a [u64],
    pub(crate) before: &'a [u64],
    pub(crate) after: &'a [u64],
}

pub(crate) enum Term {
    Const(u64),
    Config(usize),
    Before(usize),
    After(usize),
    Binary(fn(u64, u64) -> u64, Box<Term>, Box<Term>),
    Mask {
        input: Box<Term>,
        keep: u64,
        add: u64,
    },
    Row {
        matrix: Box<Term>,
        shift: usize,
    },
}

impl Term {
    fn mask(input: Self, keep: u64, add: u64) -> Self {
        Self::Mask {
            input: Box::new(input),
            keep,
            add,
        }
    }

    fn reads_state(&self) -> bool {
        match self {
            Self::Const(_) | Self::Config(_) => false,
            Self::Before(_) | Self::After(_) => true,
            Self::Binary(_, left, right) => left.reads_state() || right.reads_state(),
            Self::Mask { input, .. } => input.reads_state(),
            Self::Row { matrix, .. } => matrix.reads_state(),
        }
    }

    pub(crate) fn evaluate(&self, env: Environment<'_>) -> u64 {
        match self {
            Self::Const(value) => *value,
            Self::Config(index) => env.config[*index],
            Self::Before(index) => env.before[*index],
            Self::After(index) => env.after[*index],
            Self::Binary(op, left, right) => op(left.evaluate(env), right.evaluate(env)),
            Self::Mask { input, keep, add } => (input.evaluate(env) & keep) | add,
            Self::Row { matrix, shift } => (matrix.evaluate(env) >> shift) & 0xff,
        }
    }
}

pub(crate) enum Number {
    Literal(i64),
    Count(Term),
}

impl Number {
    fn evaluate(&self, env: Environment<'_>) -> i64 {
        match self {
            Self::Literal(value) => *value,
            Self::Count(term) => i64::from(term.evaluate(env).count_ones()),
        }
    }
}

pub(crate) enum Predicate {
    Constant(bool),
    MaskedEqual {
        input: Term,
        mask: u64,
        expected: u64,
        negated: bool,
    },
    Compare(Compare, bool, Term, Term),
    Ints(IntCompare, Number, Number),
    Mult(Mult, Term),
    Empty(Term),
    Not(Box<Self>),
    All(Vec<Self>),
    Any(Vec<Self>),
    Implies(Box<Self>, Box<Self>),
}

impl Predicate {
    fn all(mut parts: Vec<Self>) -> Self {
        if parts.len() == 1 {
            parts.pop().expect("one part")
        } else {
            Self::All(parts)
        }
    }

    fn compare(op: Compare, same_width: bool, left: Term, right: Term) -> Self {
        let negated = matches!(op, Compare::NotIn | Compare::NotEqual);
        let subset = matches!(op, Compare::In | Compare::NotIn);
        let masked = |input, mask, expected| Self::MaskedEqual {
            input,
            mask,
            expected,
            negated,
        };
        match (left, right) {
            (Term::Const(a), Term::Const(b)) => Self::Constant(compare(op, same_width, a, b)),
            (Term::Const(a), right) if subset => {
                if same_width {
                    masked(right, a, a)
                } else {
                    Self::Constant((a == 0) != negated)
                }
            }
            (left, Term::Const(b)) if subset => {
                masked(left, if same_width { !b } else { u64::MAX }, 0)
            }
            (Term::Const(c), input) | (input, Term::Const(c)) => {
                if same_width || c == 0 {
                    masked(input, u64::MAX, c)
                } else {
                    Self::Constant(negated)
                }
            }
            (left, right) => Self::Compare(op, same_width, left, right),
        }
    }

    fn word_test(&self) -> Option<WordTest> {
        let Self::MaskedEqual {
            input,
            mask,
            expected,
            negated,
        } = self
        else {
            return None;
        };
        let (mask, expected) = match input {
            Term::Before(0) => (*mask, *expected),
            Term::Row { matrix, shift }
                if matches!(**matrix, Term::Before(0)) && *expected <= 0xff =>
            {
                ((mask & 0xff) << shift, expected << shift)
            }
            _ => return None,
        };
        Some(WordTest {
            mask,
            expected,
            negated: *negated,
        })
    }

    fn fixed_values(&self, values: &mut [Option<u64>]) {
        match self {
            Self::All(parts) => {
                for part in parts {
                    part.fixed_values(values);
                }
            }
            Self::Empty(Term::Before(index)) => values[*index] = Some(0),
            Self::MaskedEqual {
                input: Term::Before(index),
                mask: u64::MAX,
                expected,
                negated: false,
            } => values[*index] = Some(*expected),
            _ => {}
        }
    }

    fn reads_state(&self) -> bool {
        match self {
            Self::Constant(_) => false,
            Self::MaskedEqual { input, .. } | Self::Mult(_, input) | Self::Empty(input) => {
                input.reads_state()
            }
            Self::Compare(_, _, left, right) => left.reads_state() || right.reads_state(),
            Self::Ints(_, left, right) => [left, right]
                .into_iter()
                .any(|number| matches!(number, Number::Count(term) if term.reads_state())),
            Self::Not(inner) => inner.reads_state(),
            Self::All(parts) | Self::Any(parts) => parts.iter().any(Self::reads_state),
            Self::Implies(left, right) => left.reads_state() || right.reads_state(),
        }
    }

    pub(crate) fn evaluate(&self, env: Environment<'_>) -> bool {
        match self {
            Self::Constant(value) => *value,
            Self::MaskedEqual {
                input,
                mask,
                expected,
                negated,
            } => ((input.evaluate(env) & mask) == *expected) != *negated,
            Self::Compare(op, same_width, left, right) => {
                compare(*op, *same_width, left.evaluate(env), right.evaluate(env))
            }
            Self::Ints(op, left, right) => {
                let (a, b) = (left.evaluate(env), right.evaluate(env));
                match op {
                    IntCompare::Less => a < b,
                    IntCompare::LessEqual => a <= b,
                    IntCompare::Greater => a > b,
                    IntCompare::GreaterEqual => a >= b,
                    IntCompare::Equal => a == b,
                    IntCompare::NotEqual => a != b,
                }
            }
            Self::Mult(mult, term) => bits::admits(*mult, term.evaluate(env)),
            Self::Empty(term) => term.evaluate(env) == 0,
            Self::Not(inner) => !inner.evaluate(env),
            Self::All(parts) => parts.iter().all(|part| part.evaluate(env)),
            Self::Any(parts) => parts.iter().any(|part| part.evaluate(env)),
            Self::Implies(left, right) => !left.evaluate(env) || right.evaluate(env),
        }
    }
}

fn compare(op: Compare, same_width: bool, a: u64, b: u64) -> bool {
    match op {
        Compare::In => a == 0 || (same_width && a & !b == 0),
        Compare::NotIn => a != 0 && (!same_width || a & !b != 0),
        Compare::Equal => a == b && (a == 0 || same_width),
        Compare::NotEqual => a != b || (a != 0 && !same_width),
    }
}

type Bindings = BTreeMap<String, u64>;

struct Compiler<'m> {
    sorts: BTreeMap<&'m str, Vec<u64>>,
    members: BTreeMap<&'m str, u64>,
    slots: BTreeMap<&'m str, (Slot, Width)>,
    nodes: Cell<usize>,
    atom_count: usize,
}

impl<'m> Compiler<'m> {
    fn new(model: &'m Model) -> Result<Self, Unsupported> {
        if !model.temporal.is_empty()
            || !model.records.is_empty()
            || !model.variants.is_empty()
            || !model.sequences.is_empty()
            || model.scope.ints.is_some()
        {
            return Err(Unsupported::new(
                "temporal obligations and structured or integer values are outside the finite atom fragment",
            ));
        }
        let mut compiler = Self {
            sorts: BTreeMap::new(),
            members: BTreeMap::new(),
            slots: BTreeMap::new(),
            nodes: Cell::new(0),
            atom_count: 0,
        };
        for name in &model.sigs {
            let atoms = compiler.allocate(model.scope.sizes[name])?;
            compiler.sorts.insert(name, atoms);
        }
        for (name, members) in &model.enums {
            let atoms = compiler.allocate(members.len())?;
            compiler.members.extend(
                members
                    .iter()
                    .map(String::as_str)
                    .zip(atoms.iter().copied()),
            );
            compiler.sorts.insert(name, atoms);
        }
        for (name, children) in &model.sums {
            let atoms = children
                .iter()
                .flat_map(|child| compiler.sorts[child.as_str()].iter().copied())
                .collect();
            compiler.sorts.insert(name, atoms);
        }
        Ok(compiler)
    }

    fn allocate(&mut self, count: usize) -> Result<Vec<u64>, Unsupported> {
        let start = self.atom_count;
        let end = start
            .checked_add(count)
            .filter(|&end| end <= 64)
            .ok_or_else(|| Unsupported::new("more than 64 atoms in the universe"))?;
        self.atom_count = end;
        Ok((start..end).map(|atom| 1 << atom).collect())
    }

    fn spend(&self) -> Result<(), Unsupported> {
        let used = self.nodes.get();
        if used == MAX_LOWERED_NODES {
            return Err(Unsupported::new(
                "parameter and quantifier expansion exceeds the lowering limit",
            ));
        }
        self.nodes.set(used + 1);
        Ok(())
    }

    fn sort(&self, name: &str) -> Result<&[u64], Unsupported> {
        self.sorts
            .get(name)
            .map(Vec::as_slice)
            .ok_or_else(|| Unsupported::new(format!("unsupported sort {name}")))
    }

    fn binary_grid(&self) -> Result<(), Unsupported> {
        if self.atom_count <= bits::ROW {
            Ok(())
        } else {
            Err(Unsupported::new(
                "a binary expression requires at most eight atoms in the universe",
            ))
        }
    }

    fn domain(&self, shape: &Shape) -> Result<bits::Domain, Unsupported> {
        let domain = match shape {
            Shape::Unary { sig, mult } => bits::Domain {
                mult: *mult,
                rows: None,
                row_width: 0,
                tuples: self.sort(sig)?.to_vec(),
            },
            Shape::Binary { from, to, mult } => {
                self.binary_grid()?;
                let from = self.sort(from)?;
                let to = self.sort(to)?;
                let tuples = from
                    .iter()
                    .flat_map(|row| {
                        let shift = bits::ROW * row.trailing_zeros() as usize;
                        to.iter().map(move |col| col << shift)
                    })
                    .collect();
                bits::Domain {
                    mult: *mult,
                    rows: Some(from.iter().fold(0, |mask, atom| mask | atom)),
                    row_width: to.len(),
                    tuples,
                }
            }
        };
        if domain.tuples.len() > MAX_TUPLES && !matches!(domain.mult, Mult::One | Mult::Lone) {
            return Err(Unsupported::new(
                "a relation's finite candidate domain exceeds 20 tuples",
            ));
        }
        Ok(domain)
    }

    fn term(
        &self,
        expr: &Expr,
        bound: &Bindings,
        after: bool,
    ) -> Result<(Width, Term), Unsupported> {
        self.spend()?;
        match expr {
            Expr::None => Ok((Width::Empty, Term::Const(0))),
            Expr::Name(name) => {
                if let Some(&value) = bound.get(name) {
                    return Ok((Width::Unary, Term::Const(value)));
                }
                if let Some(&(slot, width)) = self.slots.get(name.as_str()) {
                    let read = match slot {
                        Slot::Config(index) => Term::Config(index),
                        Slot::State(index) if after => Term::After(index),
                        Slot::State(index) => Term::Before(index),
                    };
                    return Ok((width, read));
                }
                let value = match self.members.get(name.as_str()) {
                    Some(&value) => value,
                    None => self.sort(name)?.iter().fold(0, |mask, atom| mask | atom),
                };
                Ok((Width::Unary, Term::Const(value)))
            }
            Expr::At(phase, child) => self.term(child, bound, *phase == Phase::After),
            Expr::Binary(op, left, right) => {
                let (left_width, left) = self.term(left, bound, after)?;
                let (right_width, right) = self.term(right, bound, after)?;
                use Width::{Binary, Empty, Unary};
                let set_width = || match (left_width, right_width) {
                    (Empty, width) | (width, Empty) => Ok(width),
                    (Unary, Unary) => Ok(Unary),
                    (Binary, Binary) => Ok(Binary),
                    _ => Err(Unsupported::new("mixed-arity set operation")),
                };
                let (width, operation): (Width, fn(u64, u64) -> u64) = match op {
                    SetOp::Union => (set_width()?, |a, b| a | b),
                    SetOp::Difference => (set_width()?, |a, b| a & !b),
                    SetOp::Intersection => (set_width()?, |a, b| a & b),
                    SetOp::Product => {
                        if matches!(left_width, Empty) || matches!(right_width, Empty) {
                            return Ok((Empty, Term::Const(0)));
                        }
                        if matches!(left_width, Binary) || matches!(right_width, Binary) {
                            return Err(Unsupported::new("a product would exceed arity two"));
                        }
                        self.binary_grid()?;
                        (Binary, bits::product)
                    }
                    SetOp::Join => match (left_width, right_width) {
                        (Empty, _) | (_, Empty) => return Ok((Empty, Term::Const(0))),
                        (Unary, Binary) => (Unary, bits::image),
                        (Binary, Unary) => (Unary, bits::preimage),
                        (Binary, Binary) => (Binary, bits::compose),
                        (Unary, Unary) => return Err(Unsupported::new("join of two unary sets")),
                    },
                    SetOp::Multiply => return Err(Unsupported::new("integer multiplication")),
                };
                let term = match (op, left, right) {
                    (_, Term::Const(a), Term::Const(b)) => Term::Const(operation(a, b)),
                    (SetOp::Union, Term::Const(bits), input)
                    | (SetOp::Union, input, Term::Const(bits)) => Term::mask(input, u64::MAX, bits),
                    (SetOp::Intersection, Term::Const(bits), input)
                    | (SetOp::Intersection, input, Term::Const(bits)) => Term::mask(input, bits, 0),
                    (SetOp::Difference, input, Term::Const(bits)) => Term::mask(input, !bits, 0),
                    (SetOp::Join, Term::Const(atom), matrix)
                        if matches!((left_width, right_width), (Unary, Binary))
                            && atom.is_power_of_two() =>
                    {
                        Term::Row {
                            matrix: Box::new(matrix),
                            shift: bits::ROW * atom.trailing_zeros() as usize,
                        }
                    }
                    (_, left, right) => Term::Binary(operation, Box::new(left), Box::new(right)),
                };
                Ok((width, term))
            }
            _ => Err(Unsupported::new(
                "a value expression is outside the finite atom fragment",
            )),
        }
    }

    fn number(&self, int: &Int, bound: &Bindings) -> Result<Number, Unsupported> {
        match int {
            Int::Literal(value) => Ok(Number::Literal(*value)),
            Int::Count(expr) => Ok(Number::Count(self.term(expr, bound, false)?.1)),
            Int::Value(expr) => {
                let mut expr = expr;
                let mut after = false;
                while let Expr::At(phase, child) = expr {
                    after = *phase == Phase::After;
                    expr = child;
                }
                if let Expr::Count(set) = expr {
                    return Ok(Number::Count(self.term(set, bound, after)?.1));
                }
                Err(Unsupported::new(
                    "only cardinalities and integer literals are supported in numeric comparisons",
                ))
            }
        }
    }

    fn predicate(&self, formula: &Formula, bound: &mut Bindings) -> Result<Predicate, Unsupported> {
        self.spend()?;
        let pred = match formula {
            Formula::Compare(op, left, right) => {
                let (lw, left) = self.term(left, bound, false)?;
                let (rw, right) = self.term(right, bound, false)?;
                let same_width = matches!(
                    (lw, rw),
                    (Width::Unary, Width::Unary) | (Width::Binary, Width::Binary)
                );
                Predicate::compare(*op, same_width, left, right)
            }
            Formula::Ints(op, left, right) => {
                Predicate::Ints(*op, self.number(left, bound)?, self.number(right, bound)?)
            }
            Formula::Mult(mult, term) => Predicate::Mult(*mult, self.term(term, bound, false)?.1),
            Formula::Empty(term) => Predicate::Empty(self.term(term, bound, false)?.1),
            Formula::Not(inner) => Predicate::Not(Box::new(self.predicate(inner, bound)?)),
            Formula::And(parts) => self.conjunction(parts, bound)?,
            Formula::Or(left, right) => Predicate::Any(vec![
                self.predicate(left, bound)?,
                self.predicate(right, bound)?,
            ]),
            Formula::Implies(left, right) => Predicate::Implies(
                Box::new(self.predicate(left, bound)?),
                Box::new(self.predicate(right, bound)?),
            ),
            Formula::Quantified(kind, name, sort, body) => {
                let mut nested = bound.clone();
                let parts: Vec<_> = self
                    .sort(sort)?
                    .iter()
                    .map(|&atom| {
                        nested.insert(name.clone(), atom);
                        self.predicate(body, &mut nested)
                    })
                    .collect::<Result<_, _>>()?;
                match kind {
                    Quantifier::All => Predicate::all(parts),
                    Quantifier::Some => Predicate::Any(parts),
                }
            }
            Formula::HasTag(_, _) => return Err(Unsupported::new("variant tags")),
        };
        Ok(pred)
    }

    fn conjunction(
        &self,
        formulas: &[Formula],
        bound: &mut Bindings,
    ) -> Result<Predicate, Unsupported> {
        Ok(Predicate::all(
            formulas
                .iter()
                .map(|formula| self.predicate(formula, bound))
                .collect::<Result<_, _>>()?,
        ))
    }

    fn instances(
        &self,
        action: &rmodel::Action,
        args: &[(String, String)],
        bound: &mut Bindings,
        instances: &mut Vec<Action>,
    ) -> Result<(), Unsupported> {
        self.spend()?;
        if let Some(((param, sort), remaining)) = args.split_first() {
            for &atom in self.sort(sort)? {
                bound.insert(param.clone(), atom);
                self.instances(action, remaining, bound, instances)?;
            }
            bound.remove(param);
            return Ok(());
        }
        let guard = self.conjunction(&action.guards, bound)?;
        let assignments = action
            .assignments
            .iter()
            .map(|(name, value)| {
                let Some(&(Slot::State(index), _)) = self.slots.get(name.as_str()) else {
                    return Err(Unsupported::new(format!("unknown state variable {name}")));
                };
                Ok((index, self.term(value, bound, false)?.1))
            })
            .collect::<Result<_, Unsupported>>()?;
        instances.push(Action {
            state_guard: guard.reads_state(),
            guard,
            assignments,
            word: None,
        });
        Ok(())
    }
}

fn word_update(action: &Action) -> Option<WordUpdate> {
    let [(0, term)] = action.assignments.as_slice() else {
        return None;
    };
    let (keep, add) = match term {
        Term::Const(value) => (0, *value),
        Term::Mask { input, keep, add } if matches!(**input, Term::Before(0)) => (*keep, *add),
        _ => return None,
    };
    let guard = if action.state_guard {
        Some(action.guard.word_test()?)
    } else {
        None
    };
    Some(WordUpdate { guard, keep, add })
}

fn toggle_group(instances: &[Action]) -> Option<Toggle> {
    let set = instances.first()?.word?.guard?.negated;
    let mut mask = 0;
    for action in instances {
        let WordUpdate { guard, keep, add } = action.word?;
        let WordTest {
            mask: bit,
            expected,
            negated,
        } = guard?;
        // Increasing one-hot masks keep the original instance order.
        if expected != bit || !bit.is_power_of_two() || bit <= mask || negated != set {
            return None;
        }
        if (keep, add) != if set { (u64::MAX, bit) } else { (!bit, 0) } {
            return None;
        }
        mask |= bit;
    }
    Some(Toggle { mask, set })
}

pub(crate) fn lower(model: &Model) -> Result<Compiled, Unsupported> {
    let mut compiler = Compiler::new(model)?;
    let mut config = Vec::new();
    let mut state = Vec::new();
    for relation in &model.relations {
        let domain = compiler.domain(&relation.shape)?;
        let width = if domain.rows.is_some() {
            Width::Binary
        } else {
            Width::Unary
        };
        let slot = if relation.var {
            let index = state.len();
            state.push(domain);
            Slot::State(index)
        } else {
            let index = config.len();
            config.push(domain.candidates());
            Slot::Config(index)
        };
        compiler.slots.insert(&relation.name, (slot, width));
    }
    let mut scope = Bindings::new();
    let mut actions = Vec::new();
    for action in &model.actions {
        if action.params.len() > MAX_ACTION_PARAMETERS {
            return Err(Unsupported::new(format!(
                "an action may have at most {MAX_ACTION_PARAMETERS} parameters"
            )));
        }
        let mut instances = Vec::new();
        compiler.instances(action, &action.params, &mut scope, &mut instances)?;
        if let [domain] = state.as_slice()
            && domain.mult == Mult::Set
        {
            for action in &mut instances {
                action.word = word_update(action);
            }
        }
        actions.push(ActionGroup {
            name: action.name.clone(),
            toggle: toggle_group(&instances),
            instances,
        });
    }
    let mut checks = Vec::new();
    for check in &model.checks {
        checks.push(Check {
            name: check.name.clone(),
            expects_true: check.kind == rmodel::CheckKind::Reachable,
            predicate: compiler.predicate(&check.formula, &mut scope)?,
        });
    }
    let mut transitions = Vec::new();
    for check in &model.transitions {
        transitions.push(Check {
            name: check.name.clone(),
            expects_true: false,
            predicate: compiler.predicate(&check.formula, &mut scope)?,
        });
    }
    let init = compiler.conjunction(&model.init, &mut scope)?;
    let candidates: Vec<_> = state.iter().map(bits::Domain::candidates).collect();
    let mut values = vec![None; state.len()];
    init.fixed_values(&mut values);
    let fixed: Option<Vec<_>> = values.into_iter().collect();
    let count = if candidates.iter().any(|values| values.len() == 0) {
        Some(0)
    } else {
        candidates
            .iter()
            .try_fold(1usize, |count, values| count.checked_mul(values.len()))
    };
    let search = match fixed {
        Some(values) => {
            let legal = values.iter().zip(&state).all(|(&value, domain)| {
                let mask = domain.tuples.iter().fold(0, |mask, tuple| mask | tuple);
                value & !mask == 0 && domain.admits(value)
            });
            InitialSearch::Fixed(legal.then_some(values))
        }
        None => InitialSearch::Enumerated(candidates),
    };
    let initial = Initial { count, search };
    Ok(Compiled {
        facts: compiler.conjunction(&model.facts, &mut scope)?,
        init,
        initial,
        bitmap: match state.as_slice() {
            [only] => BitmapLayout::new(&only.tuples),
            _ => None,
        },
        state,
        config,
        actions,
        checks,
        transitions,
    })
}
