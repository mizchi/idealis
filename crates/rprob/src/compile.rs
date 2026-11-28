use crate::{
    Action, ActionGroup, BitmapLayout, Check, Compiled, Initial, InitialSearch,
    ParameterizedAction, Toggle, Unsupported, WordTest, WordUpdate, bits, indexed, indexed_rows,
    indexed_sets, sequences, sparse, temporal, values,
};
use idealis_syntax::{
    Arith, Compare, Expr, Formula, Int, IntCompare, Model, Mult, Phase, Quantifier, SetOp, Shape,
    ValueType,
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const MAX_TUPLES: usize = 20;
const MAX_ACTION_PARAMETERS: usize = 128;
const MAX_LOWERED_NODES: usize = 1_000_000;

fn indexed_expression() -> Unsupported {
    Unsupported::new("indexed values require scalar slots and supported scalar expressions")
}

#[derive(Clone, Copy)]
enum Width {
    Empty,
    Unary,
    Binary,
}

impl Width {
    fn common(self, other: Self) -> Result<Self, Unsupported> {
        use Width::{Binary, Empty, Unary};
        match (self, other) {
            (Empty, width) | (width, Empty) => Ok(width),
            (Unary, Unary) => Ok(Unary),
            (Binary, Binary) => Ok(Binary),
            _ => Err(Unsupported::new("mixed-arity set operation")),
        }
    }
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
    pub(crate) arguments: &'a [u64],
}

pub(crate) enum Term {
    Const(u64),
    Config(usize),
    Before(usize),
    After(usize),
    Argument(usize),
    Binary(fn(u64, u64) -> u64, Box<Term>, Box<Term>),
    Sparse(Box<sparse::Operation>, Box<Term>, Box<Term>),
    Closure {
        edges: Box<Term>,
        compose: Vec<sparse::Operation>,
    },
    ReachableFrom {
        roots: Box<Term>,
        edges: Box<Term>,
        image: Option<Box<sparse::Operation>>,
    },
    If(Box<Predicate>, Box<Term>, Box<Term>),
    Select(Vec<(u64, Predicate)>),
    Mask {
        input: Box<Term>,
        keep: u64,
        add: u64,
    },
    Row {
        matrix: Box<Term>,
        shift: usize,
    },
    EncodeInteger(Box<IntegerOperand>, Integers),
    WriteProduct(Box<RowProduct>),
    Record(Arc<values::RecordLayout>, Vec<Term>),
    Indexed(Arc<indexed::Layout>, Vec<Term>),
    Variant(Arc<values::CaseLayout>, Box<Term>),
    Sequence(sequences::Encoding, Box<sequences::Expression>),
    SequenceElements(sequences::Encoding, Box<sequences::Expression>),
    SequenceIndex(sequences::Encoding, Box<sequences::Expression>, Box<Number>),
    Field(Box<Term>, Vec<(u64, u64)>),
    IndexedField(Box<Term>, indexed::Projection),
    IndexedRows(
        Arc<indexed_rows::Layout>,
        indexed_rows::Operation,
        Box<Term>,
        Box<Term>,
    ),
    IndexedRow(Arc<indexed_rows::Layout>, Box<Term>, Box<Term>),
    IndexedKeys(Arc<indexed_rows::Layout>, Box<Term>),
}

pub(crate) struct RowProduct {
    keys: Term,
    value: Term,
    columns: u64,
    product: Option<sparse::Operation>,
}

impl Term {
    fn mask(input: Self, keep: u64, add: u64) -> Self {
        Self::Mask {
            input: Box::new(input),
            keep,
            add,
        }
    }

    pub(crate) fn reads_state(&self) -> bool {
        match self {
            Self::Const(_) | Self::Config(_) | Self::Argument(_) => false,
            Self::Before(_) | Self::After(_) => true,
            Self::Binary(_, left, right) | Self::Sparse(_, left, right) => {
                left.reads_state() || right.reads_state()
            }
            Self::Closure { edges, .. } => edges.reads_state(),
            Self::ReachableFrom { roots, edges, .. } => roots.reads_state() || edges.reads_state(),
            Self::If(condition, yes, no) => {
                condition.reads_state() || yes.reads_state() || no.reads_state()
            }
            Self::Select(choices) => choices.iter().any(|(_, predicate)| predicate.reads_state()),
            Self::Mask { input, .. } => input.reads_state(),
            Self::Row { matrix, .. } => matrix.reads_state(),
            Self::EncodeInteger(operand, _) => operand.reads_state(),
            Self::WriteProduct(row) => row.keys.reads_state() || row.value.reads_state(),
            Self::Record(_, fields) | Self::Indexed(_, fields) => {
                fields.iter().any(Self::reads_state)
            }
            Self::Variant(_, value) | Self::Field(value, _) | Self::IndexedField(value, _) => {
                value.reads_state()
            }
            Self::Sequence(_, seq) | Self::SequenceElements(_, seq) => seq.reads_state(),
            Self::SequenceIndex(_, seq, index) => seq.reads_state() || index.reads_state(),
            Self::IndexedRows(_, _, left, right) | Self::IndexedRow(_, left, right) => {
                left.reads_state() || right.reads_state()
            }
            Self::IndexedKeys(_, rows) => rows.reads_state(),
        }
    }

    pub(crate) fn evaluate(&self, env: Environment<'_>) -> u64 {
        match self {
            Self::Const(value) => *value,
            Self::Config(index) => env.config[*index],
            Self::Before(index) => env.before[*index],
            Self::After(index) => env.after[*index],
            Self::Argument(index) => env.arguments[*index],
            Self::Binary(op, left, right) => op(left.evaluate(env), right.evaluate(env)),
            Self::Sparse(op, left, right) => op.evaluate(left.evaluate(env), right.evaluate(env)),
            Self::Closure { edges, compose } => closure(edges.evaluate(env), compose),
            Self::ReachableFrom {
                roots,
                edges,
                image,
            } => reachable_from(roots.evaluate(env), edges.evaluate(env), image.as_deref()),
            Self::If(condition, yes, no) => {
                let branch = if condition.evaluate(env) { yes } else { no };
                branch.evaluate(env)
            }
            Self::Select(choices) => choices
                .iter()
                .filter(|(_, predicate)| predicate.evaluate(env))
                .fold(0, |value, (atom, _)| value | atom),
            Self::Mask { input, keep, add } => (input.evaluate(env) & keep) | add,
            Self::Row { matrix, shift } => (matrix.evaluate(env) >> shift) & 0xff,
            Self::EncodeInteger(..) | Self::WriteProduct(_) => {
                unreachable!("computed storage requires checked evaluation")
            }
            Self::Record(layout, fields) => layout
                .encode(fields.iter().map(|field| Some(field.evaluate(env))))
                .expect("ordinary record terms have in-scope fields"),
            Self::Indexed(layout, fields) => layout
                .encode(fields.iter().map(|field| Some(field.evaluate(env))))
                .expect("indexed value operands stay in scope"),
            Self::Variant(case, value) => case
                .encode(value.evaluate(env))
                .expect("ordinary variant terms have an in-scope payload"),
            Self::Sequence(layout, seq) => layout
                .encode(&seq.evaluate(env, layout))
                .expect("ordinary sequence terms fit the declared length"),
            Self::SequenceElements(layout, seq) => seq
                .evaluate(env, layout)
                .into_iter()
                .fold(0, |mask, bit| mask | bit),
            Self::SequenceIndex(layout, seq, index) => {
                let index = usize::try_from(index.evaluate(env)).ok();
                index
                    .and_then(|index| seq.evaluate(env, layout).get(index).copied())
                    .unwrap_or(0)
            }
            Self::Field(value, mapping) => values::project(value.evaluate(env), mapping),
            Self::IndexedField(value, projection) => projection.evaluate(value.evaluate(env)),
            Self::IndexedRows(layout, op, left, right) => {
                layout.evaluate(*op, left.evaluate(env), right.evaluate(env))
            }
            Self::IndexedRow(layout, rows, key) => {
                layout.row(rows.evaluate(env), key.evaluate(env))
            }
            Self::IndexedKeys(layout, rows) => layout.keys(rows.evaluate(env)),
        }
    }

    pub(crate) fn evaluate_write(&self, env: Environment<'_>) -> Option<u64> {
        match self {
            Self::EncodeInteger(operand, integers) => {
                operand.evaluate(env, *integers).stored_bits()
            }
            Self::Record(layout, fields) => {
                layout.encode(fields.iter().map(|field| field.evaluate_write(env)))
            }
            Self::Indexed(layout, fields) => {
                layout.encode(fields.iter().map(|field| field.evaluate_write(env)))
            }
            Self::IndexedRows(layout, op, left, right) => {
                let left = left.evaluate_write(env)?;
                if matches!(op, indexed_rows::Operation::Product) && left == 0 {
                    return Some(0);
                }
                Some(layout.evaluate(*op, left, right.evaluate_write(env)?))
            }
            Self::Variant(case, value) => case.encode(value.evaluate_write(env)?),
            Self::Sequence(layout, seq) => layout.encode(&seq.evaluate(env, layout)),
            Self::WriteProduct(row) => {
                let keys = row.keys.evaluate(env);
                if keys == 0 {
                    return Some(0);
                }
                let values = row.value.evaluate_write(env)?;
                if values & !row.columns != 0 {
                    return None;
                }
                Some(match &row.product {
                    Some(op) => op.evaluate(keys, values),
                    None => bits::product(keys, values),
                })
            }
            Self::If(condition, yes, no) => {
                let branch = if condition.evaluate(env) { yes } else { no };
                branch.evaluate_write(env)
            }
            // Computed products appear here only under a union or as the
            // replacement rows of an override. Their new tuples survive.
            Self::Binary(op, left, right) => {
                Some(op(left.evaluate_write(env)?, right.evaluate_write(env)?))
            }
            Self::Sparse(op, left, right) => {
                Some(op.evaluate(left.evaluate_write(env)?, right.evaluate_write(env)?))
            }
            _ => Some(self.evaluate(env)),
        }
    }
}

fn closure(mut edges: u64, compose: &[sparse::Operation]) -> u64 {
    for step in compose {
        edges |= step.evaluate(edges, edges);
    }
    loop {
        let joined = if let Some(last) = compose.last() {
            last.evaluate(edges, edges)
        } else {
            bits::compose(edges, edges)
        };
        let expanded = edges | joined;
        if expanded == edges {
            return edges;
        }
        edges = expanded;
    }
}

fn reachable_from(mut reached: u64, edges: u64, image: Option<&sparse::Operation>) -> u64 {
    let mut frontier = reached;
    while frontier != 0 {
        let next = match image {
            Some(op) => op.evaluate(frontier, edges),
            None => bits::image(frontier, edges),
        };
        frontier = next & !reached;
        reached |= frontier;
    }
    reached
}

#[derive(Clone, Copy)]
pub(crate) struct Integers {
    low: i64,
    first: u32,
    count: u32,
}

impl Integers {
    fn bit(self, value: i64) -> Option<u64> {
        let offset = value.checked_sub(self.low)?;
        (0..i64::from(self.count))
            .contains(&offset)
            .then(|| 1 << (self.first + offset as u32))
    }

    fn decode(self, bit: u64) -> i64 {
        self.low + i64::from(bit.trailing_zeros() - self.first)
    }

    fn contains_bit(self, bit: u64) -> bool {
        (self.first..self.first + self.count).contains(&bit.trailing_zeros())
    }
}

pub(crate) enum Number {
    Literal(i64),
    Count(Term),
    CountIntegers(Box<IntegerOperand>, Integers),
    CountIndexed(Box<indexed_sets::Expression>),
    SequenceLength(sequences::Encoding, Box<sequences::Expression>),
    Stored(Box<Term>, Integers),
    Arithmetic(Arith, Box<Number>, Box<Number>),
    If(Box<Predicate>, Box<Number>, Box<Number>),
}

impl Number {
    pub(crate) fn reads_state(&self) -> bool {
        match self {
            Self::Literal(_) => false,
            Self::Count(term) => term.reads_state(),
            Self::CountIntegers(operand, _) => operand.reads_state(),
            Self::CountIndexed(set) => set.reads_state(),
            Self::Stored(term, _) => term.reads_state(),
            Self::SequenceLength(_, seq) => seq.reads_state(),
            Self::Arithmetic(_, left, right) => left.reads_state() || right.reads_state(),
            Self::If(condition, yes, no) => {
                condition.reads_state() || yes.reads_state() || no.reads_state()
            }
        }
    }

    pub(crate) fn evaluate(&self, env: Environment<'_>) -> i64 {
        match self {
            Self::Literal(value) => *value,
            Self::Count(term) => i64::from(term.evaluate(env).count_ones()),
            Self::CountIntegers(operand, integers) => operand.evaluate(env, *integers).len() as i64,
            Self::CountIndexed(set) => set.evaluate(env).len() as i64,
            Self::Stored(term, integers) => integers.decode(term.evaluate(env)),
            Self::SequenceLength(layout, seq) => seq.evaluate(env, layout).len() as i64,
            Self::Arithmetic(op, left, right) => {
                let (left, right) = (left.evaluate(env), right.evaluate(env));
                // The checked rmodel bounds every intermediate arithmetic operation.
                match op {
                    Arith::Add => left + right,
                    Arith::Sub => left - right,
                    Arith::Mul => left * right,
                }
            }
            Self::If(condition, yes, no) => {
                let selected = if condition.evaluate(env) { yes } else { no };
                selected.evaluate(env)
            }
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
    SingletonIn(Term, Term, bool),
    IndexedRowsIn(Arc<indexed_rows::Layout>, Term, Term, bool),
    SequenceCompare(Compare, Box<sequences::Operand>, Box<sequences::Operand>),
    Ints(IntCompare, Number, Number),
    IntegerSets(Compare, Box<IntegerOperand>, Box<IntegerOperand>, Integers),
    IntegerMultiplicity(Mult, Box<IntegerOperand>, Integers),
    IndexedSets(
        Compare,
        Box<indexed_sets::Expression>,
        Box<indexed_sets::Expression>,
    ),
    IndexedMultiplicity(Option<Mult>, Box<indexed_sets::Expression>),
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

    fn fixed_values(&self, bind: &mut impl FnMut(Slot, u64)) {
        let (term, value) = match self {
            Self::All(parts) => {
                for part in parts {
                    part.fixed_values(bind);
                }
                return;
            }
            Self::Empty(term) => (term, 0),
            Self::MaskedEqual {
                input,
                mask: u64::MAX,
                expected,
                negated: false,
            } => (input, *expected),
            _ => return,
        };
        match term {
            Term::Before(index) => bind(Slot::State(*index), value),
            Term::Config(index) => bind(Slot::Config(*index), value),
            _ => {}
        }
    }

    pub(crate) fn reads_state(&self) -> bool {
        match self {
            Self::Constant(_) => false,
            Self::MaskedEqual { input, .. } | Self::Mult(_, input) | Self::Empty(input) => {
                input.reads_state()
            }
            Self::Compare(_, _, left, right)
            | Self::SingletonIn(left, right, _)
            | Self::IndexedRowsIn(_, left, right, _) => left.reads_state() || right.reads_state(),
            Self::SequenceCompare(_, left, right) => left.reads_state() || right.reads_state(),
            Self::Ints(_, left, right) => left.reads_state() || right.reads_state(),
            Self::IntegerSets(_, left, right, _) => left.reads_state() || right.reads_state(),
            Self::IntegerMultiplicity(_, operand, _) => operand.reads_state(),
            Self::IndexedSets(_, left, right) => left.reads_state() || right.reads_state(),
            Self::IndexedMultiplicity(_, set) => set.reads_state(),
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
            Self::SingletonIn(left, right, negated) => {
                let left = left.evaluate(env);
                (left == 0 || left == right.evaluate(env)) != *negated
            }
            Self::IndexedRowsIn(layout, left, right, negated) => {
                layout.contains(left.evaluate(env), right.evaluate(env)) != *negated
            }
            Self::SequenceCompare(op, left, right) => left.compare(*op, right, env),
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
            Self::IntegerSets(op, left, right, integers) => left
                .evaluate(env, *integers)
                .compare(*op, &right.evaluate(env, *integers)),
            Self::IntegerMultiplicity(mult, operand, integers) => {
                let count = operand.evaluate(env, *integers).len();
                match mult {
                    Mult::Set => true,
                    Mult::Some => count > 0,
                    Mult::One => count == 1,
                    Mult::Lone => count <= 1,
                }
            }
            Self::Mult(mult, term) => bits::admits(*mult, term.evaluate(env)),
            Self::IndexedSets(op, left, right) => left.compare(*op, right, env),
            Self::IndexedMultiplicity(mult, set) => {
                let count = set.evaluate(env).len();
                match mult {
                    None => count == 0,
                    Some(Mult::One) => count == 1,
                    Some(Mult::Lone) => count <= 1,
                    Some(Mult::Some) => count > 0,
                    Some(Mult::Set) => true,
                }
            }
            Self::Empty(term) => term.evaluate(env) == 0,
            Self::Not(inner) => !inner.evaluate(env),
            Self::All(parts) => parts.iter().all(|part| part.evaluate(env)),
            Self::Any(parts) => parts.iter().any(|part| part.evaluate(env)),
            Self::Implies(left, right) => !left.evaluate(env) || right.evaluate(env),
        }
    }
}

pub(crate) enum IntegerOperand {
    Bits(Term),
    Scalar(Number),
    If(Box<Predicate>, Box<IntegerOperand>, Box<IntegerOperand>),
    Set(IntegerSetOp, Box<IntegerOperand>, Box<IntegerOperand>),
}

#[derive(Clone, Copy)]
pub(crate) enum IntegerSetOp {
    Union,
    Difference,
    Intersection,
}

impl IntegerSetOp {
    fn from_set_op(op: SetOp) -> Option<Self> {
        Some(match op {
            SetOp::Union | SetOp::SetUnion => Self::Union,
            SetOp::Difference | SetOp::SetDifference => Self::Difference,
            SetOp::Intersection => Self::Intersection,
            _ => return None,
        })
    }
}

struct IntegerValue {
    bits: u64,
    outside: BTreeSet<i64>,
}

impl IntegerValue {
    fn bits(bits: u64) -> Self {
        Self {
            bits,
            outside: BTreeSet::new(),
        }
    }

    fn stored_bits(&self) -> Option<u64> {
        self.outside.is_empty().then_some(self.bits)
    }

    fn len(&self) -> usize {
        self.bits.count_ones() as usize + self.outside.len()
    }

    fn compare(&self, op: Compare, right: &Self) -> bool {
        match op {
            Compare::In => self.bits & !right.bits == 0 && self.outside.is_subset(&right.outside),
            Compare::NotIn => {
                self.bits & !right.bits != 0 || !self.outside.is_subset(&right.outside)
            }
            Compare::Equal => self.bits == right.bits && self.outside == right.outside,
            Compare::NotEqual => self.bits != right.bits || self.outside != right.outside,
        }
    }

    fn combine(mut self, op: IntegerSetOp, right: Self) -> Self {
        match op {
            IntegerSetOp::Union => {
                self.bits |= right.bits;
                self.outside.extend(right.outside);
            }
            IntegerSetOp::Difference => {
                self.bits &= !right.bits;
                self.outside.retain(|value| !right.outside.contains(value));
            }
            IntegerSetOp::Intersection => {
                self.bits &= right.bits;
                self.outside.retain(|value| right.outside.contains(value));
            }
        }
        self
    }
}

impl IntegerOperand {
    fn reads_state(&self) -> bool {
        match self {
            Self::Bits(term) => term.reads_state(),
            Self::Scalar(number) => number.reads_state(),
            Self::If(guard, yes, no) => {
                guard.reads_state() || yes.reads_state() || no.reads_state()
            }
            Self::Set(_, left, right) => left.reads_state() || right.reads_state(),
        }
    }

    fn evaluate(&self, env: Environment<'_>, integers: Integers) -> IntegerValue {
        match self {
            Self::Bits(term) => IntegerValue::bits(term.evaluate(env)),
            Self::Scalar(number) => {
                let value = number.evaluate(env);
                if let Some(bit) = integers.bit(value) {
                    IntegerValue::bits(bit)
                } else {
                    IntegerValue {
                        bits: 0,
                        outside: BTreeSet::from([value]),
                    }
                }
            }
            Self::If(guard, yes, no) => {
                let branch = if guard.evaluate(env) { yes } else { no };
                branch.evaluate(env, integers)
            }
            Self::Set(op, left, right) => left
                .evaluate(env, integers)
                .combine(*op, right.evaluate(env, integers)),
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

#[derive(Clone)]
enum Binding {
    Constant(u64),
    Parameter {
        index: usize,
        sort: String,
        support: u64,
    },
}

type Bindings = BTreeMap<String, Binding>;

struct Compiler<'m> {
    model: &'m Model,
    sorts: BTreeMap<&'m str, Vec<u64>>,
    // Valid codes are a subset of the raw representation. Raw codes let
    // writes preserve an invalid value for Declarations and its witness.
    refined: BTreeMap<&'m str, Arc<[u64]>>,
    members: BTreeMap<&'m str, u64>,
    slots: BTreeMap<&'m str, (Slot, Width, u64)>,
    nodes: Cell<usize>,
    atom_count: usize,
    sparse: Option<RefCell<sparse::Tuples>>,
    integers: Option<Integers>,
    numeric_slots: BTreeSet<&'m str>,
    numeric_rows: BTreeMap<&'m str, u64>,
    records: values::Records<'m>,
    cases: values::Cases<'m>,
    sequences: BTreeMap<&'m str, Arc<sequences::Layout>>,
    indexed: BTreeMap<&'m str, Arc<indexed::Layout>>,
    indexed_rows: BTreeMap<&'m str, Arc<indexed_rows::Layout>>,
    value_slots: BTreeSet<&'m str>,
    value_rows: BTreeMap<&'m str, u64>,
}

impl<'m> Compiler<'m> {
    fn new(model: &'m Model, budget: InitialBudget) -> Result<Self, Unsupported> {
        let mut compiler = Self {
            model,
            sorts: BTreeMap::new(),
            refined: BTreeMap::new(),
            members: BTreeMap::new(),
            slots: BTreeMap::new(),
            nodes: Cell::new(0),
            atom_count: 0,
            sparse: None,
            integers: None,
            numeric_slots: BTreeSet::new(),
            numeric_rows: BTreeMap::new(),
            records: BTreeMap::new(),
            cases: BTreeMap::new(),
            sequences: BTreeMap::new(),
            indexed: BTreeMap::new(),
            indexed_rows: BTreeMap::new(),
            value_slots: BTreeSet::new(),
            value_rows: BTreeMap::new(),
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
        if let Some((low, high)) = model.scope.ints {
            let first = compiler.atom_count as u32;
            let atoms = compiler.allocate((high - low + 1) as usize)?;
            compiler.integers = Some(Integers {
                low,
                first,
                count: atoms.len() as u32,
            });
            let naturals = (low..=high)
                .zip(atoms.iter().copied())
                .filter_map(|(value, atom)| (value >= 0).then_some(atom))
                .collect();
            compiler.sorts.insert("Int", atoms);
            compiler.sorts.insert("Nat", naturals);
        }
        for ty in model
            .value_types_in_dependency_order()
            .map_err(|error| Unsupported::new(error.message))?
        {
            match ty {
                ValueType::Record(spec) => {
                    let fields = spec
                        .fields
                        .iter()
                        .map(|(_, sort)| compiler.value_domain(sort))
                        .collect::<Result<Vec<_>, _>>()?;
                    let count = fields
                        .iter()
                        .try_fold(1usize, |count, field| count.checked_mul(field.size()))
                        .ok_or_else(|| {
                            Unsupported::new(format!(
                                "record {} exceeds the finite value limit",
                                spec.name
                            ))
                        })?;
                    if budget == InitialBudget::Narrowed
                        && (count > 64 - compiler.atom_count
                            || fields
                                .iter()
                                .any(|field| matches!(field, indexed::Field::Indexed(_))))
                    {
                        let fields = spec
                            .fields
                            .iter()
                            .map(|(name, _)| name.clone())
                            .zip(fields)
                            .collect();
                        let layout = indexed::Layout::record(spec.name.clone(), fields)
                            .ok_or_else(|| {
                                Unsupported::new("record domain exceeds the index range")
                            })?;
                        compiler.indexed.insert(&spec.name, Arc::new(layout));
                        continue;
                    }
                    let values = compiler.allocate(count)?;
                    let fields = fields
                        .into_iter()
                        .map(|field| {
                            let indexed::Field::Bits(bits) = field else {
                                unreachable!("indexed record fields use indexed storage");
                            };
                            bits
                        })
                        .collect();
                    compiler.sorts.insert(&spec.name, values.clone());
                    compiler.records.insert(
                        &spec.name,
                        (spec, Arc::new(values::RecordLayout { values, fields })),
                    );
                }
                ValueType::Variant(spec) => {
                    let mut domain = Vec::new();
                    for case in &spec.cases {
                        let payloads = case
                            .payload
                            .as_deref()
                            .map(|sort| compiler.sort(sort).map(<[u64]>::to_vec))
                            .transpose()?;
                        let values = compiler.allocate(payloads.as_ref().map_or(1, Vec::len))?;
                        let mask = values.iter().fold(0, |mask, bit| mask | bit);
                        let payloads = values
                            .iter()
                            .copied()
                            .zip(payloads.unwrap_or_default())
                            .collect();
                        let numeric = matches!(case.payload.as_deref(), Some("Int" | "Nat"));
                        compiler.cases.insert(
                            &case.tag,
                            Arc::new(values::CaseLayout {
                                mask,
                                numeric,
                                payloads,
                            }),
                        );
                        domain.extend(values);
                    }
                    compiler.sorts.insert(&spec.name, domain);
                }
                ValueType::Sequence(spec) => {
                    if let Some(element) = compiler.indexed.get(spec.element.as_str()) {
                        let layout = indexed::Layout::sequence(
                            indexed::Field::Indexed(Arc::clone(element)),
                            spec.max,
                        )
                        .ok_or_else(|| {
                            Unsupported::new("sequence domain exceeds the index range")
                        })?;
                        compiler.indexed.insert(&spec.name, Arc::new(layout));
                        continue;
                    }
                    let elements = compiler.sort(&spec.element)?.to_vec();
                    let count = sequences::Layout::count(elements.len(), spec.max);
                    let literals = if budget == InitialBudget::Narrowed
                        && count > 64 - compiler.atom_count
                    {
                        sequences::closed_literals(model, &spec.name, &compiler.members, &elements)
                    } else {
                        None
                    };
                    if budget == InitialBudget::Narrowed
                        && literals.is_none()
                        && count > 64 - compiler.atom_count
                    {
                        let layout =
                            indexed::Layout::sequence(indexed::Field::Bits(elements), spec.max)
                                .ok_or_else(|| {
                                    Unsupported::new("sequence domain exceeds the index range")
                                })?;
                        compiler.indexed.insert(&spec.name, Arc::new(layout));
                        continue;
                    }
                    let values = compiler.allocate(literals.as_ref().map_or(count, Vec::len))?;
                    compiler.sorts.insert(&spec.name, values.clone());
                    compiler.sequences.insert(
                        &spec.name,
                        Arc::new(sequences::Layout {
                            values,
                            elements,
                            max: spec.max,
                            literals,
                        }),
                    );
                }
            }
        }
        if model.has_refinements() {
            compiler.refine_domains()?;
        }
        if compiler.atom_count > bits::ROW {
            compiler.sparse = Some(RefCell::default());
        }
        Ok(compiler)
    }

    fn refine_domains(&mut self) -> Result<(), Unsupported> {
        for ty in self
            .model
            .value_types_in_dependency_order()
            .map_err(|error| Unsupported::new(error.message))?
        {
            let affected = match ty {
                ValueType::Record(record) => {
                    !record.refinements.is_empty()
                        || record
                            .fields
                            .iter()
                            .any(|(_, sort)| self.refined.contains_key(sort.as_str()))
                }
                ValueType::Sequence(seq) => self.refined.contains_key(seq.element.as_str()),
                ValueType::Variant(variant) => variant.cases.iter().any(|case| {
                    case.payload
                        .as_deref()
                        .is_some_and(|sort| self.refined.contains_key(sort))
                }),
            };
            if !affected {
                continue;
            }
            let name = ty.name();
            let valid = if let Some(layout) = self.indexed.get(name) {
                self.spend_nodes(layout.count)?;
                (1..=layout.count as u64)
                    .filter(|&code| self.refinement_holds(ty, code))
                    .collect()
            } else {
                let raw = self.raw_sort(name)?;
                self.spend_nodes(raw.len())?;
                raw.iter()
                    .copied()
                    .filter(|&code| self.refinement_holds(ty, code))
                    .collect::<Vec<_>>()
            };
            self.refined.insert(name, valid.into());
        }
        Ok(())
    }

    fn refinement_holds(&self, ty: ValueType<'_>, code: u64) -> bool {
        let accepts = |sort, code| {
            self.refined
                .get(sort)
                .is_none_or(|allowed| allowed.binary_search(&code).is_ok())
        };
        match ty {
            ValueType::Record(record) => {
                record.fields.iter().enumerate().all(|(index, (_, sort))| {
                    accepts(
                        sort.as_str(),
                        self.record_field_code(&record.name, code, index),
                    )
                }) && record.refinements.iter().all(|clause| {
                    clause.holds(|path| {
                        let mut record = record;
                        let mut code = code;
                        for field in path {
                            let index = record
                                .fields
                                .iter()
                                .position(|(name, _)| name == field)
                                .expect("checked refinement path");
                            code = self.record_field_code(&record.name, code, index);
                            if let Some(nested) = self.model.record(&record.fields[index].1) {
                                record = nested;
                            }
                        }
                        self.integers
                            .expect("numeric refinement field")
                            .decode(code)
                    })
                })
            }
            ValueType::Sequence(seq) => {
                let values = if let Some(layout) = self.indexed.get(seq.name.as_str()) {
                    layout.sequence_codes(code)
                } else {
                    self.sequences[seq.name.as_str()].decode(code)
                };
                values
                    .into_iter()
                    .all(|value| accepts(seq.element.as_str(), value))
            }
            ValueType::Variant(variant) => variant.cases.iter().any(|case| {
                let layout = &self.cases[case.tag.as_str()];
                layout.mask & code != 0
                    && case.payload.as_deref().is_none_or(|sort| {
                        let (_, value) = layout
                            .payloads
                            .iter()
                            .find(|&&(tagged, _)| tagged == code)
                            .expect("case owns value code");
                        accepts(sort, *value)
                    })
            }),
        }
    }

    fn record_field_code(&self, sort: &str, code: u64, field: usize) -> u64 {
        if let Some(layout) = self.indexed.get(sort) {
            return layout.record_field(code, field);
        }
        let (_, layout) = &self.records[sort];
        let rank = layout.values.binary_search(&code).expect("raw record code");
        layout.field(rank, field)
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
        self.spend_nodes(1)
    }

    fn spend_nodes(&self, count: usize) -> Result<(), Unsupported> {
        let used = self.nodes.get();
        if count > MAX_LOWERED_NODES - used {
            return Err(Unsupported::new(
                "parameter and quantifier expansion exceeds the lowering limit",
            ));
        }
        self.nodes.set(used + count);
        Ok(())
    }

    fn sort(&self, name: &str) -> Result<&[u64], Unsupported> {
        let raw = self.raw_sort(name)?;
        Ok(self.refined.get(name).map_or(raw, AsRef::as_ref))
    }

    fn raw_sort(&self, name: &str) -> Result<&[u64], Unsupported> {
        self.sorts.get(name).map(Vec::as_slice).ok_or_else(|| {
            if self.indexed.contains_key(name) {
                indexed_expression()
            } else {
                Unsupported::new(format!("unsupported sort {name}"))
            }
        })
    }

    fn value_domain(&self, name: &str) -> Result<indexed::Field, Unsupported> {
        if let Some(layout) = self.indexed.get(name) {
            Ok(indexed::Field::Indexed(Arc::clone(layout)))
        } else {
            Ok(indexed::Field::Bits(self.sort(name)?.to_vec()))
        }
    }

    fn single_key(&self, expr: &Expr, bound: &Bindings) -> bool {
        match expr {
            Expr::Name(name) => {
                bound.contains_key(name)
                    || self.members.contains_key(name.as_str())
                    || self.model.relation(name).is_some_and(|rel| {
                        matches!(
                            rel.shape,
                            Shape::Unary {
                                mult: Mult::One | Mult::Lone,
                                ..
                            }
                        )
                    })
            }
            Expr::None => true,
            Expr::At(_, inner) => self.single_key(inner, bound),
            Expr::If(_, yes, no) => self.single_key(yes, bound) && self.single_key(no, bound),
            _ => false,
        }
    }

    fn indexed_relation(&self, expr: &Expr) -> Option<&'m idealis_syntax::Relation> {
        if self.indexed_rows.is_empty() {
            return None;
        }
        match expr {
            Expr::Name(name) if self.indexed_rows.contains_key(name.as_str()) => {
                self.model.relation(name)
            }
            Expr::At(_, expr) | Expr::Binary(SetOp::WithoutKeys, expr, _) => {
                self.indexed_relation(expr)
            }
            Expr::Binary(
                SetOp::Override | SetOp::Difference | SetOp::Intersection,
                left,
                right,
            )
            | Expr::If(_, left, right) => self
                .indexed_relation(left)
                .or_else(|| self.indexed_relation(right)),
            _ => None,
        }
    }

    fn indexed_rows_term(
        &self,
        relation: &idealis_syntax::Relation,
        expr: &Expr,
        bound: &Bindings,
        after: bool,
        write: bool,
    ) -> Result<Term, Unsupported> {
        let layout = &self.indexed_rows[relation.name.as_str()];
        let Shape::Binary { from, to, .. } = &relation.shape else {
            return Err(indexed_expression());
        };
        self.spend()?;
        Ok(match expr {
            Expr::None => Term::Const(0),
            Expr::Name(name)
                if self.indexed_rows.contains_key(name.as_str())
                    && matches!(&self.model.relation(name).expect("known relation").shape,
                    Shape::Binary { from: keys, to: values, .. } if keys == from && values == to) =>
            {
                match self.slots[name.as_str()].0 {
                    Slot::Config(index) => Term::Config(index),
                    Slot::State(index) if after => Term::After(index),
                    Slot::State(index) => Term::Before(index),
                }
            }
            Expr::At(phase, inner) => {
                return self.indexed_rows_term(
                    relation,
                    inner,
                    bound,
                    *phase == Phase::After,
                    write,
                );
            }
            Expr::If(condition, yes, no) => Term::If(
                Box::new(self.predicate(condition, bound, after)?),
                Box::new(self.indexed_rows_term(relation, yes, bound, after, write)?),
                Box::new(self.indexed_rows_term(relation, no, bound, after, write)?),
            ),
            Expr::Binary(op, left, right) => {
                use indexed_rows::Operation;
                let (op, left, right) = match op {
                    SetOp::Product => (
                        Operation::Product,
                        self.term(left, bound, after)?.1,
                        self.indexed_term(to, right, bound, after, write)?,
                    ),
                    SetOp::WithoutKeys => (
                        Operation::WithoutKeys,
                        self.indexed_rows_term(relation, left, bound, after, false)?,
                        self.term(right, bound, after)?.1,
                    ),
                    SetOp::Override | SetOp::Difference | SetOp::Intersection => (
                        match op {
                            SetOp::Override => Operation::Override,
                            SetOp::Difference => Operation::Difference,
                            _ => Operation::Intersection,
                        },
                        self.indexed_rows_term(relation, left, bound, after, false)?,
                        self.indexed_rows_term(
                            relation,
                            right,
                            bound,
                            after,
                            write && *op == SetOp::Override,
                        )?,
                    ),
                    _ => return Err(indexed_expression()),
                };
                match (&left, &right) {
                    (Term::Const(left), Term::Const(right)) => {
                        Term::Const(layout.evaluate(op, *left, *right))
                    }
                    _ => Term::IndexedRows(Arc::clone(layout), op, Box::new(left), Box::new(right)),
                }
            }
            _ => return Err(indexed_expression()),
        })
    }

    fn indexed_sort(&self, expr: &Expr, bound: &Bindings) -> Option<&'m str> {
        if self.indexed.is_empty() {
            return None;
        }
        let sort = match expr {
            Expr::Name(name) => match bound.get(name) {
                Some(Binding::Parameter { sort, .. }) => sort,
                Some(Binding::Constant(_)) => return None,
                None => match &self.model.relation(name)?.shape {
                    Shape::Unary { sig, .. } => sig,
                    Shape::Binary { .. } => return None,
                },
            },
            Expr::Record(sort, _) | Expr::Sequence(sort, _) => sort,
            Expr::At(_, inner) => return self.indexed_sort(inner, bound),
            Expr::Append(seq, _)
            | Expr::Tail(seq)
            | Expr::Take(seq, _)
            | Expr::RemoveAt(seq, _)
            | Expr::RemoveAll(seq, _) => {
                return self.indexed_sort(seq, bound);
            }
            Expr::Index(seq, _) => &self.model.sequence(self.indexed_sort(seq, bound)?)?.element,
            Expr::Binary(SetOp::Join, keys, rows) if self.single_key(keys, bound) => {
                let relation = self.indexed_relation(rows)?;
                let Shape::Binary { to, .. } = &relation.shape else {
                    return None;
                };
                to
            }
            Expr::Field(receiver, field) => {
                let record = self.model.record(self.indexed_sort(receiver, bound)?)?;
                &record.fields.iter().find(|(name, _)| name == field)?.1
            }
            Expr::If(_, yes, no) => {
                return self
                    .indexed_sort(yes, bound)
                    .or_else(|| self.indexed_sort(no, bound));
            }
            _ => return None,
        };
        self.indexed
            .get_key_value(sort.as_str())
            .map(|(&key, _)| key)
    }

    fn indexed_element_sort(&self, seq: &Expr, bound: &Bindings) -> Option<&'m str> {
        let sequence = self.model.sequence(self.indexed_sort(seq, bound)?)?;
        self.indexed
            .get_key_value(sequence.element.as_str())
            .map(|(&name, _)| name)
    }

    fn indexed_set_sort(&self, expr: &Expr, bound: &Bindings) -> Option<&'m str> {
        if self.indexed.is_empty() {
            return None;
        }
        match expr {
            Expr::Elements(seq) => self.indexed_element_sort(seq, bound),
            Expr::Binary(SetOp::Join, keys, rows) if !self.single_key(keys, bound) => {
                let Shape::Binary { to, .. } = &self.indexed_relation(rows)?.shape else {
                    return None;
                };
                Some(to)
            }
            Expr::At(_, inner) => self.indexed_set_sort(inner, bound),
            Expr::If(_, a, b) => self
                .indexed_set_sort(a, bound)
                .or_else(|| self.indexed_set_sort(b, bound)),
            Expr::Binary(op, a, b) if indexed_sets::Operation::from_op(*op).is_some() => self
                .indexed_set_sort(a, bound)
                .or_else(|| self.indexed_set_sort(b, bound))
                .or_else(|| self.indexed_sort(a, bound))
                .or_else(|| self.indexed_sort(b, bound)),
            _ => None,
        }
    }

    fn indexed_set(
        &self,
        sort: &str,
        expr: &Expr,
        bound: &Bindings,
        after: bool,
    ) -> Result<Box<indexed_sets::Expression>, Unsupported> {
        use indexed_sets::Expression as Set;
        self.spend()?;
        Ok(Box::new(match expr {
            Expr::At(phase, inner) => {
                return self.indexed_set(sort, inner, bound, *phase == Phase::After);
            }
            Expr::Elements(seq) if self.indexed_element_sort(seq, bound) == Some(sort) => {
                let (layout, seq, _) = self.sequence_expression(seq, bound, after)?;
                Set::Elements(layout, Box::new(seq))
            }
            Expr::Binary(SetOp::Join, keys, rows)
                if self.indexed_set_sort(expr, bound) == Some(sort) =>
            {
                let relation = self.indexed_relation(rows).ok_or_else(indexed_expression)?;
                Set::Rows(
                    Arc::clone(&self.indexed_rows[relation.name.as_str()]),
                    self.indexed_rows_term(relation, rows, bound, after, false)?,
                    self.term(keys, bound, after)?.1,
                )
            }
            Expr::Binary(op, a, b) => Set::Set(
                indexed_sets::Operation::from_op(*op).ok_or_else(indexed_expression)?,
                self.indexed_set(sort, a, bound, after)?,
                self.indexed_set(sort, b, bound, after)?,
            ),
            Expr::If(condition, yes, no) => Set::If(
                Box::new(self.predicate(condition, bound, after)?),
                self.indexed_set(sort, yes, bound, after)?,
                self.indexed_set(sort, no, bound, after)?,
            ),
            _ => Set::Singleton(self.indexed_term(sort, expr, bound, after, false)?),
        }))
    }

    fn indexed_term(
        &self,
        sort: &str,
        expr: &Expr,
        bound: &Bindings,
        after: bool,
        write: bool,
    ) -> Result<Term, Unsupported> {
        self.spend()?;
        if sequence_operation(expr) && self.indexed_sort(expr, bound) == Some(sort) {
            let (layout, seq, len) = self.sequence_expression(expr, bound, after)?;
            if !write && len > layout.max() {
                return Err(Unsupported::new(
                    "a sequence expression may exceed its finite length outside an assignment",
                ));
            }
            return Ok(Term::Sequence(layout, Box::new(seq)));
        }
        let fields = match expr {
            Expr::None => return Ok(Term::Const(0)),
            Expr::Name(name) if self.indexed_sort(expr, bound) == Some(sort) => {
                if let Some(Binding::Parameter { index, .. }) = bound.get(name) {
                    return Ok(Term::Argument(*index));
                }
                return Ok(match self.slots[name.as_str()].0 {
                    Slot::Config(index) => Term::Config(index),
                    Slot::State(index) if after => Term::After(index),
                    Slot::State(index) => Term::Before(index),
                });
            }
            Expr::At(phase, inner) => {
                return self.indexed_term(sort, inner, bound, *phase == Phase::After, write);
            }
            Expr::Index(seq, index) if self.indexed_sort(expr, bound) == Some(sort) => {
                let (layout, seq, _) = self.sequence_expression(seq, bound, after)?;
                return Ok(Term::SequenceIndex(
                    layout,
                    Box::new(seq),
                    Box::new(self.scalar(index, bound, after)?),
                ));
            }
            Expr::Binary(SetOp::Join, keys, rows)
                if self.indexed_sort(expr, bound) == Some(sort) =>
            {
                let relation_type = self.indexed_relation(rows).ok_or_else(indexed_expression)?;
                let layout = Arc::clone(&self.indexed_rows[relation_type.name.as_str()]);
                let relation = self.indexed_rows_term(relation_type, rows, bound, after, false)?;
                let (_, key) = self.term(keys, bound, after)?;
                return Ok(Term::IndexedRow(layout, Box::new(relation), Box::new(key)));
            }
            Expr::Field(receiver, field) if self.indexed_sort(expr, bound) == Some(sort) => {
                return self
                    .indexed_projection(receiver, field, bound, after)
                    .map(|(term, _)| term);
            }
            Expr::If(condition, yes, no) => {
                return Ok(Term::If(
                    Box::new(self.predicate(condition, bound, after)?),
                    Box::new(self.indexed_term(sort, yes, bound, after, write)?),
                    Box::new(self.indexed_term(sort, no, bound, after, write)?),
                ));
            }
            Expr::Record(name, values) if name == sort => {
                let spec = self.model.record(name).ok_or_else(indexed_expression)?;
                values
                    .iter()
                    .zip(&spec.fields)
                    .map(|(value, (_, ty))| self.indexed_field(ty, value, bound, after, write))
                    .collect::<Result<Vec<_>, _>>()?
            }
            Expr::Sequence(name, values) if name == sort => {
                let spec = self.model.sequence(name).ok_or_else(indexed_expression)?;
                values
                    .iter()
                    .map(|value| self.indexed_field(&spec.element, value, bound, after, write))
                    .collect::<Result<Vec<_>, _>>()?
            }
            _ => return Err(indexed_expression()),
        };
        let layout = &self.indexed[sort];
        if fields.iter().all(|term| matches!(term, Term::Const(_))) {
            let code = layout
                .encode(fields.iter().map(|term| match term {
                    Term::Const(value) => Some(*value),
                    _ => unreachable!("constant fields"),
                }))
                .ok_or_else(indexed_expression)?;
            return Ok(Term::Const(code));
        }
        Ok(Term::Indexed(Arc::clone(layout), fields))
    }

    fn indexed_field(
        &self,
        sort: &str,
        expr: &Expr,
        bound: &Bindings,
        after: bool,
        write: bool,
    ) -> Result<Term, Unsupported> {
        if self.indexed.contains_key(sort) {
            return self.indexed_term(sort, expr, bound, after, write);
        }
        if write {
            return self.value_write(expr, matches!(sort, "Int" | "Nat"), bound);
        }
        let (_, term, support) = self.lower_term(expr, bound, after)?;
        let mask = self.sort(sort)?.iter().fold(0, |mask, &bit| mask | bit);
        if support & !mask != 0 {
            return Err(indexed_expression());
        }
        Ok(term)
    }

    fn domain(&self, shape: &Shape) -> Result<bits::Domain, Unsupported> {
        let domain = match shape {
            Shape::Unary { sig, mult } => {
                if let Some(layout) = self.indexed.get(sig.as_str()) {
                    if !matches!(mult, Mult::One | Mult::Lone) {
                        return Err(indexed_expression());
                    }
                    return Ok(bits::Domain {
                        mult: *mult,
                        rows: None,
                        row_width: 0,
                        tuples: Vec::new(),
                        integer_mask: None,
                        ordinal_count: Some(layout.count),
                        refinement: self
                            .refined
                            .get(sig.as_str())
                            .cloned()
                            .map(bits::Refinement::Ordinals),
                    });
                }
                let tuples = self.sort(sig)?.to_vec();
                let integer_mask = matches!(sig.as_str(), "Int" | "Nat")
                    .then(|| tuples.iter().fold(0, |mask, bit| mask | bit));
                let valid_bits = self
                    .refined
                    .contains_key(sig.as_str())
                    .then(|| tuples.iter().fold(0, |mask, bit| mask | bit));
                bits::Domain {
                    mult: *mult,
                    rows: None,
                    refinement: valid_bits.map(bits::Refinement::Bits),
                    row_width: 0,
                    tuples,
                    integer_mask,
                    ordinal_count: None,
                }
            }
            Shape::Binary { from, to, mult } => {
                if let Some(layout) = self.indexed.get(to.as_str()) {
                    if !matches!(mult, Mult::One | Mult::Lone) {
                        return Err(indexed_expression());
                    }
                    let rows = indexed_rows::Layout::with_refinement(
                        self.sort(from)?,
                        layout.count,
                        *mult,
                        self.refined.get(to.as_str()).cloned(),
                    )
                    .ok_or_else(|| {
                        Unsupported::new("indexed relation rows exceed one state word")
                    })?;
                    return Ok(bits::Domain {
                        mult: *mult,
                        rows: Some(bits::Rows::Indexed(Arc::new(rows))),
                        refinement: None,
                        row_width: 0,
                        tuples: Vec::new(),
                        integer_mask: None,
                        ordinal_count: None,
                    });
                }
                let numeric = matches!(to.as_str(), "Int" | "Nat");
                let from = self.sort(from)?;
                let refined = self.refined.contains_key(to.as_str());
                let to = self.sort(to)?;
                let (tuples, rows) = if let Some(layout) = &self.sparse {
                    let mut layout = layout.borrow_mut();
                    let mut tuples = Vec::new();
                    let mut rows = Vec::new();
                    for &row in from {
                        let mut mask = 0;
                        for &column in to {
                            let bit = layout.insert(row, column)?;
                            tuples.push(bit);
                            mask |= bit;
                        }
                        rows.push(mask);
                    }
                    (tuples, bits::Rows::Sparse(rows))
                } else {
                    let tuples = from
                        .iter()
                        .flat_map(|row| {
                            let shift = bits::ROW * row.trailing_zeros() as usize;
                            to.iter().map(move |col| col << shift)
                        })
                        .collect();
                    (
                        tuples,
                        bits::Rows::Dense(from.iter().fold(0, |mask, atom| mask | atom)),
                    )
                };
                let choices = to.len() + usize::from(*mult == Mult::Lone);
                if matches!(mult, Mult::One | Mult::Lone)
                    && choices.checked_pow(from.len() as u32).is_none()
                {
                    return Err(Unsupported::new(
                        "a function's candidate count exceeds the index range",
                    ));
                }
                let integer_mask =
                    numeric.then(|| tuples.iter().fold(0, |mask, tuple| mask | tuple));
                let valid_bits = refined.then(|| tuples.iter().fold(0, |mask, tuple| mask | tuple));
                bits::Domain {
                    mult: *mult,
                    rows: Some(rows),
                    refinement: valid_bits.map(bits::Refinement::Bits),
                    row_width: to.len(),
                    tuples,
                    integer_mask,
                    ordinal_count: None,
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
        let (width, term, _) = self.lower_term(expr, bound, after)?;
        Ok((width, term))
    }

    // The third component bounds the bits that a sparse operand may contain.
    // Binary operations allocate new pair bits only from those input supports.
    fn lower_term(
        &self,
        expr: &Expr,
        bound: &Bindings,
        after: bool,
    ) -> Result<(Width, Term, u64), Unsupported> {
        self.spend()?;
        match expr {
            Expr::None => Ok((Width::Empty, Term::Const(0), 0)),
            Expr::Literal(value) => {
                let bit = self
                    .integers
                    .and_then(|integers| integers.bit(*value))
                    .ok_or_else(|| {
                        Unsupported::new(
                            "integer literal in a set expression is outside the finite scope",
                        )
                    })?;
                Ok((Width::Unary, Term::Const(bit), bit))
            }
            Expr::Name(name) => {
                if let Some(binding) = bound.get(name) {
                    return match binding {
                        Binding::Constant(value) => Ok((Width::Unary, Term::Const(*value), *value)),
                        Binding::Parameter { index, support, .. }
                            if self.indexed_sort(expr, bound).is_none() =>
                        {
                            Ok((Width::Unary, Term::Argument(*index), *support))
                        }
                        Binding::Parameter { .. } => Err(indexed_expression()),
                    };
                }
                if let Some(&(slot, width, support)) = self.slots.get(name.as_str()) {
                    if self.indexed_sort(expr, bound).is_some()
                        || self.indexed_rows.contains_key(name.as_str())
                    {
                        return Err(indexed_expression());
                    }
                    let read = match slot {
                        Slot::Config(index) => Term::Config(index),
                        Slot::State(index) if after => Term::After(index),
                        Slot::State(index) => Term::Before(index),
                    };
                    return Ok((width, read, support));
                }
                let value = match self.members.get(name.as_str()) {
                    Some(&value) => value,
                    None => self.sort(name)?.iter().fold(0, |mask, atom| mask | atom),
                };
                Ok((Width::Unary, Term::Const(value), value))
            }
            Expr::If(condition, yes, no) => {
                let condition = self.predicate(condition, bound, after)?;
                let (left, yes, yes_support) = self.lower_term(yes, bound, after)?;
                let (right, no, no_support) = self.lower_term(no, bound, after)?;
                Ok((
                    left.common(right)?,
                    Term::If(Box::new(condition), Box::new(yes), Box::new(no)),
                    yes_support | no_support,
                ))
            }
            Expr::Select(name, sort, predicate) => {
                let mut nested = bound.clone();
                let mut support = 0;
                let choices = self
                    .sort(sort)?
                    .iter()
                    .map(|&atom| {
                        nested.insert(name.clone(), Binding::Constant(atom));
                        support |= atom;
                        Ok((atom, self.predicate(predicate, &nested, after)?))
                    })
                    .collect::<Result<_, Unsupported>>()?;
                Ok((Width::Unary, Term::Select(choices), support))
            }
            Expr::At(phase, child) => self.lower_term(child, bound, *phase == Phase::After),
            Expr::Sequence(..)
            | Expr::Append(..)
            | Expr::Tail(..)
            | Expr::Take(..)
            | Expr::RemoveAt(..)
            | Expr::RemoveAll(..) => self.sequence_term(expr, bound, after, false),
            Expr::Elements(seq) | Expr::Index(seq, _) => {
                let (layout, seq, _) = self.sequence_expression(seq, bound, after)?;
                let support = layout.element_mask().ok_or_else(indexed_expression)?;
                let term = match expr {
                    Expr::Index(_, index) => Term::SequenceIndex(
                        layout,
                        Box::new(seq),
                        Box::new(self.scalar(index, bound, after)?),
                    ),
                    _ => Term::SequenceElements(layout, Box::new(seq)),
                };
                Ok((Width::Unary, term, support))
            }
            Expr::Record(name, fields) => self.record_term(name, fields, bound, after, false),
            Expr::Field(receiver, field) => self.project_field(receiver, field, bound, after),
            Expr::Variant(tag, payload) => {
                self.variant_term(tag, payload.as_deref(), bound, after, false)
            }
            Expr::Payload(receiver, tag) => {
                let case = &self.cases[tag.as_str()];
                let (_, term, support) = self.lower_term(receiver, bound, after)?;
                if let Term::Const(bits) = term {
                    let value = values::project(bits, &case.payloads);
                    return Ok((Width::Unary, Term::Const(value), value));
                }
                let support = if support == 0 && self.sparse.is_none() {
                    case.mask
                } else {
                    support
                };
                self.spend_nodes(case.payloads.len())?;
                Ok((
                    Width::Unary,
                    Term::Field(Box::new(term), case.payloads.clone()),
                    values::project(support, &case.payloads),
                ))
            }
            Expr::Keys(relation) => {
                if let Some(relation_type) = self.indexed_relation(relation) {
                    let layout = &self.indexed_rows[relation_type.name.as_str()];
                    let term =
                        self.indexed_rows_term(relation_type, relation, bound, after, false)?;
                    return Ok((
                        Width::Unary,
                        Term::IndexedKeys(Arc::clone(layout), Box::new(term)),
                        layout.key_mask(),
                    ));
                }
                let (_, relation, support) = self.lower_term(relation, bound, after)?;
                if let Some(layout) = &self.sparse {
                    let project = layout.borrow().preimage(support, u64::MAX);
                    let keys = project.evaluate(support, u64::MAX);
                    if let Term::Const(relation) = relation {
                        let value = project.evaluate(relation, u64::MAX);
                        return Ok((Width::Unary, Term::Const(value), value));
                    }
                    self.spend_nodes(project.rule_count())?;
                    return Ok((
                        Width::Unary,
                        Term::Sparse(
                            Box::new(project),
                            Box::new(relation),
                            Box::new(Term::Const(u64::MAX)),
                        ),
                        keys,
                    ));
                }
                const COLUMNS: u64 = (1 << bits::ROW) - 1;
                let term = match relation {
                    Term::Const(value) => Term::Const(bits::preimage(value, COLUMNS)),
                    input => Term::Binary(
                        bits::preimage,
                        Box::new(input),
                        Box::new(Term::Const(COLUMNS)),
                    ),
                };
                Ok((Width::Unary, term, 0))
            }
            Expr::Closure(edges) => {
                let (_, edges, mut support) = self.lower_term(edges, bound, after)?;
                let mut compose = Vec::new();
                if let Some(layout) = &self.sparse {
                    let mut layout = layout.borrow_mut();
                    // Each step can introduce new ordered pairs into the shared
                    // layout. The final step covers every possible result pair.
                    loop {
                        let step = layout.compose(support, support)?;
                        self.spend_nodes(step.rule_count())?;
                        let expanded = support | step.evaluate(support, support);
                        compose.push(step);
                        if expanded == support {
                            break;
                        }
                        support = expanded;
                    }
                }
                let term = match edges {
                    Term::Const(value) => {
                        let value = closure(value, &compose);
                        return Ok((Width::Binary, Term::Const(value), value));
                    }
                    edges => Term::Closure {
                        edges: Box::new(edges),
                        compose,
                    },
                };
                Ok((Width::Binary, term, support))
            }
            Expr::ReachableFrom(roots, edges) => {
                let (width, roots, root_bits) = self.lower_term(roots, bound, after)?;
                let (_, edges, edge_bits) = self.lower_term(edges, bound, after)?;
                if matches!(roots, Term::Const(0)) || matches!(edges, Term::Const(0)) {
                    return Ok((width, roots, root_bits));
                }
                let image = self
                    .sparse
                    .as_ref()
                    .map(|tuples| tuples.borrow().image(u64::MAX, edge_bits));
                let support = if let Some(image) = &image {
                    self.spend_nodes(image.rule_count())?;
                    reachable_from(root_bits, edge_bits, Some(image))
                } else {
                    0
                };
                let term = match (roots, edges) {
                    (Term::Const(roots), Term::Const(edges)) => {
                        let value = reachable_from(roots, edges, image.as_ref());
                        return Ok((Width::Unary, Term::Const(value), value));
                    }
                    (roots, edges) => Term::ReachableFrom {
                        roots: Box::new(roots),
                        edges: Box::new(edges),
                        image: image.map(Box::new),
                    },
                };
                Ok((Width::Unary, term, support))
            }
            Expr::Binary(op, left, right) => {
                let (left_width, left, a) = self.lower_term(left, bound, after)?;
                let (right_width, right, b) = self.lower_term(right, bound, after)?;
                use Width::{Binary, Empty, Unary};
                let (width, operation): (Width, fn(u64, u64) -> u64) = match op {
                    SetOp::Union | SetOp::SetUnion => {
                        (left_width.common(right_width)?, |a, b| a | b)
                    }
                    SetOp::Override => (Binary, bits::override_rows),
                    SetOp::WithoutKeys => (Binary, bits::without_keys),
                    SetOp::Difference | SetOp::SetDifference => {
                        (left_width.common(right_width)?, |a, b| a & !b)
                    }
                    SetOp::Intersection => (left_width.common(right_width)?, |a, b| a & b),
                    SetOp::Product => {
                        if matches!(left_width, Empty) || matches!(right_width, Empty) {
                            return Ok((Empty, Term::Const(0), 0));
                        }
                        if matches!(left_width, Binary) || matches!(right_width, Binary) {
                            return Err(Unsupported::new("a product would exceed arity two"));
                        }
                        (Binary, bits::product)
                    }
                    SetOp::Join => match (left_width, right_width) {
                        (Empty, _) | (_, Empty) => return Ok((Empty, Term::Const(0), 0)),
                        (Unary, Binary) => (Unary, bits::image),
                        (Binary, Unary) => (Unary, bits::preimage),
                        (Binary, Binary) => (Binary, bits::compose),
                        (Unary, Unary) => return Err(Unsupported::new("join of two unary sets")),
                    },
                    SetOp::Multiply => return Err(Unsupported::new("integer multiplication")),
                };
                if let Some(layout) = &self.sparse
                    && matches!(
                        op,
                        SetOp::Product | SetOp::Join | SetOp::Override | SetOp::WithoutKeys
                    )
                {
                    let mut layout = layout.borrow_mut();
                    let sparse = match op {
                        SetOp::Product => layout.product(a, b)?,
                        SetOp::Override => layout.override_rows(a, b),
                        SetOp::WithoutKeys => layout.without_keys(a, b),
                        SetOp::Join => match (left_width, right_width) {
                            (Unary, Binary) => layout.image(a, b),
                            (Binary, Unary) => layout.preimage(a, b),
                            (Binary, Binary) => layout.compose(a, b)?,
                            _ => unreachable!("checked join arity"),
                        },
                        _ => unreachable!("checked sparse operator"),
                    };
                    let support = match op {
                        SetOp::Override => a | b,
                        SetOp::WithoutKeys => a,
                        _ => sparse.evaluate(a, b),
                    };
                    let term = match (left, right) {
                        (Term::Const(a), Term::Const(b)) => {
                            let value = sparse.evaluate(a, b);
                            return Ok((width, Term::Const(value), value));
                        }
                        (left, right) => {
                            self.spend_nodes(sparse.rule_count())?;
                            Term::Sparse(Box::new(sparse), Box::new(left), Box::new(right))
                        }
                    };
                    return Ok((width, term, support));
                }
                let term = match (op, left, right) {
                    (_, Term::Const(a), Term::Const(b)) => Term::Const(operation(a, b)),
                    (SetOp::Union | SetOp::SetUnion, Term::Const(bits), input)
                    | (SetOp::Union | SetOp::SetUnion, input, Term::Const(bits)) => {
                        Term::mask(input, u64::MAX, bits)
                    }
                    (SetOp::Intersection, Term::Const(bits), input)
                    | (SetOp::Intersection, input, Term::Const(bits)) => Term::mask(input, bits, 0),
                    (SetOp::Difference | SetOp::SetDifference, input, Term::Const(bits)) => {
                        Term::mask(input, !bits, 0)
                    }
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
                let support = match &term {
                    Term::Const(value) => *value,
                    _ => match op {
                        SetOp::Union | SetOp::SetUnion | SetOp::Override => a | b,
                        SetOp::Difference | SetOp::SetDifference | SetOp::WithoutKeys => a,
                        SetOp::Intersection => a & b,
                        // In the dense layout no operator consults the support.
                        SetOp::Product | SetOp::Join | SetOp::Multiply => 0,
                    },
                };
                Ok((width, term, support))
            }
            _ => Err(Unsupported::new(
                "a value expression is outside the finite atom fragment",
            )),
        }
    }

    fn sequence_expression(
        &self,
        expr: &Expr,
        bound: &Bindings,
        after: bool,
    ) -> Result<(sequences::Encoding, sequences::Expression, usize), Unsupported> {
        use sequences::Expression as Seq;
        self.spend()?;
        let indexed = self
            .indexed_sort(expr, bound)
            .filter(|sort| self.model.sequence(sort).is_some());
        match expr {
            Expr::At(phase, inner) => {
                self.sequence_expression(inner, bound, *phase == Phase::After)
            }
            Expr::Sequence(name, items) => {
                let codec = if let Some(sort) = indexed {
                    sequences::Encoding::Ordinal(Arc::clone(&self.indexed[sort]))
                } else {
                    sequences::Encoding::Bits(Arc::clone(&self.sequences[name.as_str()]))
                };
                let values = items
                    .iter()
                    .map(|item| self.sequence_element(&codec, item, bound, after))
                    .collect::<Result<_, _>>()?;
                Ok((codec, Seq::Literal(values), items.len()))
            }
            Expr::If(condition, yes, no) => {
                let (layout, yes, yes_len) = self.sequence_expression(yes, bound, after)?;
                let (_, no, no_len) = self.sequence_expression(no, bound, after)?;
                Ok((
                    layout,
                    Seq::If(
                        self.predicate(condition, bound, after)?,
                        Box::new(yes),
                        Box::new(no),
                    ),
                    yes_len.max(no_len),
                ))
            }
            Expr::Append(seq, _)
            | Expr::Tail(seq)
            | Expr::Take(seq, _)
            | Expr::RemoveAt(seq, _)
            | Expr::RemoveAll(seq, _) => {
                let (layout, seq, len) = self.sequence_expression(seq, bound, after)?;
                let seq = Box::new(seq);
                let (seq, len) = match expr {
                    Expr::Append(_, item) => (
                        Seq::Append(seq, self.sequence_element(&layout, item, bound, after)?),
                        len + 1,
                    ),
                    Expr::Tail(_) => (Seq::Tail(seq), len.saturating_sub(1)),
                    Expr::Take(_, count) => {
                        let count = self.scalar(count, bound, after)?;
                        let len = match count {
                            Number::Literal(n) => n.clamp(0, len as i64) as usize,
                            _ => len,
                        };
                        (Seq::Take(seq, count), len)
                    }
                    Expr::RemoveAt(_, index) => {
                        (Seq::RemoveAt(seq, self.scalar(index, bound, after)?), len)
                    }
                    Expr::RemoveAll(original, excluded) => {
                        let updated = if let Some(sort) = self.indexed_element_sort(original, bound)
                        {
                            Seq::RemoveIndexed(seq, self.indexed_set(sort, excluded, bound, after)?)
                        } else {
                            Seq::RemoveAll(seq, self.term(excluded, bound, after)?.1)
                        };
                        (updated, len)
                    }
                    _ => unreachable!("matched a sequence operation"),
                };
                Ok((layout, seq, len))
            }
            _ if let Some(sort) = indexed => {
                let layout = sequences::Encoding::Ordinal(Arc::clone(&self.indexed[sort]));
                let max = layout.max();
                Ok((
                    layout,
                    Seq::Stored(self.indexed_term(sort, expr, bound, after, false)?),
                    max,
                ))
            }
            _ => {
                let (_, term, support) = self.lower_term(expr, bound, after)?;
                let sort = match expr {
                    Expr::Name(name) if !bound.contains_key(name) => self
                        .model
                        .relation(name)
                        .and_then(|relation| match &relation.shape {
                            Shape::Unary { sig, .. } => Some(sig.as_str()),
                            _ => None,
                        }),
                    Expr::Binary(SetOp::Join, _, relation)
                        if let Expr::Name(name) = relation.as_ref() =>
                    {
                        self.model
                            .relation(name)
                            .and_then(|relation| match &relation.shape {
                                Shape::Binary { to, .. } => Some(to.as_str()),
                                _ => None,
                            })
                    }
                    _ => None,
                };
                let layout = sort
                    .and_then(|sort| self.sequences.get(sort))
                    .or_else(|| {
                        let mut layouts = self
                            .sequences
                            .values()
                            .filter(|layout| support & !layout.mask() == 0);
                        let layout = layouts.next()?;
                        layouts.next().is_none().then_some(layout)
                    })
                    .ok_or_else(|| {
                        Unsupported::new("cannot identify the sequence domain of an operand")
                    })?;
                Ok((
                    sequences::Encoding::Bits(Arc::clone(layout)),
                    Seq::Stored(term),
                    layout.max,
                ))
            }
        }
    }

    fn sequence_element(
        &self,
        layout: &sequences::Encoding,
        expr: &Expr,
        bound: &Bindings,
        after: bool,
    ) -> Result<Term, Unsupported> {
        if layout.element_mask().is_some() {
            return self.term(expr, bound, after).map(|(_, term)| term);
        }
        if matches!(expr, Expr::None) {
            return Ok(Term::Const(0));
        }
        let sort = self
            .indexed_sort(expr, bound)
            .ok_or_else(indexed_expression)?;
        self.indexed_term(sort, expr, bound, after, false)
    }

    fn sequence_term(
        &self,
        expr: &Expr,
        bound: &Bindings,
        after: bool,
        write: bool,
    ) -> Result<(Width, Term, u64), Unsupported> {
        let (layout, seq, len) = self.sequence_expression(expr, bound, after)?;
        let sequences::Encoding::Bits(bits) = &layout else {
            return Err(indexed_expression());
        };
        if !write && len > bits.max {
            return Err(Unsupported::new(
                "a sequence expression may exceed its finite length outside an assignment",
            ));
        }
        if let sequences::Expression::Literal(items) = &seq
            && let Some(items) = items
                .iter()
                .map(|item| match item {
                    Term::Const(value) => Some(*value),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()
            && let Some(bit) = layout.encode(&items)
        {
            return Ok((Width::Unary, Term::Const(bit), bit));
        }
        let support = bits.mask();
        Ok((Width::Unary, Term::Sequence(layout, Box::new(seq)), support))
    }

    fn sequence_operand(
        &self,
        expr: &Expr,
        bound: &Bindings,
        after: bool,
    ) -> Result<Box<sequences::Operand>, Unsupported> {
        if sequence_operation(expr) {
            let (layout, seq, _) = self.sequence_expression(expr, bound, after)?;
            Ok(Box::new(sequences::Operand::Computed(
                layout,
                Box::new(seq),
            )))
        } else if let Some(sort) = self.indexed_sort(expr, bound) {
            Ok(Box::new(sequences::Operand::Indexed(
                self.indexed_term(sort, expr, bound, after, false)?,
            )))
        } else {
            let (width, term) = self.term(expr, bound, after)?;
            if matches!(width, Width::Binary) {
                return Err(Unsupported::new("expected a unary sequence comparison"));
            }
            Ok(Box::new(sequences::Operand::Encoded(term)))
        }
    }

    fn record_term(
        &self,
        name: &str,
        fields: &[Expr],
        bound: &Bindings,
        after: bool,
        write: bool,
    ) -> Result<(Width, Term, u64), Unsupported> {
        let (spec, layout) = self
            .records
            .get(name)
            .ok_or_else(|| Unsupported::new(format!("unsupported record {name}")))?;
        let mut terms = Vec::with_capacity(fields.len());
        for (index, (expr, (field, sort))) in fields.iter().zip(&spec.fields).enumerate() {
            let term = if write {
                self.value_write(expr, matches!(sort.as_str(), "Int" | "Nat"), bound)?
            } else {
                let (_, term, support) = self.lower_term(expr, bound, after)?;
                let domain = layout.fields[index].iter().fold(0, |mask, bit| mask | bit);
                if support == 0 || support & !domain != 0 {
                    return Err(Unsupported::new(format!(
                        "cannot prove {name}.{field} stays in range outside a record assignment"
                    )));
                }
                term
            };
            terms.push(term);
        }
        if terms.iter().all(|term| matches!(term, Term::Const(_)))
            && let Some(bit) = layout.encode(terms.iter().map(|term| {
                let Term::Const(value) = term else {
                    unreachable!("checked constant")
                };
                Some(*value)
            }))
        {
            return Ok((Width::Unary, Term::Const(bit), bit));
        }
        let support = layout.values.iter().fold(0, |mask, bit| mask | bit);
        Ok((
            Width::Unary,
            Term::Record(Arc::clone(layout), terms),
            support,
        ))
    }

    fn variant_term(
        &self,
        tag: &str,
        payload: Option<&Expr>,
        bound: &Bindings,
        after: bool,
        write: bool,
    ) -> Result<(Width, Term, u64), Unsupported> {
        let case = &self.cases[tag];
        let Some(payload) = payload else {
            return Ok((Width::Unary, Term::Const(case.mask), case.mask));
        };
        let term = if write {
            self.value_write(payload, case.numeric, bound)?
        } else {
            let (_, term, support) = self.lower_term(payload, bound, after)?;
            let domain = case.payloads.iter().fold(0, |mask, &(_, bit)| mask | bit);
            if support & !domain != 0 {
                return Err(Unsupported::new(format!(
                    "cannot prove {tag} payload stays in range outside an assignment"
                )));
            }
            term
        };
        if let Term::Const(payload) = term
            && let Some(bit) = case.encode(payload)
        {
            return Ok((Width::Unary, Term::Const(bit), bit));
        }
        Ok((
            Width::Unary,
            Term::Variant(Arc::clone(case), Box::new(term)),
            case.mask,
        ))
    }

    fn value_write(
        &self,
        expr: &Expr,
        integer: bool,
        bound: &Bindings,
    ) -> Result<Term, Unsupported> {
        self.spend()?;
        match expr {
            Expr::Record(name, fields) => Ok(self.record_term(name, fields, bound, false, true)?.1),
            Expr::Variant(tag, payload) => Ok(self
                .variant_term(tag, payload.as_deref(), bound, false, true)?
                .1),
            Expr::Sequence(..)
            | Expr::Append(..)
            | Expr::Tail(..)
            | Expr::Take(..)
            | Expr::RemoveAt(..)
            | Expr::RemoveAll(..) => Ok(self.sequence_term(expr, bound, false, true)?.1),
            Expr::If(guard, yes, no) => Ok(Term::If(
                Box::new(self.predicate(guard, bound, false)?),
                Box::new(self.value_write(yes, integer, bound)?),
                Box::new(self.value_write(no, integer, bound)?),
            )),
            _ if integer
                && self.computed_integers(expr)
                && let Some(integers) = self.integers =>
            {
                Ok(Term::EncodeInteger(
                    self.integer_operand(expr, bound, false)?,
                    integers,
                ))
            }
            _ => Ok(self.term(expr, bound, false)?.1),
        }
    }

    fn indexed_projection(
        &self,
        receiver: &Expr,
        field: &str,
        bound: &Bindings,
        after: bool,
    ) -> Result<(Term, Option<u64>), Unsupported> {
        let sort = self
            .indexed_sort(receiver, bound)
            .ok_or_else(indexed_expression)?;
        let (projection, support) = self.indexed[sort]
            .project(field)
            .ok_or_else(indexed_expression)?;
        let value = self.indexed_term(sort, receiver, bound, after, false)?;
        let term = match value {
            Term::Const(value) => Term::Const(projection.evaluate(value)),
            value => Term::IndexedField(Box::new(value), projection),
        };
        Ok((term, support))
    }

    fn project_field(
        &self,
        receiver: &Expr,
        field: &str,
        bound: &Bindings,
        after: bool,
    ) -> Result<(Width, Term, u64), Unsupported> {
        if self.indexed_sort(receiver, bound).is_some() {
            let (term, support) = self.indexed_projection(receiver, field, bound, after)?;
            return Ok((Width::Unary, term, support.ok_or_else(indexed_expression)?));
        }
        let (width, term, support) = self.lower_term(receiver, bound, after)?;
        if matches!(width, Width::Empty) || matches!(term, Term::Const(0)) {
            return Ok((width, Term::Const(0), 0));
        }
        let support = if support == 0 && self.sparse.is_none() {
            u64::MAX
        } else {
            support
        };
        let fields = self
            .records
            .values()
            .flat_map(|(spec, layout)| {
                let index = spec.fields.iter().position(|(name, _)| name == field);
                layout
                    .values
                    .iter()
                    .copied()
                    .enumerate()
                    .filter_map(move |(rank, record)| {
                        index.map(|index| (record, layout.field(rank, index)))
                    })
            })
            .collect::<Vec<_>>();
        let mapping = match width {
            Width::Unary => fields,
            Width::Binary if let Some(sparse) = &self.sparse => {
                sparse.borrow_mut().project(support, &fields)?
            }
            Width::Binary => (0..bits::ROW)
                .flat_map(|row| {
                    let shift = row * bits::ROW;
                    fields
                        .iter()
                        .map(move |(record, value)| (record << shift, value << shift))
                })
                .collect(),
            Width::Empty => unreachable!("handled empty receiver"),
        };
        self.spend_nodes(mapping.len())?;
        if let Term::Const(value) = term {
            let value = values::project(value, &mapping);
            return Ok((width, Term::Const(value), value));
        }
        let support = values::project(support, &mapping);
        Ok((width, Term::Field(Box::new(term), mapping), support))
    }

    fn number(&self, int: &Int, bound: &Bindings, after: bool) -> Result<Number, Unsupported> {
        match int {
            Int::Literal(value) => Ok(Number::Literal(*value)),
            Int::Count(expr) => self.count(expr, bound, after),
            Int::Value(expr) | Int::LocatedValue(expr, _) => self.scalar(expr, bound, after),
        }
    }

    // Keep the bit-set path for comparisons entirely inside the stored
    // universe, including constant equality used to bind initial states.
    fn computed_integers(&self, expression: &Expr) -> bool {
        match expression {
            Expr::Count(_) | Expr::Length(_) | Expr::Arith(..) => true,
            Expr::Literal(value) => self
                .integers
                .is_some_and(|integers| integers.bit(*value).is_none()),
            Expr::At(_, inner) => self.computed_integers(inner),
            Expr::If(_, yes, no) => self.computed_integers(yes) || self.computed_integers(no),
            Expr::Binary(op, left, right) if IntegerSetOp::from_set_op(*op).is_some() => {
                self.computed_integers(left) || self.computed_integers(right)
            }
            _ => false,
        }
    }

    fn count(
        &self,
        expression: &Expr,
        bound: &Bindings,
        after: bool,
    ) -> Result<Number, Unsupported> {
        if let Some(relation) = self.indexed_relation(expression) {
            let layout = &self.indexed_rows[relation.name.as_str()];
            let term = self.indexed_rows_term(relation, expression, bound, after, false)?;
            Ok(Number::Count(Term::IndexedKeys(
                Arc::clone(layout),
                Box::new(term),
            )))
        } else if let Some(sort) = self
            .indexed_set_sort(expression, bound)
            .or_else(|| self.indexed_sort(expression, bound))
        {
            Ok(Number::CountIndexed(
                self.indexed_set(sort, expression, bound, after)?,
            ))
        } else if let Some(integers) = self.integers
            && self.computed_integers(expression)
        {
            Ok(Number::CountIntegers(
                self.integer_operand(expression, bound, after)?,
                integers,
            ))
        } else {
            Ok(Number::Count(self.term(expression, bound, after)?.1))
        }
    }

    fn scalar(&self, expr: &Expr, bound: &Bindings, after: bool) -> Result<Number, Unsupported> {
        self.spend()?;
        Ok(match expr {
            Expr::Literal(value) => Number::Literal(*value),
            Expr::Count(value) => self.count(value, bound, after)?,
            Expr::Length(seq) => {
                let (layout, seq, _) = self.sequence_expression(seq, bound, after)?;
                Number::SequenceLength(layout, Box::new(seq))
            }
            Expr::At(phase, inner) => return self.scalar(inner, bound, *phase == Phase::After),
            Expr::Arith(op, left, right) => Number::Arithmetic(
                *op,
                Box::new(self.scalar(left, bound, after)?),
                Box::new(self.scalar(right, bound, after)?),
            ),
            Expr::If(condition, yes, no) => Number::If(
                Box::new(self.predicate(condition, bound, after)?),
                Box::new(self.scalar(yes, bound, after)?),
                Box::new(self.scalar(no, bound, after)?),
            ),
            Expr::Name(name) if let Some(integers) = self.integers => {
                if let Some(Binding::Constant(bit)) = bound.get(name)
                    && integers.contains_bit(*bit)
                {
                    Number::Literal(integers.decode(*bit))
                } else if self.numeric_slots.contains(name.as_str())
                    || matches!(bound.get(name), Some(Binding::Parameter { sort, .. }) if matches!(sort.as_str(), "Int" | "Nat"))
                {
                    Number::Stored(Box::new(self.term(expr, bound, after)?.1), integers)
                } else {
                    return Err(Unsupported::new("expected a scalar integer expression"));
                }
            }
            Expr::Binary(SetOp::Join, ..)
            | Expr::Field(..)
            | Expr::Payload(..)
            | Expr::Index(..)
                if let Some(integers) = self.integers =>
            {
                Number::Stored(Box::new(self.term(expr, bound, after)?.1), integers)
            }
            _ => return Err(Unsupported::new("expected a scalar integer expression")),
        })
    }

    fn integer_operand(
        &self,
        expr: &Expr,
        bound: &Bindings,
        after: bool,
    ) -> Result<Box<IntegerOperand>, Unsupported> {
        let operand = if self.computed_integers(expr) {
            self.spend()?;
            match expr {
                Expr::At(phase, inner) => {
                    return self.integer_operand(inner, bound, *phase == Phase::After);
                }
                Expr::If(guard, yes, no) => IntegerOperand::If(
                    Box::new(self.predicate(guard, bound, after)?),
                    self.integer_operand(yes, bound, after)?,
                    self.integer_operand(no, bound, after)?,
                ),
                Expr::Binary(op, left, right) if let Some(op) = IntegerSetOp::from_set_op(*op) => {
                    IntegerOperand::Set(
                        op,
                        self.integer_operand(left, bound, after)?,
                        self.integer_operand(right, bound, after)?,
                    )
                }
                _ => IntegerOperand::Scalar(self.scalar(expr, bound, after)?),
            }
        } else {
            let (width, term) = self.term(expr, bound, after)?;
            if matches!(width, Width::Binary) {
                return Err(Unsupported::new("expected a set of integers"));
            }
            IntegerOperand::Bits(term)
        };
        Ok(Box::new(operand))
    }

    fn predicate(
        &self,
        formula: &Formula,
        bound: &Bindings,
        after: bool,
    ) -> Result<Predicate, Unsupported> {
        self.spend()?;
        let pred = match formula {
            Formula::Compare(op, left, right) => {
                if let Some(relation) = self
                    .indexed_relation(left)
                    .or_else(|| self.indexed_relation(right))
                {
                    let left = self.indexed_rows_term(relation, left, bound, after, false)?;
                    let right = self.indexed_rows_term(relation, right, bound, after, false)?;
                    return Ok(match op {
                        Compare::Equal | Compare::NotEqual => {
                            Predicate::compare(*op, true, left, right)
                        }
                        Compare::In | Compare::NotIn => Predicate::IndexedRowsIn(
                            Arc::clone(&self.indexed_rows[relation.name.as_str()]),
                            left,
                            right,
                            *op == Compare::NotIn,
                        ),
                    });
                }
                if let Some(sort) = self
                    .indexed_set_sort(left, bound)
                    .or_else(|| self.indexed_set_sort(right, bound))
                {
                    return Ok(Predicate::IndexedSets(
                        *op,
                        self.indexed_set(sort, left, bound, after)?,
                        self.indexed_set(sort, right, bound, after)?,
                    ));
                }
                if sequence_operation(left) || sequence_operation(right) {
                    return Ok(Predicate::SequenceCompare(
                        *op,
                        self.sequence_operand(left, bound, after)?,
                        self.sequence_operand(right, bound, after)?,
                    ));
                }
                if let Some(sort) = self
                    .indexed_sort(left, bound)
                    .or_else(|| self.indexed_sort(right, bound))
                {
                    let left = self.indexed_term(sort, left, bound, after, false)?;
                    let right = self.indexed_term(sort, right, bound, after, false)?;
                    return Ok(match op {
                        Compare::Equal | Compare::NotEqual => {
                            Predicate::compare(*op, true, left, right)
                        }
                        Compare::In | Compare::NotIn => {
                            Predicate::SingletonIn(left, right, *op == Compare::NotIn)
                        }
                    });
                }
                if let (Some((left, left_after)), Some((right, right_after))) =
                    (counted_set(left, after), counted_set(right, after))
                {
                    // Cardinalities are singleton integers. Their containment
                    // and equality comparisons have the same truth value.
                    let comparison = match op {
                        Compare::Equal | Compare::In => IntCompare::Equal,
                        Compare::NotEqual | Compare::NotIn => IntCompare::NotEqual,
                    };
                    return Ok(Predicate::Ints(
                        comparison,
                        self.count(left, bound, left_after)?,
                        self.count(right, bound, right_after)?,
                    ));
                }
                if let Some(integers) = self.integers
                    && (self.computed_integers(left) || self.computed_integers(right))
                {
                    return Ok(Predicate::IntegerSets(
                        *op,
                        self.integer_operand(left, bound, after)?,
                        self.integer_operand(right, bound, after)?,
                        integers,
                    ));
                }
                let (lw, left) = self.term(left, bound, after)?;
                let (rw, right) = self.term(right, bound, after)?;
                let same_width = matches!(
                    (lw, rw),
                    (Width::Unary, Width::Unary) | (Width::Binary, Width::Binary)
                );
                Predicate::compare(*op, same_width, left, right)
            }
            Formula::Ints(op, left, right) => Predicate::Ints(
                *op,
                self.number(left, bound, after)?,
                self.number(right, bound, after)?,
            ),
            Formula::Mult(mult, term)
                if let Some(integers) = self.integers
                    && self.computed_integers(term) =>
            {
                Predicate::IntegerMultiplicity(
                    *mult,
                    self.integer_operand(term, bound, after)?,
                    integers,
                )
            }
            Formula::Empty(term)
                if let Some(integers) = self.integers
                    && self.computed_integers(term) =>
            {
                Predicate::Not(Box::new(Predicate::IntegerMultiplicity(
                    Mult::Some,
                    self.integer_operand(term, bound, after)?,
                    integers,
                )))
            }
            Formula::Mult(mult, term) => {
                if let Some(relation) = self.indexed_relation(term) {
                    let layout = &self.indexed_rows[relation.name.as_str()];
                    let term = self.indexed_rows_term(relation, term, bound, after, false)?;
                    Predicate::Mult(*mult, Term::IndexedKeys(Arc::clone(layout), Box::new(term)))
                } else if let Some(sort) = self.indexed_set_sort(term, bound) {
                    Predicate::IndexedMultiplicity(
                        Some(*mult),
                        self.indexed_set(sort, term, bound, after)?,
                    )
                } else if let Some(sort) = self.indexed_sort(term, bound) {
                    let term = self.indexed_term(sort, term, bound, after, false)?;
                    match mult {
                        Mult::One | Mult::Some => Predicate::Not(Box::new(Predicate::Empty(term))),
                        Mult::Set | Mult::Lone => Predicate::Constant(true),
                    }
                } else {
                    Predicate::Mult(*mult, self.term(term, bound, after)?.1)
                }
            }
            Formula::Empty(term) => {
                if let Some(relation) = self.indexed_relation(term) {
                    return Ok(Predicate::Empty(
                        self.indexed_rows_term(relation, term, bound, after, false)?,
                    ));
                }
                if let Some(sort) = self.indexed_set_sort(term, bound) {
                    return Ok(Predicate::IndexedMultiplicity(
                        None,
                        self.indexed_set(sort, term, bound, after)?,
                    ));
                }
                Predicate::Empty(if let Some(sort) = self.indexed_sort(term, bound) {
                    self.indexed_term(sort, term, bound, after, false)?
                } else {
                    self.term(term, bound, after)?.1
                })
            }
            Formula::Not(inner) => Predicate::Not(Box::new(self.predicate(inner, bound, after)?)),
            Formula::And(parts) => self.conjunction(parts, bound, after)?,
            Formula::Or(left, right) => Predicate::Any(vec![
                self.predicate(left, bound, after)?,
                self.predicate(right, bound, after)?,
            ]),
            Formula::Implies(left, right) => Predicate::Implies(
                Box::new(self.predicate(left, bound, after)?),
                Box::new(self.predicate(right, bound, after)?),
            ),
            Formula::Quantified(kind, name, sort, body) => {
                let mut nested = bound.clone();
                let parts: Vec<_> = self
                    .sort(sort)?
                    .iter()
                    .map(|&atom| {
                        nested.insert(name.clone(), Binding::Constant(atom));
                        self.predicate(body, &nested, after)
                    })
                    .collect::<Result<_, _>>()?;
                match kind {
                    Quantifier::All => Predicate::all(parts),
                    Quantifier::Some => Predicate::Any(parts),
                }
            }
            Formula::HasTag(value, tag) => {
                let (_, term) = self.term(value, bound, after)?;
                Predicate::compare(
                    Compare::NotEqual,
                    true,
                    Term::mask(term, self.cases[tag.as_str()].mask, 0),
                    Term::Const(0),
                )
            }
        };
        Ok(pred)
    }

    fn computed_row(
        &self,
        expression: &Expr,
        bound: &Bindings,
        integers: Option<Integers>,
        columns: u64,
    ) -> Result<Option<(Term, u64)>, Unsupported> {
        let Expr::Binary(SetOp::Product, keys, value) = expression else {
            return Ok(None);
        };
        let value = match integers {
            Some(integers) if self.computed_integers(value) => {
                Term::EncodeInteger(self.integer_operand(value, bound, false)?, integers)
            }
            None if matches!(
                value.as_ref(),
                Expr::Record(..)
                    | Expr::Variant(..)
                    | Expr::If(..)
                    | Expr::Sequence(..)
                    | Expr::Append(..)
                    | Expr::Tail(..)
                    | Expr::Take(..)
                    | Expr::RemoveAt(..)
                    | Expr::RemoveAll(..)
            ) =>
            {
                self.value_write(value, false, bound)?
            }
            _ => return Ok(None),
        };
        let (_, keys, key_support) = self.lower_term(keys, bound, false)?;
        let (product, support) = if let Some(sparse) = &self.sparse {
            let operation = sparse.borrow_mut().product(key_support, columns)?;
            self.spend_nodes(operation.rule_count())?;
            let support = operation.evaluate(key_support, columns);
            (Some(operation), support)
        } else {
            (None, 0)
        };
        self.spend()?;
        Ok(Some((
            Term::WriteProduct(Box::new(RowProduct {
                keys,
                value,
                columns,
                product,
            })),
            support,
        )))
    }

    fn assignment(&self, name: &str, value: &Expr, bound: &Bindings) -> Result<Term, Unsupported> {
        if self.indexed_rows.contains_key(name) {
            let relation = self
                .model
                .relation(name)
                .expect("declared indexed relation");
            return self.indexed_rows_term(relation, value, bound, false, true);
        }
        if !self.indexed.is_empty()
            && let Some(relation) = self.model.relation(name)
            && let Shape::Unary { sig, .. } = &relation.shape
            && self.indexed.contains_key(sig.as_str())
        {
            return self.indexed_term(sig, value, bound, false, true);
        }
        if self.value_slots.contains(name) {
            return self.value_write(value, false, bound);
        }
        if let Some(integers) = self.integers
            && self.numeric_slots.contains(name)
            && self.computed_integers(value)
        {
            return Ok(Term::EncodeInteger(
                self.integer_operand(value, bound, false)?,
                integers,
            ));
        }
        let row = self
            .numeric_rows
            .get(name)
            .zip(self.integers)
            .map(|(&columns, integers)| (columns, Some(integers)))
            .or_else(|| self.value_rows.get(name).map(|&columns| (columns, None)));
        if let Some((columns, integers)) = row {
            self.spend()?;
            if let Expr::If(condition, yes, no) = value {
                return Ok(Term::If(
                    Box::new(self.predicate(condition, bound, false)?),
                    Box::new(self.assignment(name, yes, bound)?),
                    Box::new(self.assignment(name, no, bound)?),
                ));
            }
            if let Some((term, _)) = self.computed_row(value, bound, integers, columns)? {
                return Ok(term);
            }
            if let Expr::Binary(op @ (SetOp::Union | SetOp::Override), left, right) = value
                && let Some((right, right_support)) =
                    self.computed_row(right, bound, integers, columns)?
            {
                let (_, left, left_support) = self.lower_term(left, bound, false)?;
                let left = Box::new(left);
                let right = Box::new(right);
                if *op == SetOp::Union {
                    return Ok(Term::Binary(|a, b| a | b, left, right));
                }
                if let Some(sparse) = &self.sparse {
                    let operation = sparse.borrow().override_rows(left_support, right_support);
                    self.spend_nodes(operation.rule_count())?;
                    return Ok(Term::Sparse(Box::new(operation), left, right));
                }
                return Ok(Term::Binary(bits::override_rows, left, right));
            }
        }
        Ok(self.term(value, bound, false)?.1)
    }

    fn conjunction(
        &self,
        formulas: &[Formula],
        bound: &Bindings,
        after: bool,
    ) -> Result<Predicate, Unsupported> {
        Ok(Predicate::all(
            formulas
                .iter()
                .map(|formula| self.predicate(formula, bound, after))
                .collect::<Result<_, _>>()?,
        ))
    }

    fn instances(
        &self,
        action: &idealis_syntax::Action,
        args: &[(String, String)],
        bound: &mut Bindings,
        instances: &mut Vec<Action>,
        transitions: &[(usize, &Formula)],
    ) -> Result<(), Unsupported> {
        self.spend()?;
        if let Some(((param, sort), remaining)) = args.split_first() {
            for &atom in self.sort(sort)? {
                bound.insert(param.clone(), Binding::Constant(atom));
                self.instances(action, remaining, bound, instances, transitions)?;
            }
            bound.remove(param);
            return Ok(());
        }
        let guard = self.conjunction(&action.guards, bound, false)?;
        instances.push(self.action(action, bound, transitions, guard)?);
        Ok(())
    }

    fn action(
        &self,
        action: &idealis_syntax::Action,
        bound: &Bindings,
        transitions: &[(usize, &Formula)],
        guard: Predicate,
    ) -> Result<Action, Unsupported> {
        let assignments = action
            .assignments
            .iter()
            .map(|(name, value)| {
                let Some(&(Slot::State(index), _, _)) = self.slots.get(name.as_str()) else {
                    return Err(Unsupported::new(format!("unknown state variable {name}")));
                };
                Ok((index, self.assignment(name, value, bound)?))
            })
            .collect::<Result<_, Unsupported>>()?;
        Ok(Action {
            state_guard: guard.reads_state(),
            guard,
            assignments,
            word: None,
            transitions: transitions
                .iter()
                .map(|(index, formula)| Ok((*index, self.predicate(formula, bound, false)?)))
                .collect::<Result<_, Unsupported>>()?,
        })
    }

    fn parameterized_action(
        &self,
        action: &idealis_syntax::Action,
        transitions: &[(usize, &Formula)],
    ) -> Result<ParameterizedAction, Unsupported> {
        let mut bound = Bindings::new();
        let domains = action
            .params
            .iter()
            .enumerate()
            .map(|(index, (param, sort))| {
                let (domain, support) = if let Some(layout) = self.indexed.get(sort.as_str()) {
                    (
                        self.refined.get(sort.as_str()).map_or_else(
                            || bits::Candidates::Ordinals {
                                start: 1,
                                count: layout.count,
                            },
                            |valid| bits::Candidates::Table(valid.to_vec()),
                        ),
                        0,
                    )
                } else {
                    let bits = self.sort(sort)?;
                    (
                        bits::Candidates::Table(bits.to_vec()),
                        bits.iter().fold(0, |mask, bit| mask | bit),
                    )
                };
                bound.insert(
                    param.clone(),
                    Binding::Parameter {
                        index,
                        sort: sort.clone(),
                        support,
                    },
                );
                Ok(domain)
            })
            .collect::<Result<Vec<_>, Unsupported>>()?;
        let count = candidate_product(&domains)
            .ok_or_else(|| Unsupported::new("argument binding count exceeds the index range"))?;
        let (before, after) = action.guard_parts();
        let compile_guards = |guards: Vec<&Formula>| {
            Ok::<_, Unsupported>(Predicate::all(
                guards
                    .into_iter()
                    .map(|guard| self.predicate(guard, &bound, false))
                    .collect::<Result<_, _>>()?,
            ))
        };
        let before_binding = compile_guards(before)?;
        let action = self.action(action, &bound, transitions, compile_guards(after)?)?;
        Ok(ParameterizedAction {
            before_binding,
            action,
            domains,
            count,
        })
    }

    fn temporal_instances(
        &self,
        question: &idealis_syntax::TemporalCheck,
        params: &[(String, String)],
        bound: &mut Bindings,
        instances: &mut Vec<temporal::Goal>,
    ) -> Result<(), Unsupported> {
        self.spend()?;
        if let Some(((param, sort), remaining)) = params.split_first() {
            for &atom in self.sort(sort)? {
                bound.insert(param.clone(), Binding::Constant(atom));
                self.temporal_instances(question, remaining, bound, instances)?;
            }
            bound.remove(param);
        } else {
            let trigger = question
                .trigger
                .as_ref()
                .map(|formula| self.predicate(formula, bound, false))
                .transpose()?;
            let response = self.predicate(&question.formula, bound, false)?;
            instances.push(temporal::Goal { trigger, response });
        }
        Ok(())
    }
}

fn counted_set(mut expr: &Expr, mut after: bool) -> Option<(&Expr, bool)> {
    while let Expr::At(phase, child) = expr {
        after = *phase == Phase::After;
        expr = child;
    }
    match expr {
        Expr::Count(set) => Some((set, after)),
        _ => None,
    }
}

fn sequence_operation(mut expr: &Expr) -> bool {
    while let Expr::At(_, child) = expr {
        expr = child;
    }
    matches!(
        expr,
        Expr::Append(..)
            | Expr::Tail(..)
            | Expr::Take(..)
            | Expr::RemoveAt(..)
            | Expr::RemoveAll(..)
    )
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
        if !action.transitions.is_empty() {
            return None;
        }
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
    lower_impl::<false>(model, None, InitialBudget::Comparable, false).map(|(compiled, _)| compiled)
}

pub(crate) fn lower_safety(model: &Model) -> Result<Compiled, Unsupported> {
    lower_impl::<false>(model, None, InitialBudget::Narrowed, false).map(|(compiled, _)| compiled)
}

pub(crate) fn lower_for_witness(
    model: &Model,
    force_argument_search: bool,
) -> Result<(Compiled, crate::witness::Layout<'_>), Unsupported> {
    lower_impl::<true>(model, None, InitialBudget::Narrowed, force_argument_search)
        .map(|(compiled, layout)| (compiled, layout.expect("requested layout")))
}

pub(crate) fn lower_temporal<'a>(
    model: &'a Model,
    question: &idealis_syntax::TemporalCheck,
) -> Result<(Compiled, crate::witness::Layout<'a>), Unsupported> {
    lower_impl::<true>(model, Some(question), InitialBudget::Narrowed, false)
        .map(|(compiled, layout)| (compiled, layout.expect("requested layout")))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InitialBudget {
    Comparable,
    Narrowed,
}

fn candidate_product(candidates: &[bits::Candidates]) -> Option<usize> {
    if candidates.iter().any(|values| values.len() == 0) {
        Some(0)
    } else {
        candidates
            .iter()
            .try_fold(1usize, |count, values| count.checked_mul(values.len()))
    }
}

fn legal_candidate(domain: &bits::Domain, value: u64) -> bool {
    if domain.is_indexed() {
        return domain.admits(value);
    }
    let mask = domain.tuples.iter().fold(0, |mask, tuple| mask | tuple);
    value & !mask == 0 && domain.admits(value)
}

fn narrowed_candidates(domain: &bits::Domain, fixed: Option<u64>) -> bits::Candidates {
    match fixed {
        Some(value) => bits::Candidates::Table(
            legal_candidate(domain, value)
                .then_some(value)
                .into_iter()
                .collect(),
        ),
        None => domain.candidates(),
    }
}

fn initial_search(
    init: &Predicate,
    state: &[bits::Domain],
    budget: InitialBudget,
    has_values: bool,
) -> Initial {
    let mut fixed = vec![None; state.len()];
    init.fixed_values(&mut |slot, value| {
        if let Slot::State(index) = slot {
            fixed[index] = Some(value);
        }
    });
    if let Some(values) = fixed.iter().copied().collect::<Option<Vec<_>>>() {
        let initial = values
            .iter()
            .zip(state)
            .all(|(&value, domain)| legal_candidate(domain, value))
            .then_some(values);
        let count = if budget == InitialBudget::Comparable && !has_values {
            let candidates: Vec<_> = state.iter().map(bits::Domain::candidates).collect();
            candidate_product(&candidates)
        } else {
            Some(usize::from(initial.is_some()))
        };
        return Initial {
            count,
            search: InitialSearch::Fixed(initial),
        };
    }
    let candidates: Vec<_> = state
        .iter()
        .zip(fixed)
        .map(|(domain, value)| {
            if budget == InitialBudget::Narrowed {
                narrowed_candidates(domain, value)
            } else {
                domain.candidates()
            }
        })
        .collect();
    Initial {
        count: candidate_product(&candidates),
        search: InitialSearch::Enumerated(candidates),
    }
}

fn lower_impl<'a, const TRACE: bool>(
    model: &'a Model,
    question: Option<&idealis_syntax::TemporalCheck>,
    initial_budget: InitialBudget,
    force_argument_search: bool,
) -> Result<(Compiled, Option<crate::witness::Layout<'a>>), Unsupported> {
    let mut compiler = Compiler::new(model, initial_budget)?;
    let mut config = Vec::new();
    let mut state = Vec::new();
    for relation in &model.relations {
        let domain = compiler.domain(&relation.shape)?;
        if let Some(bits::Rows::Indexed(rows)) = &domain.rows {
            compiler
                .indexed_rows
                .insert(&relation.name, Arc::clone(rows));
        }
        if let Shape::Unary { sig, .. } = &relation.shape
            && (model.record(sig).is_some()
                || model.variant(sig).is_some()
                || model.sequence(sig).is_some())
        {
            compiler.value_slots.insert(&relation.name);
        }
        if let Shape::Binary { to, .. } = &relation.shape
            && (model.record(to).is_some()
                || model.variant(to).is_some()
                || model.sequence(to).is_some())
            && !domain.is_indexed()
        {
            let columns = compiler
                .raw_sort(to)?
                .iter()
                .fold(0, |mask, bit| mask | bit);
            compiler.value_rows.insert(&relation.name, columns);
        }
        if domain.integer_mask.is_some() {
            if let Shape::Binary { to, .. } = &relation.shape {
                let columns = compiler.sort(to)?.iter().fold(0, |mask, bit| mask | bit);
                compiler.numeric_rows.insert(&relation.name, columns);
            } else {
                compiler.numeric_slots.insert(&relation.name);
            }
        }
        let width = if domain.rows.is_some() {
            Width::Binary
        } else {
            Width::Unary
        };
        let support = domain.tuples.iter().fold(0, |mask, tuple| mask | tuple);
        let slot = if relation.var {
            let index = state.len();
            state.push(domain);
            Slot::State(index)
        } else {
            let index = config.len();
            config.push(domain);
            Slot::Config(index)
        };
        compiler
            .slots
            .insert(&relation.name, (slot, width, support));
    }
    let mut scope = Bindings::new();
    let mut actions = Vec::new();
    let search_arguments = force_argument_search
        || model.actions.iter().any(|action| {
            action
                .params
                .iter()
                .any(|(_, sort)| compiler.indexed.contains_key(sort.as_str()))
        });
    for action in &model.actions {
        if action.params.len() > MAX_ACTION_PARAMETERS {
            return Err(Unsupported::new(format!(
                "an action may have at most {MAX_ACTION_PARAMETERS} parameters"
            )));
        }
        let mut instances = Vec::new();
        let transitions: Vec<_> = model
            .transitions
            .iter()
            .enumerate()
            .filter(|(_, check)| check.action.as_ref() == Some(&action.name))
            .map(|(index, check)| (index, &check.formula))
            .collect();
        if search_arguments {
            actions.push(ActionGroup {
                name: action.name.clone(),
                toggle: None,
                instances,
                parameterized: Some(compiler.parameterized_action(action, &transitions)?),
            });
            continue;
        }
        compiler.instances(
            action,
            &action.params,
            &mut scope,
            &mut instances,
            &transitions,
        )?;
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
            parameterized: None,
        });
    }
    let mut checks = Vec::new();
    for check in &model.checks {
        checks.push(Check {
            name: check.name.clone(),
            expects_true: check.kind == idealis_syntax::CheckKind::Reachable,
            predicate: compiler.predicate(&check.formula, &scope, false)?,
        });
    }
    let transitions = model
        .transitions
        .iter()
        .map(|check| check.name.clone())
        .collect();
    let global_transitions = model
        .transitions
        .iter()
        .enumerate()
        .filter(|(_, check)| check.action.is_none())
        .map(|(index, check)| Ok((index, compiler.predicate(&check.formula, &scope, false)?)))
        .collect::<Result<_, Unsupported>>()?;
    let init = compiler.conjunction(&model.init, &scope, false)?;
    let initial = initial_search(&init, &state, initial_budget, model.has_values());
    let temporal = question
        .map(|question| {
            if question.params.len() > MAX_ACTION_PARAMETERS {
                return Err(Unsupported::new("too many temporal parameters"));
            }
            let mut goals = Vec::new();
            compiler.temporal_instances(question, &question.params, &mut scope, &mut goals)?;
            let mut fairness = Vec::new();
            for rule in question.fairness.as_ref().unwrap_or(&model.fairness) {
                let group = actions
                    .iter()
                    .position(|group| group.name == rule.action)
                    .expect("checked fairness action");
                if rule.each {
                    let action = &actions[group];
                    let count = action
                        .parameterized
                        .as_ref()
                        .map_or(action.instances.len(), |plan| plan.count);
                    compiler.spend_nodes(count)?;
                    for instance in 0..count {
                        fairness.push(temporal::Rule {
                            kind: rule.kind,
                            group,
                            instance: Some(instance),
                        });
                    }
                } else {
                    fairness.push(temporal::Rule {
                        kind: rule.kind,
                        group,
                        instance: None,
                    });
                }
            }
            Ok(temporal::Question { goals, fairness })
        })
        .transpose()?;
    let facts = compiler.conjunction(&model.facts, &scope, false)?;
    let config = match initial_budget {
        InitialBudget::Comparable => config.iter().map(bits::Domain::candidates).collect(),
        InitialBudget::Narrowed => {
            let mut fixed = vec![None; config.len()];
            facts.fixed_values(&mut |slot, value| {
                if let Slot::Config(index) = slot {
                    fixed[index].get_or_insert(value);
                }
            });
            config
                .iter()
                .zip(fixed)
                .map(|(domain, value)| narrowed_candidates(domain, value))
                .collect()
        }
    };
    let compiled = Compiled {
        facts,
        init,
        initial,
        bitmap: match state.as_slice() {
            [only] if !only.is_indexed() => BitmapLayout::new(&only.tuples),
            _ => None,
        },
        state,
        config,
        actions,
        checks,
        transitions,
        global_transitions,
        temporal,
        checked_storage: model.has_values(),
    };
    let layout = TRACE.then(|| crate::witness::Layout {
        sorts: compiler.sorts,
        refined: compiler.refined,
        sparse: compiler.sparse.map(RefCell::into_inner),
        records: compiler.records,
        cases: compiler.cases,
        sequences: compiler.sequences,
        indexed: compiler.indexed,
        indexed_rows: compiler.indexed_rows,
    });
    Ok((compiled, layout))
}
