//! SAT backend for the relational core (plan 0085): bounded relational
//! logic translated to an and-inverter graph, Tseitin-encoded and solved by
//! the in-crate CDCL solver. Hierarchies and integer expressions are left to
//! the enumerator.
pub(crate) mod cdcl;
pub(crate) mod circuit;

use crate::eval::Budget;
use crate::syntax::{Binary, Declaration, Expr, Module, Mult, Parent, Quantifier, Unary};
use crate::{Analysis, Command, Instance, Kind, Stats, Status};
use circuit::{Circuit, FALSE, Gate, TRUE};
use std::collections::{BTreeMap, BTreeSet};

/// A relation: tuples of atom indices with the gate that holds them; absent
/// tuples are false.
#[derive(Clone)]
struct Relation {
    arity: usize,
    tuples: BTreeMap<Vec<u8>, Gate>,
}
impl Relation {
    fn new(arity: usize) -> Self {
        Self {
            arity,
            tuples: BTreeMap::new(),
        }
    }
    fn get(&self, tuple: &[u8]) -> Gate {
        self.tuples.get(tuple).copied().unwrap_or(FALSE)
    }
}
enum Value {
    Formula(Gate),
    Relation(Relation),
    /// Two's complement at the command's bitwidth (plan 0086).
    Integer(Vec<Gate>),
}
impl Value {
    fn formula(self) -> Gate {
        match self {
            Self::Formula(gate) => gate,
            _ => unreachable!("validated formula"),
        }
    }
    fn relation(self) -> Relation {
        match self {
            Self::Relation(relation) => relation,
            _ => unreachable!("validated relation"),
        }
    }
}

/// Whether the SAT backend covers a model: everything but integer division
/// and remainder (plans 0085, 0086).
pub(crate) fn supported(module: &Module) -> bool {
    fn plain(expression: &Expr, module: &Module) -> bool {
        match expression {
            Expr::Number(_) | Expr::Name(_) => true,
            // Division by zero is unknown in the enumerator; SAT cannot
            // reproduce that conditional outcome.
            Expr::Apply(name, arguments) => {
                (module.predicates.contains_key(name) || !matches!(name.as_str(), "div" | "rem"))
                    && arguments.iter().all(|argument| plain(argument, module))
            }
            Expr::Block(expressions) => expressions.iter().all(|e| plain(e, module)),
            Expr::Unary(_, child) => plain(child, module),
            Expr::Binary(_, left, right) => plain(left, module) && plain(right, module),
            Expr::Quantified(_, declarations, body) => {
                declarations.iter().all(|d| plain(&d.domain, module)) && plain(body, module)
            }
        }
    }
    module.facts.iter().all(|fact| plain(fact, module))
        && module.assertions.values().all(|body| plain(body, module))
        && module.predicates.values().all(|predicate| {
            plain(&predicate.body, module)
                && predicate
                    .params
                    .iter()
                    .all(|param| plain(&param.domain, module))
        })
        && module
            .commands
            .iter()
            .all(|command| plain(&command.body, module))
}

/// The enumerator's search width: the bits its field slots choose.
pub(crate) fn width(module: &Module, command: &Command) -> usize {
    let integers = if command.scope.integer_bitwidth == 0 {
        0
    } else {
        1usize << command.scope.integer_bitwidth
    };
    let maximum = |name: &str| match name {
        "Int" => integers,
        "String" => 0,
        name => command
            .scope
            .signatures
            .iter()
            .find(|scope| scope.name == name)
            .map_or(0, |scope| scope.maximum),
    };
    module
        .fields
        .iter()
        .map(|field| {
            maximum(&field.owner)
                * field
                    .columns
                    .iter()
                    .map(|column| maximum(column))
                    .product::<usize>()
        })
        .sum()
}

struct Translator<'a> {
    module: &'a Module,
    circuit: Circuit,
    budget: Budget,
    names: Vec<String>,
    signatures: BTreeMap<String, Relation>,
    fields: BTreeMap<String, Relation>,
    universe: Relation,
    /// Each signature's bound and exactness in this command.
    scopes: BTreeMap<String, (usize, bool)>,
    /// Integer bitwidth; Int atoms are the first 2^bitwidth.
    bitwidth: usize,
}

