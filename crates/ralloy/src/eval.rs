use crate::syntax::Mult;
use crate::{
    Stats,
    syntax::{Binary, Declaration, Expr, Module, Quantifier, Unary},
};
use std::collections::{BTreeMap, BTreeSet};

/// Unary and binary relations keep their bit representations; relations of
/// arity three and four are sets of atom-index tuples (plan 0079).
#[derive(Clone)]
pub(crate) enum Relation {
    Unary(u64),
    Binary(Vec<u64>),
    Nary(usize, BTreeSet<Vec<u8>>),
}
enum Value {
    Formula(bool),
    Relation(Relation),
    /// Already wrapped to the command's bitwidth (plan 0082).
    Integer(i64),
}
impl Value {
    fn formula(self) -> bool {
        let Self::Formula(value) = self else {
            unreachable!("validated formula")
        };
        value
    }
    fn relation(self) -> Relation {
        let Self::Relation(value) = self else {
            unreachable!("validated relation")
        };
        value
    }
}
impl Relation {
    fn rows(self) -> Vec<u64> {
        match self {
            Self::Binary(rows) => rows,
            _ => unreachable!("validated binary relation"),
        }
    }
    fn set(self) -> u64 {
        match self {
            Self::Unary(set) => set,
            _ => unreachable!("validated unary relation"),
        }
    }
    fn count(&self) -> u32 {
        match self {
            Self::Unary(set) => set.count_ones(),
            Self::Binary(rows) => rows.iter().map(|row| row.count_ones()).sum(),
            Self::Nary(_, tuples) => tuples.len() as u32,
        }
    }
    fn arity(&self) -> usize {
        match self {
            Self::Unary(_) => 1,
            Self::Binary(_) => 2,
            Self::Nary(arity, _) => *arity,
        }
    }
    /// The relation as a set of tuples, for operators across arities.
    fn tuples(self) -> BTreeSet<Vec<u8>> {
        match self {
            Self::Unary(set) => bits(set).map(|atom| vec![atom]).collect(),
            Self::Binary(rows) => rows
                .into_iter()
                .enumerate()
                .flat_map(|(source, row)| bits(row).map(move |target| vec![source as u8, target]))
                .collect(),
            Self::Nary(_, tuples) => tuples,
        }
    }
    /// Back to the bit representations when the arity allows it.
    fn from_tuples(arity: usize, tuples: BTreeSet<Vec<u8>>, atoms: usize) -> Self {
        match arity {
            1 => Self::Unary(tuples.iter().fold(0, |set, tuple| set | 1 << tuple[0])),
            2 => {
                let mut rows = vec![0; atoms];
                for tuple in &tuples {
                    rows[tuple[0] as usize] |= 1 << tuple[1];
                }
                Self::Binary(rows)
            }
            _ => Self::Nary(arity, tuples),
        }
    }
}
/// Atom indices of the set bits, lowest first.
pub(crate) fn bits(mut set: u64) -> impl Iterator<Item = u8> {
    std::iter::from_fn(move || {
        (set != 0).then(|| {
            let atom = set.trailing_zeros() as u8;
            set &= set - 1;
            atom
        })
    })
}
/// Whether a multiplicity admits `count` related atoms.
pub(crate) fn admits(mult: Mult, count: u32) -> bool {
    match mult {
        Mult::One => count == 1,
        Mult::Lone => count <= 1,
        Mult::Some => count >= 1,
        Mult::Set => true,
    }
}
/// Whether the tuples `mask` selects from `space` satisfy `A m -> n B`: every
/// atom of `left` maps to `n` atoms of B, every atom of `right` is reached
/// from `m` atoms of A.
pub(crate) fn arrow_admits(
    mask: u64,
    space: &[Vec<u8>],
    left: u64,
    right: u64,
    (m, n): (Mult, Mult),
) -> bool {
    let selected = || bits(mask).map(|index| &space[index as usize]);
    bits(left).all(|atom| {
        admits(
            n,
            selected().filter(|tuple| tuple[0] == atom).count() as u32,
        )
    }) && bits(right).all(|atom| {
        admits(
            m,
            selected().filter(|tuple| tuple[1] == atom).count() as u32,
        )
    })
}
pub(crate) struct Budget {
    pub stats: Stats,
    pub limit: u64,
}
impl Budget {
    pub fn tick(&mut self) -> Result<(), ()> {
        if self.stats.work == self.limit {
            return Err(());
        }
        self.stats.work += 1;
        Ok(())
    }
}
#[derive(Clone)]
pub(crate) struct World {
    /// Integer bitwidth; Int atoms are the first 2^bitwidth names.
    pub bitwidth: usize,
    pub names: Vec<String>,
    pub sets: BTreeMap<String, u64>,
    /// Per field and owner atom: the target set of a single-column field, or
    /// a mask over `spaces` for a field of several columns.
    pub fields: Vec<Vec<u64>>,
    /// The column tuples a field of several columns can relate an owner to.
    pub spaces: Vec<Option<Vec<Vec<u8>>>>,
}
impl World {
    pub fn new(bitwidth: usize) -> Self {
        let count = if bitwidth == 0 { 0 } else { 1i32 << bitwidth };
        Self {
            bitwidth,
            names: (0..count)
                .map(|integer| (integer - count / 2).to_string())
                .collect(),
            sets: BTreeMap::from([
                (
                    "Int".into(),
                    if count == 0 { 0 } else { (1u64 << count) - 1 },
                ),
                ("String".into(), 0),
            ]),
            fields: Vec::new(),
            spaces: Vec::new(),
        }
    }
    /// The product of the column signatures, in lexicographic order.
    pub fn space(&self, columns: &[String]) -> Vec<Vec<u8>> {
        let mut space = vec![vec![]];
        for column in columns {
            let set = self.sets[column];
            space = space
                .into_iter()
                .flat_map(|prefix: Vec<u8>| {
                    bits(set).map(move |atom| {
                        let mut tuple = prefix.clone();
                        tuple.push(atom);
                        tuple
                    })
                })
                .collect();
        }
        space
    }
    fn field(&self, index: usize, arity: usize) -> Relation {
        match &self.spaces[index] {
            None => Relation::Binary(self.fields[index].clone()),
            Some(space) => Relation::Nary(
                arity,
                self.fields[index]
                    .iter()
                    .enumerate()
                    .flat_map(|(owner, mask)| {
                        bits(*mask).map(move |index| {
                            let mut tuple = vec![owner as u8];
                            tuple.extend(&space[index as usize]);
                            tuple
                        })
                    })
                    .collect(),
            ),
        }
    }
}
type Environment = BTreeMap<String, Relation>;
pub(crate) struct Evaluator<'a> {
    pub module: &'a Module,
    /// The state being evaluated: `trace[time]`.
    pub world: &'a World,
    pub budget: &'a mut Budget,
    /// The lasso of a temporal model, or the single world (plan 0090).
    pub trace: &'a [World],
    pub time: usize,
    /// The state that follows the last one.
    pub back: usize,
}
impl Evaluator<'_> {
    pub fn formula(&mut self, expression: &Expr) -> Result<bool, ()> {
        self.expression(expression, &Environment::new())
            .map(Value::formula)
    }
    fn expression(&mut self, expression: &Expr, locals: &Environment) -> Result<Value, ()> {
        self.budget.tick()?;
        match expression {
            Expr::Name(name) => {
                if let Some(value) = locals.get(name) {
                    return Ok(Value::Relation(value.clone()));
                }
                if name == "none" {
                    return Ok(Value::Relation(Relation::Unary(0)));
                }
                if name == "iden" {
                    return Ok(Value::Relation(Relation::Binary(
                        (0..self.world.names.len())
                            .map(|index| 1 << index)
                            .collect(),
                    )));
                }
                if let Some(set) = self.world.sets.get(name) {
                    return Ok(Value::Relation(Relation::Unary(*set)));
                }
                if let Some(index) = self
                    .module
                    .fields
                    .iter()
                    .position(|field| &field.name == name)
                {
                    let arity = 1 + self.module.fields[index].columns.len();
                    return Ok(Value::Relation(self.world.field(index, arity)));
                }
                self.expression(&self.module.predicates[name].body, &Environment::new())
            }
            // Arguments are evaluated where the call is; the body sees only
            // the parameters (plan 0081).
            Expr::Apply(name, arguments) if !self.module.predicates.contains_key(name) => {
                let mut values = Vec::new();
                for argument in arguments {
                    let value = self.expression(argument, locals)?;
                    values.push(self.integer(value));
                }
                let (left, right) = (values[0], values.get(1).copied().unwrap_or(0));
                // Division and remainder by zero are not modeled: unknown.
                if matches!(name.as_str(), "div" | "rem") && right == 0 {
                    return Err(());
                }
                let value = match name.as_str() {
                    "plus" => left.wrapping_add(right),
                    "minus" => left.wrapping_sub(right),
                    "mul" => left.wrapping_mul(right),
                    "div" => left.wrapping_div(right),
                    "rem" => left.wrapping_rem(right),
                    _ => left.wrapping_neg(),
                };
                Ok(Value::Integer(self.wrap(value)))
            }
            Expr::Apply(name, arguments) => {
                let predicate = &self.module.predicates[name];
                let mut parameters = Environment::new();
                let names = predicate.params.iter().flat_map(|param| &param.names);
                for (parameter, argument) in names.zip(arguments) {
                    let value = self.expression(argument, locals)?.relation();
                    parameters.insert(parameter.clone(), value);
                }
                self.expression(&predicate.body, &parameters)
            }
            Expr::Block(expressions) => {
                for expression in expressions {
                    if !self.expression(expression, locals)?.formula() {
                        return Ok(Value::Formula(false));
                    }
                }
                Ok(Value::Formula(true))
            }
            Expr::Unary(
                operator @ (Unary::Always | Unary::Eventually | Unary::After | Unary::Prime),
                child,
            ) => {
                let now = self.time;
                let last = self.trace.len() - 1;
                let next = if now < last { now + 1 } else { self.back };
                // From position i the lasso reaches min(i, back)..=last.
                let positions: Vec<usize> = match operator {
                    Unary::After | Unary::Prime => vec![next],
                    _ => (now.min(self.back)..=last).collect(),
                };
                let (world, trace) = (self.world, self.trace);
                let mut result = None;
                let mut found = matches!(operator, Unary::Always);
                for position in positions {
                    self.budget.tick()?;
                    self.time = position;
                    self.world = &trace[position];
                    let value = self.expression(child, locals);
                    self.time = now;
                    self.world = world;
                    let value = value?;
                    match operator {
                        Unary::Prime => result = Some(value),
                        Unary::After => found = value.formula(),
                        Unary::Always if !value.formula_ref() => {
                            found = false;
                            break;
                        }
                        Unary::Eventually if value.formula_ref() => {
                            found = true;
                            break;
                        }
                        _ => {}
                    }
                }
                Ok(result.unwrap_or(Value::Formula(found)))
            }
            Expr::Unary(operator, child) => {
                let value = self.expression(child, locals)?;
                if matches!(operator, Unary::Not) {
                    return Ok(Value::Formula(!value.formula()));
                }
                let relation = value.relation();
                if matches!(operator, Unary::Cardinality) {
                    return Ok(Value::Integer(self.wrap(i64::from(relation.count()))));
                }
                if matches!(operator, Unary::No | Unary::Some | Unary::One | Unary::Lone) {
                    let count = relation.count();
                    return Ok(Value::Formula(match operator {
                        Unary::No => count == 0,
                        Unary::Some => count > 0,
                        Unary::One => count == 1,
                        _ => count <= 1,
                    }));
                }
                let mut rows = relation.rows();
                if matches!(operator, Unary::Transpose) {
                    let mut transposed = vec![0; rows.len()];
                    for (source, row) in rows.iter().enumerate() {
                        for (target, result) in transposed.iter_mut().enumerate() {
                            self.budget.tick()?;
                            if row & (1 << target) != 0 {
                                *result |= 1 << source;
                            }
                        }
                    }
                    return Ok(Value::Relation(Relation::Binary(transposed)));
                }
                for middle in 0..rows.len() {
                    let successors = rows[middle];
                    for row in &mut rows {
                        self.budget.tick()?;
                        if *row & (1 << middle) != 0 {
                            *row |= successors;
                        }
                    }
                }
                if matches!(operator, Unary::Reflexive) {
                    for (index, row) in rows.iter_mut().enumerate() {
                        *row |= 1 << index;
                    }
                }
                Ok(Value::Relation(Relation::Binary(rows)))
            }
            Expr::Binary(operator, left, right) => {
                let left = self.expression(left, locals)?;
                if matches!(operator, Binary::And) && !left.formula_ref() {
                    return Ok(Value::Formula(false));
                }
                if matches!(operator, Binary::Or) && left.formula_ref() {
                    return Ok(Value::Formula(true));
                }
                if matches!(operator, Binary::Implies) && !left.formula_ref() {
                    return Ok(Value::Formula(true));
                }
                let right = self.expression(right, locals)?;
                if matches!(
                    operator,
                    Binary::And | Binary::Or | Binary::Implies | Binary::Iff
                ) {
                    let left = left.formula();
                    let right = right.formula();
                    return Ok(Value::Formula(match operator {
                        Binary::And => left && right,
                        Binary::Or => left || right,
                        Binary::Implies => !left || right,
                        _ => left == right,
                    }));
                }
                let integers =
                    matches!(left, Value::Integer(_)) || matches!(right, Value::Integer(_));
                if matches!(
                    operator,
                    Binary::Less | Binary::LessEqual | Binary::Greater | Binary::GreaterEqual
                ) || integers && matches!(operator, Binary::Equal | Binary::NotEqual)
                {
                    let (left, right) = (self.integer(left), self.integer(right));
                    return Ok(Value::Formula(match operator {
                        Binary::Less => left < right,
                        Binary::LessEqual => left <= right,
                        Binary::Greater => left > right,
                        Binary::GreaterEqual => left >= right,
                        Binary::Equal => left == right,
                        _ => left != right,
                    }));
                }
                let left = left.relation();
                let right = right.relation();
                self.relations(*operator, left, right)
            }
            Expr::Number(value) => Ok(Value::Integer(self.wrap(*value))),
            Expr::Quantified(quantifier, declarations, body) => {
                let mut matched = 0;
                self.quantify(*quantifier, declarations, 0, body, locals, &mut matched)?;
                Ok(Value::Formula(match quantifier {
                    Quantifier::All | Quantifier::No => matched == 0,
                    Quantifier::Some => matched > 0,
                    Quantifier::One => matched == 1,
                    Quantifier::Lone => matched <= 1,
                }))
            }
        }
    }
    fn relations(
        &mut self,
        operator: Binary,
        left: Relation,
        right: Relation,
    ) -> Result<Value, ()> {
        let count = self.world.names.len();
        let across = matches!(left, Relation::Nary(..))
            || matches!(right, Relation::Nary(..))
            || matches!(operator, Binary::Product) && (left.arity(), right.arity()) != (1, 1)
            || matches!(operator, Binary::Domain) && right.arity() == 1
            || matches!(operator, Binary::Range) && left.arity() == 1;
        if across {
            return self.tuples(operator, left, right);
        }
        if matches!(operator, Binary::Product) {
            let left = left.set();
            let right = right.set();
            return Ok(Value::Relation(Relation::Binary(
                (0..count)
                    .map(|index| if left & (1 << index) != 0 { right } else { 0 })
                    .collect(),
            )));
        }
        if matches!(operator, Binary::Domain | Binary::Range) {
            let (set, mut rows) = if matches!(operator, Binary::Domain) {
                (left.set(), right.rows())
            } else {
                (right.set(), left.rows())
            };
            for (index, row) in rows.iter_mut().enumerate() {
                self.budget.tick()?;
                if matches!(operator, Binary::Domain) {
                    if set & (1 << index) == 0 {
                        *row = 0;
                    }
                } else {
                    *row &= set;
                }
            }
            return Ok(Value::Relation(Relation::Binary(rows)));
        }
        if matches!(operator, Binary::Join) {
            let result = match (left, right) {
                (Relation::Unary(set), Relation::Binary(rows)) => {
                    let mut output = 0;
                    for (index, row) in rows.into_iter().enumerate() {
                        self.budget.tick()?;
                        if set & (1 << index) != 0 {
                            output |= row;
                        }
                    }
                    Relation::Unary(output)
                }
                (Relation::Binary(rows), Relation::Unary(set)) => {
                    let mut output = 0;
                    for (index, row) in rows.into_iter().enumerate() {
                        self.budget.tick()?;
                        if row & set != 0 {
                            output |= 1 << index;
                        }
                    }
                    Relation::Unary(output)
                }
                (Relation::Binary(left), Relation::Binary(right)) => {
                    let mut output = vec![0; count];
                    for (source, row) in left.into_iter().enumerate() {
                        for (middle, successors) in right.iter().enumerate() {
                            self.budget.tick()?;
                            if row & (1 << middle) != 0 {
                                output[source] |= successors;
                            }
                        }
                    }
                    Relation::Binary(output)
                }
                _ => unreachable!("validated join"),
            };
            return Ok(Value::Relation(result));
        }
        let binary = matches!(left, Relation::Binary(_)) || matches!(right, Relation::Binary(_));
        let (left, right) = if binary {
            (left.rows(), right.rows())
        } else {
            (vec![left.set()], vec![right.set()])
        };
        if matches!(
            operator,
            Binary::Equal | Binary::NotEqual | Binary::In | Binary::NotIn
        ) {
            let subset = matches!(operator, Binary::In | Binary::NotIn);
            let mut satisfied = true;
            for (left, right) in left.iter().zip(&right) {
                self.budget.tick()?;
                satisfied &= if subset {
                    left & !right == 0
                } else {
                    left == right
                };
            }
            return Ok(Value::Formula(
                satisfied != matches!(operator, Binary::NotEqual | Binary::NotIn),
            ));
        }
        let mut output = Vec::new();
        for (left, right) in left.into_iter().zip(right) {
            self.budget.tick()?;
            output.push(match operator {
                Binary::Union => left | right,
                Binary::Difference => left & !right,
                Binary::Intersection => left & right,
                _ => unreachable!("validated relational operator"),
            });
        }
        Ok(Value::Relation(if binary {
            Relation::Binary(output)
        } else {
            Relation::Unary(output[0])
        }))
    }
    /// Operators on sets of tuples, for operands of arity above two or
    /// results whose arity differs from the bit representations.
    fn tuples(&mut self, operator: Binary, left: Relation, right: Relation) -> Result<Value, ()> {
        let atoms = self.world.names.len();
        let (left_arity, right_arity) = (left.arity(), right.arity());
        let (left, right) = (left.tuples(), right.tuples());
        for _ in 0..left.len() + right.len() {
            self.budget.tick()?;
        }
        let (arity, output) = match operator {
            Binary::Equal => return Ok(Value::Formula(left == right)),
            Binary::NotEqual => return Ok(Value::Formula(left != right)),
            Binary::In => return Ok(Value::Formula(left.is_subset(&right))),
            Binary::NotIn => return Ok(Value::Formula(!left.is_subset(&right))),
            Binary::Union => (left_arity, &left | &right),
            Binary::Difference => (left_arity, &left - &right),
            Binary::Intersection => (left_arity, &left & &right),
            Binary::Product => {
                let mut output = BTreeSet::new();
                for prefix in &left {
                    for suffix in &right {
                        self.budget.tick()?;
                        output.insert(prefix.iter().chain(suffix).copied().collect());
                    }
                }
                (left_arity + right_arity, output)
            }
            Binary::Join => {
                let mut heads: BTreeMap<u8, Vec<&Vec<u8>>> = BTreeMap::new();
                for tuple in &right {
                    heads.entry(tuple[0]).or_default().push(tuple);
                }
                let mut output = BTreeSet::new();
                for prefix in &left {
                    for suffix in heads.get(prefix.last().unwrap()).into_iter().flatten() {
                        self.budget.tick()?;
                        output.insert(
                            prefix[..prefix.len() - 1]
                                .iter()
                                .chain(&suffix[1..])
                                .copied()
                                .collect(),
                        );
                    }
                }
                (left_arity + right_arity - 2, output)
            }
            Binary::Domain => {
                let set: BTreeSet<u8> = left.iter().map(|tuple| tuple[0]).collect();
                (
                    right_arity,
                    right
                        .into_iter()
                        .filter(|tuple| set.contains(&tuple[0]))
                        .collect(),
                )
            }
            Binary::Range => {
                let set: BTreeSet<u8> = right.iter().map(|tuple| tuple[0]).collect();
                (
                    left_arity,
                    left.into_iter()
                        .filter(|tuple| set.contains(tuple.last().unwrap()))
                        .collect(),
                )
            }
            _ => unreachable!("validated relational operator"),
        };
        Ok(Value::Relation(Relation::from_tuples(arity, output, atoms)))
    }
    /// Two's complement at the command's bitwidth, as Alloy 6.2.0 answers.
    fn wrap(&self, value: i64) -> i64 {
        let bitwidth = self.world.bitwidth;
        if bitwidth == 0 {
            return 0;
        }
        let modulus = 1i64 << bitwidth;
        (value + modulus / 2).rem_euclid(modulus) - modulus / 2
    }
    /// An integer, or the sum of the Int atoms of a unary relation.
    fn integer(&self, value: Value) -> i64 {
        match value {
            Value::Integer(value) => value,
            Value::Relation(Relation::Unary(set)) => {
                let integers = if self.world.bitwidth == 0 {
                    0
                } else {
                    1i64 << self.world.bitwidth
                };
                let sum = bits(set)
                    .map(i64::from)
                    .filter(|atom| *atom < integers)
                    .map(|atom| atom - integers / 2)
                    .sum();
                self.wrap(sum)
            }
            _ => unreachable!("validated integer"),
        }
    }
    fn quantify(
        &mut self,
        quantifier: Quantifier,
        declarations: &[Declaration],
        index: usize,
        body: &Expr,
        locals: &Environment,
        matched: &mut u64,
    ) -> Result<bool, ()> {
        self.budget.tick()?;
        if index == declarations.len() {
            let value = self.expression(body, locals)?.formula();
            if value != matches!(quantifier, Quantifier::All) {
                *matched += 1;
            }
            return Ok(match quantifier {
                Quantifier::All | Quantifier::No | Quantifier::Some => *matched > 0,
                Quantifier::One | Quantifier::Lone => *matched > 1,
            });
        }
        let declaration = &declarations[index];
        let domain = self
            .expression(&declaration.domain, locals)?
            .relation()
            .set();
        self.bind(
            quantifier,
            declarations,
            index,
            0,
            domain,
            0,
            body,
            locals,
            matched,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn bind(
        &mut self,
        quantifier: Quantifier,
        declarations: &[Declaration],
        declaration_index: usize,
        name_index: usize,
        domain: u64,
        used: u64,
        body: &Expr,
        locals: &Environment,
        matched: &mut u64,
    ) -> Result<bool, ()> {
        let declaration = &declarations[declaration_index];
        if name_index == declaration.names.len() {
            return self.quantify(
                quantifier,
                declarations,
                declaration_index + 1,
                body,
                locals,
                matched,
            );
        }
        let mut choices = if declaration.disjoint {
            domain & !used
        } else {
            domain
        };
        while choices != 0 {
            self.budget.tick()?;
            let atom = choices.isolate_lowest_one();
            choices &= !atom;
            let mut locals = locals.clone();
            locals.insert(declaration.names[name_index].clone(), Relation::Unary(atom));
            if self.bind(
                quantifier,
                declarations,
                declaration_index,
                name_index + 1,
                domain,
                used | atom,
                body,
                &locals,
                matched,
            )? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
impl Value {
    fn formula_ref(&self) -> bool {
        let Self::Formula(value) = self else {
            unreachable!("validated formula")
        };
        *value
    }
}
