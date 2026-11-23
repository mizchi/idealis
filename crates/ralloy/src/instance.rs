use crate::eval::{Budget, Evaluator, World, admits, arrow_admits};
use crate::syntax::{Expr, Module, Parent};
use crate::{CheckReason, CheckStatus, Command, Error, ErrorKind, Instance, InstanceCheck, Stats};
use std::collections::BTreeMap;

enum Failure {
    Invalid(CheckReason),
    Mismatch,
    Unknown,
    Protocol(&'static str),
}
impl From<()> for Failure {
    fn from(_: ()) -> Self {
        Self::Unknown
    }
}

pub(crate) fn check(
    module: &Module,
    command: &Command,
    body: &Expr,
    instance: &Instance,
    max_work: u64,
) -> Result<InstanceCheck, Error> {
    let mut budget = Budget {
        stats: Stats::default(),
        limit: max_work,
    };
    let (status, reason) = match evaluate(module, command, body, instance, &mut budget) {
        Ok(()) => (CheckStatus::Matched, None),
        Err(Failure::Invalid(reason)) => (CheckStatus::Invalid, Some(reason)),
        Err(Failure::Mismatch) => (CheckStatus::Mismatch, Some(CheckReason::Property)),
        Err(Failure::Unknown) => (CheckStatus::Unknown, Some(CheckReason::Work)),
        Err(Failure::Protocol(message)) => return Err(Error::new(ErrorKind::Protocol, message)),
    };
    Ok(InstanceCheck {
        command: command.clone(),
        status,
        reason,
        stats: budget.stats,
    })
}

fn evaluate(
    module: &Module,
    command: &Command,
    body: &Expr,
    instance: &Instance,
    budget: &mut Budget,
) -> Result<(), Failure> {
    if instance.signatures.len() != module.signatures.len()
        || module
            .signatures
            .iter()
            .any(|signature| !instance.signatures.contains_key(&signature.name))
        || instance.fields.len() != module.fields.len()
        || module
            .fields
            .iter()
            .any(|field| !instance.fields.contains_key(&field.name))
    {
        return Err(Failure::Protocol(
            "instance must specify exactly the declared signatures and fields",
        ));
    }
    let mut world = World::new(command.scope.integer_bitwidth);
    let mut atoms: BTreeMap<_, _> = world
        .names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index))
        .collect();
    // Top-level signatures own the atoms; the others name atoms of their
    // parent (plan 0080).
    for &index in &module.hierarchy {
        let signature = &module.signatures[index];
        let bound = &command.scope.signatures[index];
        budget.tick()?;
        let values = &instance.signatures[&signature.name];
        if values.len() > bound.maximum || bound.exact && values.len() != bound.maximum {
            return Err(Failure::Invalid(CheckReason::Scope));
        }
        if !admits(signature.mult, values.len() as u32) {
            return Err(Failure::Invalid(CheckReason::Declaration));
        }
        let mut set = 0u64;
        for name in values {
            budget.tick()?;
            let atom = match &signature.parent {
                None => {
                    if name.is_empty()
                        || name.len() > 256
                        || atoms.insert(name.clone(), world.names.len()).is_some()
                    {
                        return Err(Failure::Protocol(
                            "atom names must be nonempty, unique and at most 256 bytes",
                        ));
                    }
                    world.names.push(name.clone());
                    world.names.len() - 1
                }
                Some((_, parent)) => {
                    let atom = *atoms
                        .get(name)
                        .ok_or(Failure::Protocol("unknown signature atom"))?;
                    if world.sets[parent] & (1 << atom) == 0 {
                        return Err(Failure::Invalid(CheckReason::Declaration));
                    }
                    atom
                }
            };
            if set & (1 << atom) != 0 {
                return Err(Failure::Protocol("duplicate signature atom"));
            }
            set |= 1u64 << atom;
        }
        world.sets.insert(signature.name.clone(), set);
    }
    // Children of one parent are disjoint, and an abstract parent with
    // children has no atoms of its own.
    for (index, signature) in module.signatures.iter().enumerate() {
        let children: Vec<u64> = module
            .signatures
            .iter()
            .filter(|child| {
                child.parent.as_ref().is_some_and(|(kind, parent)| {
                    *kind == Parent::Extends && *parent == module.signatures[index].name
                })
            })
            .map(|child| world.sets[&child.name])
            .collect();
        let union = children.iter().fold(0, |union, set| union | set);
        if children.iter().map(|set| set.count_ones()).sum::<u32>() != union.count_ones()
            || signature.abstract_ && !children.is_empty() && union != world.sets[&signature.name]
        {
            return Err(Failure::Invalid(CheckReason::Declaration));
        }
    }
    world.sets.insert(
        "univ".into(),
        if world.names.len() == 64 {
            u64::MAX
        } else {
            (1u64 << world.names.len()) - 1
        },
    );
    for field in &module.fields {
        let space = (field.columns.len() > 1).then(|| world.space(&field.columns));
        let mut rows = vec![0u64; world.names.len()];
        for tuple in &instance.fields[&field.name] {
            budget.tick()?;
            if tuple.len() != 1 + field.columns.len() {
                return Err(Failure::Protocol("field tuple arity"));
            }
            let atoms = tuple
                .iter()
                .map(|name| {
                    atoms
                        .get(name)
                        .copied()
                        .ok_or(Failure::Protocol("unknown field tuple atom"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let source = atoms[0];
            if world.sets[&field.owner] & (1 << source) == 0
                || field
                    .columns
                    .iter()
                    .zip(&atoms[1..])
                    .any(|(column, atom)| world.sets[column] & (1 << atom) == 0)
            {
                return Err(Failure::Invalid(CheckReason::Declaration));
            }
            let bit = match &space {
                None => atoms[1],
                Some(space) => space
                    .iter()
                    .position(|candidate| {
                        candidate
                            .iter()
                            .map(|atom| *atom as usize)
                            .eq(atoms[1..].iter().copied())
                    })
                    .expect("member of the column product"),
            };
            if rows[source] & (1 << bit) != 0 {
                return Err(Failure::Protocol("duplicate field tuple"));
            }
            rows[source] |= 1 << bit;
        }
        for (source, row) in rows.iter().enumerate() {
            budget.tick()?;
            if world.sets[&field.owner] & (1 << source) != 0 {
                let admitted = match (&space, field.arrow) {
                    (Some(space), Some(arrow)) => arrow_admits(
                        *row,
                        space,
                        world.sets[&field.columns[0]],
                        world.sets[&field.columns[1]],
                        arrow,
                    ),
                    _ => admits(field.mult, row.count_ones()),
                };
                if !admitted {
                    return Err(Failure::Invalid(CheckReason::Declaration));
                }
            }
        }
        world.fields.push(rows);
        world.spaces.push(space);
    }
    budget.stats.candidates = 1;
    let mut evaluator = Evaluator {
        module,
        world: &world,
        budget,
    };
    for fact in &module.facts {
        if !evaluator.formula(fact)? {
            return Err(Failure::Invalid(CheckReason::Facts));
        }
    }
    evaluator.budget.stats.fact_instances = 1;
    if !evaluator.formula(body)? {
        return Err(Failure::Mismatch);
    }
    Ok(())
}