pub(crate) fn analyze(module: &Module, command: &Command, body: &Expr, max_work: u64) -> Analysis {
    let mut translator = Translator::new(module, command, max_work);
    let result = translator.solve(command, body);
    let status = match &result {
        Err(()) => Status::Unknown,
        Ok((status, _)) => *status,
    };
    let instance = result.ok().and_then(|(_, instance)| instance);
    let mut stats = translator.budget.stats;
    stats.candidates = u64::from(instance.is_some());
    Analysis {
        command: command.clone(),
        status,
        instance,
        stats,
    }
}

impl<'a> Translator<'a> {
    fn new(module: &'a Module, command: &Command, max_work: u64) -> Self {
        let integers = if command.scope.integer_bitwidth == 0 {
            0
        } else {
            1i32 << command.scope.integer_bitwidth
        };
        let mut names: Vec<String> = (0..integers)
            .map(|integer| (integer - integers / 2).to_string())
            .collect();
        let mut circuit = Circuit::new();
        let mut universe = Relation::new(1);
        for atom in 0..names.len() {
            universe.tuples.insert(vec![atom as u8], TRUE);
        }
        let mut signatures = BTreeMap::new();
        let mut ints = Relation::new(1);
        ints.tuples = universe.tuples.clone();
        signatures.insert("Int".to_owned(), ints);
        signatures.insert("String".to_owned(), Relation::new(1));
        // Top-level signatures own blocks of atoms; the others choose among
        // their parent's atoms (plan 0086).
        for &index in &module.hierarchy {
            let signature = &module.signatures[index];
            let scope = &command.scope.signatures[index];
            let mut relation = Relation::new(1);
            match &signature.parent {
                None => {
                    for position in 0..scope.maximum {
                        let atom = names.len() as u8;
                        names.push(format!("{}${position}", signature.name));
                        let gate = if scope.exact { TRUE } else { circuit.input() };
                        relation.tuples.insert(vec![atom], gate);
                        universe.tuples.insert(vec![atom], gate);
                    }
                }
                Some((_, parent)) => {
                    let atoms: Vec<Vec<u8>> = signatures[parent].tuples.keys().cloned().collect();
                    for atom in atoms {
                        relation.tuples.insert(atom, circuit.input());
                    }
                }
            }
            signatures.insert(signature.name.clone(), relation);
        }
        let mut translator = Self {
            module,
            circuit,
            budget: Budget {
                stats: Stats::default(),
                limit: max_work,
            },
            names,
            signatures,
            fields: BTreeMap::new(),
            universe,
            scopes: command
                .scope
                .signatures
                .iter()
                .map(|scope| (scope.name.clone(), (scope.maximum, scope.exact)))
                .collect(),
            bitwidth: command.scope.integer_bitwidth,
        };
        for field in &module.fields {
            let owner = translator.signatures[&field.owner].clone();
            let columns: Vec<Relation> = field
                .columns
                .iter()
                .map(|column| translator.signatures[column].clone())
                .collect();
            let mut relation = Relation::new(1 + columns.len());
            for owner_atom in owner.tuples.keys() {
                let mut space: Vec<Vec<u8>> = vec![vec![]];
                for column in &columns {
                    space = space
                        .into_iter()
                        .flat_map(|prefix| {
                            column.tuples.keys().map(move |atom| {
                                let mut tuple = prefix.clone();
                                tuple.push(atom[0]);
                                tuple
                            })
                        })
                        .collect();
                }
                for tuple in space {
                    let mut key = owner_atom.clone();
                    key.extend(&tuple);
                    let gate = translator.circuit.input();
                    relation.tuples.insert(key, gate);
                }
            }
            translator.fields.insert(field.name.clone(), relation);
        }
        translator
    }

