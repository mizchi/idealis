use crate::syntax::{Binary, Expr, Module, Mult, Unary};
use crate::{Command, Error, ErrorKind, Kind, Scope, SignatureScope};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
enum Type {
    Boolean,
    Relation(Vec<u16>),
}
impl Type {
    fn columns(self) -> Result<Vec<u16>, Error> {
        match self {
            Self::Relation(columns) => Ok(columns),
            _ => Err(Error::new(ErrorKind::Type, "expected relation")),
        }
    }
    fn boolean(self) -> Result<(), Error> {
        if matches!(self, Self::Boolean) {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::Type, "expected formula"))
        }
    }
}

/// Relations of higher arity are rejected as unsupported.
const MAX_ARITY: usize = 4;

struct Checker<'a> {
    module: &'a Module,
    globals: BTreeMap<String, Type>,
    visiting: BTreeSet<String>,
    validated: BTreeSet<String>,
    work: usize,
}

pub(crate) fn validate(module: &Module) -> Result<Vec<Command>, Error> {
    let mut checker = Checker {
        module,
        globals: BTreeMap::new(),
        visiting: BTreeSet::new(),
        validated: BTreeSet::new(),
        work: 0,
    };
    for (index, name) in module
        .signatures
        .iter()
        .map(|signature| signature.name.as_str())
        .chain(["Int", "String"])
        .enumerate()
    {
        if checker
            .globals
            .insert(name.to_owned(), Type::Relation(vec![1 << index]))
            .is_some()
        {
            return Err(Error::new(
                ErrorKind::Type,
                format!("duplicate signature {name}"),
            ));
        }
    }
    let all = (1 << (module.signatures.len() + 2)) - 1;
    checker
        .globals
        .insert("univ".into(), Type::Relation(vec![all]));
    checker
        .globals
        .insert("none".into(), Type::Relation(vec![all]));
    checker
        .globals
        .insert("iden".into(), Type::Relation(vec![all, all]));
    for field in &module.fields {
        let owner = checker
            .globals
            .get(&field.owner)
            .cloned()
            .ok_or_else(|| Error::new(ErrorKind::Type, "unknown owner"))?
            .columns()?[0];
        let mut columns = vec![owner];
        for column in &field.columns {
            if !module
                .signatures
                .iter()
                .any(|signature| &signature.name == column)
                && !["Int", "String"].contains(&column.as_str())
            {
                return Err(Error::new(
                    ErrorKind::Type,
                    format!("unknown field range {column}"),
                ));
            }
            columns.push(checker.globals[column].clone().columns()?[0]);
        }
        if checker
            .globals
            .insert(field.name.clone(), Type::Relation(columns))
            .is_some()
        {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "overloaded or duplicate field names",
            ));
        }
    }
    for name in module.predicates.keys().chain(module.assertions.keys()) {
        if checker.globals.contains_key(name)
            || module.predicates.contains_key(name) && module.assertions.contains_key(name)
        {
            return Err(Error::new(
                ErrorKind::Type,
                format!("duplicate declaration {name}"),
            ));
        }
    }
    for name in module.predicates.keys() {
        checker.predicate(name, 0)?;
    }
    for expression in module.facts.iter().chain(module.assertions.values()) {
        checker
            .expression(expression, &BTreeMap::new(), 0)?
            .boolean()?;
    }
    let mut commands = Vec::new();
    for command in &module.commands {
        let body = if let (Kind::Check, Expr::Name(name)) = (command.kind, &command.body) {
            module
                .assertions
                .get(name)
                .ok_or_else(|| Error::new(ErrorKind::Type, format!("unknown assertion {name}")))?
        } else {
            &command.body
        };
        checker.expression(body, &BTreeMap::new(), 0)?.boolean()?;
        if command.scope.bitwidth > 5 || command.scope.overall.is_some_and(|maximum| maximum > 8) {
            return Err(Error::new(
                ErrorKind::Resource,
                "supported scope: 0-8 atoms per signature, 0-5 integer bits",
            ));
        }
        let mut explicit = BTreeMap::new();
        for (name, maximum, exact) in &command.scope.entries {
            let signature = module
                .signatures
                .iter()
                .find(|signature| &signature.name == name)
                .ok_or_else(|| {
                    Error::new(ErrorKind::Scope, format!("unknown scoped signature {name}"))
                })?;
            if *maximum > 8 {
                return Err(Error::new(ErrorKind::Resource, "signature scope exceeds 8"));
            }
            if explicit.insert(name.clone(), (*maximum, *exact)).is_some() {
                return Err(Error::new(ErrorKind::Scope, "duplicate signature scope"));
            }
            if signature.mult == Mult::One && *maximum != 1
                || signature.mult == Mult::Lone && *maximum > 1
                || signature.mult == Mult::Some && *maximum == 0
            {
                return Err(Error::new(
                    ErrorKind::Scope,
                    "scope conflicts with signature multiplicity",
                ));
            }
        }
        let mut signatures = Vec::new();
        for signature in &module.signatures {
            let (maximum, exact) = match signature.mult {
                Mult::One => (1, true),
                Mult::Lone => explicit.get(&signature.name).copied().unwrap_or((1, false)),
                _ => explicit
                    .get(&signature.name)
                    .copied()
                    .or(command.scope.overall.map(|maximum| (maximum, false)))
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::Scope,
                            format!("missing scope for {}", signature.name),
                        )
                    })?,
            };
            signatures.push(SignatureScope {
                name: signature.name.clone(),
                maximum,
                exact,
            });
        }
        let integer_atoms = if command.scope.bitwidth == 0 {
            0
        } else {
            1 << command.scope.bitwidth
        };
        // A field of several columns chooses, per owner atom, a subset of the
        // product of its column signatures, kept as a 64-bit mask.
        for field in module.fields.iter().filter(|field| field.columns.len() > 1) {
            let space = field
                .columns
                .iter()
                .map(|column| match column.as_str() {
                    "Int" => integer_atoms,
                    "String" => 0,
                    name => signatures
                        .iter()
                        .find(|signature| signature.name == name)
                        .map_or(0, |signature| signature.maximum),
                })
                .product::<usize>();
            if space > 64 {
                return Err(Error::new(
                    ErrorKind::Resource,
                    format!("field {} exceeds 64 tuples per owner atom", field.name),
                ));
            }
        }
        if signatures
            .iter()
            .map(|signature| signature.maximum)
            .sum::<usize>()
            + integer_atoms
            > 64
        {
            return Err(Error::new(ErrorKind::Resource, "universe exceeds 64 atoms"));
        }
        commands.push(Command {
            label: command.label.clone(),
            kind: command.kind,
            scope: Scope {
                signatures,
                integer_bitwidth: command.scope.bitwidth,
            },
        });
    }
    Ok(commands)
}