    /// Declarations: signature multiplicities, field typing and field
    /// multiplicities.
    fn declarations(&mut self) -> Result<Gate, ()> {
        let mut constraints = Vec::new();
        for signature in &self.module.signatures {
            let members: Vec<Gate> = self.signatures[&signature.name]
                .tuples
                .values()
                .copied()
                .collect();
            constraints.push(match signature.mult {
                Mult::One => self.circuit.exactly_one(&members),
                Mult::Lone => self.circuit.at_most_one(&members),
                Mult::Some => self.circuit.any(members.iter().copied()),
                Mult::Set => TRUE,
            });
            let Some((_, parent)) = &signature.parent else {
                continue;
            };
            // Within the parent, at most the scope's bound, exactly it when
            // exact (plan 0086).
            let (maximum, exact) = self.scopes[&signature.name];
            constraints.push(if exact {
                self.circuit.exactly(&members, maximum)
            } else {
                self.circuit.at_most(&members, maximum)
            });
            for (atom, &gate) in &self.signatures[&signature.name].tuples.clone() {
                let inside = self.signatures[parent].get(atom);
                constraints.push(self.circuit.implies(gate, inside));
            }
        }
        // `extends` children of one parent are disjoint, and an abstract
        // parent with children has no atoms of its own.
        for parent in &self.module.signatures {
            let children: Vec<&String> = self
                .module
                .signatures
                .iter()
                .filter(|child| {
                    child.parent.as_ref().is_some_and(|(kind, name)| {
                        *kind == Parent::Extends && *name == parent.name
                    })
                })
                .map(|child| &child.name)
                .collect();
            if children.is_empty() {
                continue;
            }
            for (atom, &member) in &self.signatures[&parent.name].tuples.clone() {
                let gates: Vec<Gate> = children
                    .iter()
                    .map(|child| self.signatures[*child].get(atom))
                    .collect();
                constraints.push(self.circuit.at_most_one(&gates));
                if parent.abstract_ {
                    let some = self.circuit.any(gates);
                    constraints.push(self.circuit.implies(member, some));
                }
            }
        }
        for field in &self.module.fields {
            let relation = self.fields[&field.name].clone();
            let owner = self.signatures[&field.owner].clone();
            let columns: Vec<Relation> = field
                .columns
                .iter()
                .map(|column| self.signatures[column].clone())
                .collect();
            for (tuple, &gate) in &relation.tuples {
                self.budget.tick()?;
                let mut typed = owner.get(&tuple[..1]);
                for (column, atom) in columns.iter().zip(&tuple[1..]) {
                    typed = self.circuit.and(typed, column.get(&[*atom]));
                }
                constraints.push(self.circuit.implies(gate, typed));
            }
            for (owner_atom, &present) in &owner.tuples {
                let row: Vec<(&[u8], Gate)> = relation
                    .tuples
                    .iter()
                    .filter(|(tuple, _)| tuple[0] == owner_atom[0])
                    .map(|(tuple, gate)| (&tuple[1..], *gate))
                    .collect();
                let admitted = match field.arrow {
                    None => {
                        let gates: Vec<Gate> = row.iter().map(|(_, gate)| *gate).collect();
                        self.count(field.mult, &gates)
                    }
                    Some((left, right)) => {
                        let mut all = TRUE;
                        for (side, mult, column) in
                            [(0, right, &columns[0]), (1, left, &columns[1])]
                        {
                            for (atom, &member) in &column.tuples {
                                let gates: Vec<Gate> = row
                                    .iter()
                                    .filter(|(tuple, _)| tuple[side] == atom[0])
                                    .map(|(_, gate)| *gate)
                                    .collect();
                                let counted = self.count(mult, &gates);
                                let guarded = self.circuit.implies(member, counted);
                                all = self.circuit.and(all, guarded);
                            }
                        }
                        all
                    }
                };
                constraints.push(self.circuit.implies(present, admitted));
            }
        }
        Ok(self.circuit.all(constraints))
    }
    fn count(&mut self, mult: Mult, gates: &[Gate]) -> Gate {
        match mult {
            Mult::One => self.circuit.exactly_one(gates),
            Mult::Lone => self.circuit.at_most_one(gates),
            Mult::Some => self.circuit.any(gates.iter().copied()),
            Mult::Set => TRUE,
        }
    }

    fn solve(&mut self, command: &Command, body: &Expr) -> Result<(Status, Option<Instance>), ()> {
        let declarations = self.declarations()?;
        let mut facts = declarations;
        for fact in &self.module.facts {
            let gate = self.expression(fact, &BTreeMap::new())?.formula();
            facts = self.circuit.and(facts, gate);
        }
        let property = self.expression(body, &BTreeMap::new())?.formula();
        let root = match command.kind {
            Kind::Run => self.circuit.and(facts, property),
            Kind::Check => self.circuit.and(facts, property ^ 1),
        };
        if let Some(model) = self.satisfy(root)? {
            let status = match command.kind {
                Kind::Run => Status::Instance,
                Kind::Check => Status::Counterexample,
            };
            return Ok((status, Some(self.instance(&model))));
        }
        if command.kind == Kind::Run {
            return Ok((Status::NoInstance, None));
        }
        let status = if self.satisfy(facts)?.is_some() {
            Status::NoCounterexample
        } else {
            Status::NoInstance
        };
        Ok((status, None))
    }
    /// A model as the value of every circuit input, or `None`.
    fn satisfy(&mut self, root: Gate) -> Result<Option<Vec<bool>>, ()> {
        let (mut solver, inputs) = self.circuit.encode(root);
        Ok(solver.solve(&mut self.budget)?.map(|model| {
            inputs
                .iter()
                .map(|variable| variable.is_some_and(|variable| model[variable]))
                .collect()
        }))
    }
    fn holds(&self, gate: Gate, model: &[bool]) -> bool {
        let value = match gate >> 1 {
            0 => false,
            _ => model[self.circuit.input_index(gate).expect("input gate")],
        };
        value != (gate & 1 == 1)
    }
    fn instance(&self, model: &[bool]) -> Instance {
        // Parents first, so an atom keeps the name of its most specific
        // `extends` signature; subset signatures do not rename.
        let mut renamed = BTreeMap::new();
        for &position in &self.module.hierarchy {
            let signature = &self.module.signatures[position];
            if matches!(signature.parent, Some((Parent::In, _))) {
                continue;
            }
            let mut index = 0;
            for (tuple, &gate) in &self.signatures[&signature.name].tuples {
                if self.holds(gate, model) {
                    renamed.insert(tuple[0], format!("{}${index}", signature.name));
                    index += 1;
                }
            }
        }
        let name = |atom: u8| {
            renamed
                .get(&atom)
                .cloned()
                .unwrap_or_else(|| self.names[atom as usize].clone())
        };
        let signatures = self
            .module
            .signatures
            .iter()
            .map(|signature| {
                let atoms = self.signatures[&signature.name]
                    .tuples
                    .iter()
                    .filter(|(_, gate)| self.holds(**gate, model))
                    .map(|(tuple, _)| name(tuple[0]))
                    .collect();
                (signature.name.clone(), atoms)
            })
            .collect();
        let fields = self
            .module
            .fields
            .iter()
            .map(|field| {
                let tuples = self.fields[&field.name]
                    .tuples
                    .iter()
                    .filter(|(_, gate)| self.holds(**gate, model))
                    .map(|(tuple, _)| tuple.iter().map(|atom| name(*atom)).collect())
                    .collect();
                (field.name.clone(), tuples)
            })
            .collect();
        Instance { signatures, fields }
    }