impl Checker<'_> {
    fn predicate(&mut self, name: &str, depth: usize) -> Result<Type, Error> {
        if self.validated.contains(name) {
            return Ok(Type::Boolean);
        }
        let body = self
            .module
            .predicates
            .get(name)
            .ok_or_else(|| Error::new(ErrorKind::Type, format!("unknown predicate {name}")))?;
        if !self.visiting.insert(name.to_owned()) {
            return Err(Error::new(ErrorKind::Unsupported, "recursive predicates"));
        }
        self.expression(body, &BTreeMap::new(), depth + 1)?
            .boolean()?;
        self.visiting.remove(name);
        self.validated.insert(name.to_owned());
        Ok(Type::Boolean)
    }
    fn expression(
        &mut self,
        expression: &Expr,
        locals: &BTreeMap<String, Type>,
        depth: usize,
    ) -> Result<Type, Error> {
        self.work += 1;
        if depth > 64 || self.work > 100_000 {
            return Err(Error::new(ErrorKind::Resource, "type checking limit"));
        }
        match expression {
            Expr::Name(name) => {
                if let Some(kind) = locals.get(name).or(self.globals.get(name)) {
                    Ok(kind.clone())
                } else {
                    self.predicate(name, depth)
                }
            }
            Expr::Call(name) => self.predicate(name, depth),
            Expr::Block(expressions) => {
                for expression in expressions {
                    self.expression(expression, locals, depth + 1)?.boolean()?;
                }
                Ok(Type::Boolean)
            }
            Expr::Unary(operator, child) => {
                let kind = self.expression(child, locals, depth + 1)?;
                match operator {
                    Unary::Not => {
                        kind.boolean()?;
                        Ok(Type::Boolean)
                    }
                    Unary::No | Unary::Some | Unary::One | Unary::Lone => {
                        kind.columns()?;
                        Ok(Type::Boolean)
                    }
                    Unary::Transpose | Unary::Closure | Unary::Reflexive => {
                        let mut columns = kind.columns()?;
                        if columns.len() != 2 {
                            return Err(Error::new(
                                ErrorKind::Type,
                                "closure/transpose requires binary relation",
                            ));
                        }
                        if matches!(operator, Unary::Transpose) {
                            columns.swap(0, 1);
                        }
                        if matches!(operator, Unary::Reflexive) {
                            columns.fill(u16::MAX);
                        }
                        Ok(Type::Relation(columns))
                    }
                }
            }
            Expr::Binary(operator, left, right) => {
                let left = self.expression(left, locals, depth + 1)?;
                let right = self.expression(right, locals, depth + 1)?;
                if matches!(
                    operator,
                    Binary::And | Binary::Or | Binary::Implies | Binary::Iff
                ) {
                    left.boolean()?;
                    right.boolean()?;
                    return Ok(Type::Boolean);
                }
                let left_columns = left.columns()?;
                let right_columns = right.columns()?;
                if matches!(operator, Binary::Product) {
                    if left_columns.len() + right_columns.len() > MAX_ARITY {
                        return Err(Error::new(
                            ErrorKind::Unsupported,
                            "relations above arity four",
                        ));
                    }
                    return Ok(Type::Relation(
                        left_columns.into_iter().chain(right_columns).collect(),
                    ));
                }
                if matches!(operator, Binary::Join) {
                    // Alloy accepts a join of disjoint column types; it is
                    // empty. Only the arity is an error (plan 0079).
                    if left_columns.len() + right_columns.len() <= 2 {
                        return Err(Error::new(ErrorKind::Type, "ill-typed join"));
                    }
                    return Ok(Type::Relation(
                        left_columns[..left_columns.len() - 1]
                            .iter()
                            .chain(&right_columns[1..])
                            .copied()
                            .collect(),
                    ));
                }
                if matches!(operator, Binary::Domain | Binary::Range) {
                    let (set, relation) = if matches!(operator, Binary::Domain) {
                        (left_columns, right_columns)
                    } else {
                        (right_columns, left_columns)
                    };
                    if set.len() != 1 {
                        return Err(Error::new(ErrorKind::Type, "restriction requires a set"));
                    }
                    return Ok(Type::Relation(relation));
                }
                if left_columns.len() != right_columns.len() {
                    return Err(Error::new(ErrorKind::Type, "relation arity mismatch"));
                }
                if matches!(
                    operator,
                    Binary::Equal | Binary::NotEqual | Binary::In | Binary::NotIn
                ) {
                    return Ok(Type::Boolean);
                }
                Ok(Type::Relation(
                    left_columns
                        .into_iter()
                        .zip(right_columns)
                        .map(|(left, right)| left | right)
                        .collect(),
                ))
            }
            Expr::Quantified(_, declarations, body) => {
                let mut locals = locals.clone();
                let mut seen = BTreeSet::new();
                for declaration in declarations {
                    let domain = self
                        .expression(&declaration.domain, &locals, depth + 1)?
                        .columns()?;
                    if domain.len() != 1 {
                        return Err(Error::new(
                            ErrorKind::Unsupported,
                            "quantifier domain must be unary",
                        ));
                    }
                    for name in &declaration.names {
                        if !seen.insert(name) {
                            return Err(Error::new(
                                ErrorKind::Type,
                                "duplicate quantified variable",
                            ));
                        }
                        locals.insert(name.clone(), Type::Relation(domain.clone()));
                    }
                }
                self.expression(body, &locals, depth + 1)?.boolean()?;
                Ok(Type::Boolean)
            }
        }
    }
}