    fn expression(
        &mut self,
        expression: &Expr,
        locals: &BTreeMap<String, Relation>,
    ) -> Result<Value, ()> {
        self.budget.tick()?;
        Ok(match expression {
            Expr::Name(name) => {
                if let Some(relation) = locals.get(name) {
                    Value::Relation(relation.clone())
                } else if name == "none" {
                    Value::Relation(Relation::new(1))
                } else if name == "univ" {
                    Value::Relation(self.universe.clone())
                } else if name == "iden" {
                    let mut iden = Relation::new(2);
                    for (tuple, gate) in &self.universe.tuples {
                        iden.tuples.insert(vec![tuple[0], tuple[0]], *gate);
                    }
                    Value::Relation(iden)
                } else if let Some(relation) = self.signatures.get(name) {
                    Value::Relation(relation.clone())
                } else if let Some(relation) = self.fields.get(name) {
                    Value::Relation(relation.clone())
                } else {
                    let body = &self.module.predicates[name].body;
                    self.expression(body, &BTreeMap::new())?
                }
            }
            Expr::Apply(name, arguments) if !self.module.predicates.contains_key(name) => {
                let mut values = Vec::new();
                for argument in arguments {
                    let value = self.expression(argument, locals)?;
                    values.push(self.integer(value));
                }
                Value::Integer(match name.as_str() {
                    "plus" => self.circuit.add(&values[0], &values[1]),
                    "minus" => self.circuit.subtract(&values[0], &values[1]),
                    "mul" => self.circuit.multiply(&values[0], &values[1]),
                    "negate" => self.circuit.negative(&values[0]),
                    _ => unreachable!("unsupported division"),
                })
            }
            Expr::Apply(name, arguments) => {
                let predicate = &self.module.predicates[name];
                let mut parameters = BTreeMap::new();
                let names = predicate.params.iter().flat_map(|param| &param.names);
                for (parameter, argument) in names.zip(arguments) {
                    let value = self.expression(argument, locals)?.relation();
                    parameters.insert(parameter.clone(), value);
                }
                self.expression(&predicate.body, &parameters)?
            }
            Expr::Number(value) => Value::Integer(Circuit::constant(*value, self.bitwidth)),
            Expr::Block(expressions) => {
                let mut all = TRUE;
                for expression in expressions {
                    let gate = self.expression(expression, locals)?.formula();
                    all = self.circuit.and(all, gate);
                }
                Value::Formula(all)
            }
            Expr::Unary(operator, child) => {
                let value = self.expression(child, locals)?;
                match operator {
                    Unary::Not => Value::Formula(value.formula() ^ 1),
                    Unary::No | Unary::Some | Unary::One | Unary::Lone => {
                        let gates: Vec<Gate> = value.relation().tuples.into_values().collect();
                        Value::Formula(match operator {
                            Unary::No => self.circuit.any(gates) ^ 1,
                            Unary::Some => self.circuit.any(gates),
                            Unary::One => self.circuit.exactly_one(&gates),
                            _ => self.circuit.at_most_one(&gates),
                        })
                    }
                    Unary::Transpose => {
                        let relation = value.relation();
                        let mut transposed = Relation::new(2);
                        for (tuple, gate) in relation.tuples {
                            transposed.tuples.insert(vec![tuple[1], tuple[0]], gate);
                        }
                        Value::Relation(transposed)
                    }
                    Unary::Closure | Unary::Reflexive => {
                        let mut closure = value.relation();
                        // Squaring doubles the path length each round.
                        let mut length = 1;
                        while length < self.names.len().max(1) {
                            let squared = self.join(&closure, &closure)?;
                            closure = self.union(&closure, &squared)?;
                            length *= 2;
                        }
                        if matches!(operator, Unary::Reflexive) {
                            let mut iden = Relation::new(2);
                            for (tuple, gate) in &self.universe.tuples {
                                iden.tuples.insert(vec![tuple[0], tuple[0]], *gate);
                            }
                            closure = self.union(&closure, &iden)?;
                        }
                        Value::Relation(closure)
                    }
                    Unary::Cardinality => {
                        let gates: Vec<Gate> = value.relation().tuples.into_values().collect();
                        Value::Integer(self.circuit.count(&gates, self.bitwidth))
                    }
                }
            }
            Expr::Binary(operator, left, right) => {
                let left = self.expression(left, locals)?;
                let right = self.expression(right, locals)?;
                if matches!(
                    operator,
                    Binary::And | Binary::Or | Binary::Implies | Binary::Iff
                ) {
                    let (left, right) = (left.formula(), right.formula());
                    return Ok(Value::Formula(match operator {
                        Binary::And => self.circuit.and(left, right),
                        Binary::Or => self.circuit.or(left, right),
                        Binary::Implies => self.circuit.implies(left, right),
                        _ => self.circuit.iff(left, right),
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
                        Binary::Less => self.circuit.less(&left, &right),
                        Binary::Greater => self.circuit.less(&right, &left),
                        Binary::LessEqual => self.circuit.less(&right, &left) ^ 1,
                        Binary::GreaterEqual => self.circuit.less(&left, &right) ^ 1,
                        Binary::Equal => self.circuit.equal(&left, &right),
                        _ => self.circuit.equal(&left, &right) ^ 1,
                    }));
                }
                let (left, right) = (left.relation(), right.relation());
                match operator {
                    Binary::Equal | Binary::NotEqual | Binary::In | Binary::NotIn => {
                        let keys: BTreeSet<&Vec<u8>> =
                            left.tuples.keys().chain(right.tuples.keys()).collect();
                        let mut all = TRUE;
                        for key in keys {
                            self.budget.tick()?;
                            let (l, r) = (left.get(key), right.get(key));
                            let gate = if matches!(operator, Binary::Equal | Binary::NotEqual) {
                                self.circuit.iff(l, r)
                            } else {
                                self.circuit.implies(l, r)
                            };
                            all = self.circuit.and(all, gate);
                        }
                        Value::Formula(if matches!(operator, Binary::NotEqual | Binary::NotIn) {
                            all ^ 1
                        } else {
                            all
                        })
                    }
                    Binary::Union => Value::Relation(self.union(&left, &right)?),
                    Binary::Intersection | Binary::Difference => {
                        let mut output = Relation::new(left.arity);
                        for (tuple, &gate) in &left.tuples {
                            self.budget.tick()?;
                            let other = right.get(tuple);
                            let other = if matches!(operator, Binary::Difference) {
                                other ^ 1
                            } else {
                                other
                            };
                            let both = self.circuit.and(gate, other);
                            if both != FALSE {
                                output.tuples.insert(tuple.clone(), both);
                            }
                        }
                        Value::Relation(output)
                    }
                    Binary::Product => {
                        let mut output = Relation::new(left.arity + right.arity);
                        for (prefix, &first) in &left.tuples {
                            for (suffix, &second) in &right.tuples {
                                self.budget.tick()?;
                                let both = self.circuit.and(first, second);
                                if both != FALSE {
                                    let tuple = prefix.iter().chain(suffix).copied().collect();
                                    output.tuples.insert(tuple, both);
                                }
                            }
                        }
                        Value::Relation(output)
                    }
                    Binary::Join => Value::Relation(self.join(&left, &right)?),
                    Binary::Domain | Binary::Range => {
                        let (set, relation, position) = if matches!(operator, Binary::Domain) {
                            (&left, &right, 0)
                        } else {
                            (&right, &left, usize::MAX)
                        };
                        let mut output = Relation::new(relation.arity);
                        for (tuple, &gate) in &relation.tuples {
                            self.budget.tick()?;
                            let atom = if position == 0 {
                                tuple[0]
                            } else {
                                *tuple.last().unwrap()
                            };
                            let kept = self.circuit.and(gate, set.get(&[atom]));
                            if kept != FALSE {
                                output.tuples.insert(tuple.clone(), kept);
                            }
                        }
                        Value::Relation(output)
                    }
                    _ => unreachable!("unsupported integer comparison"),
                }
            }
            Expr::Quantified(quantifier, declarations, body) => {
                let mut cases = Vec::new();
                self.expand(
                    declarations,
                    0,
                    0,
                    TRUE,
                    &mut Vec::new(),
                    locals,
                    body,
                    &mut cases,
                )?;
                Value::Formula(match quantifier {
                    Quantifier::All => {
                        let mut all = TRUE;
                        for (guard, value) in cases {
                            let gate = self.circuit.implies(guard, value);
                            all = self.circuit.and(all, gate);
                        }
                        all
                    }
                    _ => {
                        let matched: Vec<Gate> = cases
                            .into_iter()
                            .map(|(guard, value)| self.circuit.and(guard, value))
                            .collect();
                        match quantifier {
                            Quantifier::Some => self.circuit.any(matched),
                            Quantifier::No => self.circuit.any(matched) ^ 1,
                            Quantifier::One => self.circuit.exactly_one(&matched),
                            _ => self.circuit.at_most_one(&matched),
                        }
                    }
                })
            }
        })
    }
    /// Every binding of the declared names to one atom of its domain's upper
    /// bound, with the membership guard and the body's value.
    #[allow(clippy::too_many_arguments)]
    fn expand(
        &mut self,
        declarations: &[Declaration],
        declaration: usize,
        name: usize,
        guard: Gate,
        used: &mut Vec<u8>,
        locals: &BTreeMap<String, Relation>,
        body: &Expr,
        cases: &mut Vec<(Gate, Gate)>,
    ) -> Result<(), ()> {
        let Some(current) = declarations.get(declaration) else {
            let value = self.expression(body, locals)?.formula();
            cases.push((guard, value));
            return Ok(());
        };
        if name == current.names.len() {
            let mut fresh = Vec::new();
            return self.expand(
                declarations,
                declaration + 1,
                0,
                guard,
                &mut fresh,
                locals,
                body,
                cases,
            );
        }
        let domain = self.expression(&current.domain, locals)?.relation();
        for (tuple, &member) in &domain.tuples {
            self.budget.tick()?;
            if current.disjoint && used.contains(&tuple[0]) {
                continue;
            }
            let mut inner = locals.clone();
            let mut bound = Relation::new(1);
            bound.tuples.insert(tuple.clone(), TRUE);
            inner.insert(current.names[name].clone(), bound);
            let guarded = self.circuit.and(guard, member);
            used.push(tuple[0]);
            self.expand(
                declarations,
                declaration,
                name + 1,
                guarded,
                used,
                &inner,
                body,
                cases,
            )?;
            used.pop();
        }
        Ok(())
    }
    /// An integer, or the sum of the Int atoms of a unary relation.
    fn integer(&mut self, value: Value) -> Vec<Gate> {
        match value {
            Value::Integer(bits) => bits,
            Value::Relation(relation) => {
                let integers = if self.bitwidth == 0 {
                    0
                } else {
                    1i64 << self.bitwidth
                };
                let mut sum = Circuit::constant(0, self.bitwidth);
                for (tuple, &gate) in &relation.tuples {
                    let atom = i64::from(tuple[0]);
                    if atom >= integers {
                        continue;
                    }
                    let term: Vec<Gate> = Circuit::constant(atom - integers / 2, self.bitwidth)
                        .into_iter()
                        .map(|bit| self.circuit.and(bit, gate))
                        .collect();
                    sum = self.circuit.add(&sum, &term);
                }
                sum
            }
            Value::Formula(_) => unreachable!("validated integer"),
        }
    }
    fn union(&mut self, left: &Relation, right: &Relation) -> Result<Relation, ()> {
        let mut output = left.clone();
        for (tuple, &gate) in &right.tuples {
            self.budget.tick()?;
            let merged = self.circuit.or(output.get(tuple), gate);
            output.tuples.insert(tuple.clone(), merged);
        }
        Ok(output)
    }
    fn join(&mut self, left: &Relation, right: &Relation) -> Result<Relation, ()> {
        let mut heads: BTreeMap<u8, Vec<(&Vec<u8>, Gate)>> = BTreeMap::new();
        for (tuple, &gate) in &right.tuples {
            heads.entry(tuple[0]).or_default().push((tuple, gate));
        }
        let mut output = Relation::new(left.arity + right.arity - 2);
        for (prefix, &first) in &left.tuples {
            for (suffix, second) in heads.get(prefix.last().unwrap()).into_iter().flatten() {
                self.budget.tick()?;
                let both = self.circuit.and(first, *second);
                if both == FALSE {
                    continue;
                }
                let tuple: Vec<u8> = prefix[..prefix.len() - 1]
                    .iter()
                    .chain(&suffix[1..])
                    .copied()
                    .collect();
                let merged = self.circuit.or(output.get(&tuple), both);
                output.tuples.insert(tuple, merged);
            }
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::{analyze, supported};
    use crate::{CheckReason, CheckStatus, Kind, Model, Status, search::enumerate};

    /// On every corpus command the backend covers, SAT answers like the
    /// enumerator, and its instances pass the instance check.
    #[test]
    fn sat_agrees_with_the_enumerator_on_the_corpus() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/alloy");
        let (mut compared, mut instances, mut beyond) = (0, 0, 0);
        let mut paths: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .flat_map(|group| {
                std::fs::read_dir(group)
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
            })
            .collect();
        paths.sort();
        for path in paths {
            let model = Model::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
            if !supported(&model.syntax) {
                continue;
            }
            for index in 0..model.commands().len() {
                let (command, body) = model.get_command(index).unwrap();
                let label = format!("{} {}", path.display(), command.label);
                let solved = analyze(&model.syntax, command, body, 50_000_000);
                assert_ne!(solved.status, Status::Unknown, "{label}");
                if let Some(instance) = &solved.instance {
                    let check = model.check_instance(index, instance, 50_000_000).unwrap();
                    let expected = match command.kind {
                        Kind::Run => (CheckStatus::Matched, None),
                        Kind::Check => (CheckStatus::Mismatch, Some(CheckReason::Property)),
                    };
                    assert_eq!((check.status, check.reason), expected, "{label}");
                    instances += 1;
                }
                let enumerated = enumerate(&model.syntax, command, body, 5_000_000);
                if enumerated.status == Status::Unknown {
                    beyond += 1;
                    continue;
                }
                assert_eq!(solved.status, enumerated.status, "{label}");
                compared += 1;
            }
        }
        eprintln!("sat differential: {compared} compared, {instances} instances, {beyond} beyond");
        assert!(
            compared > 150 && instances > 90,
            "{compared} {instances} {beyond}"
        );
    }
}
