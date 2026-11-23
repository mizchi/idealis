use crate::eval::{Budget, Evaluator, World, admits, arrow_admits, bits};
use crate::syntax::{Expr, Module, Mult, Parent};
use crate::{Analysis, Command, Error, Instance, Kind, Stats, Status};
use std::collections::BTreeMap;

pub(crate) fn analyze(
    module: &Module,
    command: &Command,
    body: &Expr,
    max_work: u64,
) -> Result<Analysis, Error> {
    let mut search = Search {
        module,
        command,
        body,
        budget: Budget {
            stats: Stats::default(),
            limit: max_work,
        },
        found: None,
        order: Vec::new(),
        subset_order: Vec::new(),
    };
    let order = crate::check::hierarchy(module).expect("validated hierarchy");
    let (subsets, own): (Vec<usize>, Vec<usize>) = order
        .into_iter()
        .partition(|index| matches!(module.signatures[*index].parent, Some((Parent::In, _))));
    search.order = own;
    search.subset_order = subsets;
    let exhausted = search.counts(0, &mut vec![0; module.signatures.len()]);
    let status = match exhausted {
        Err(()) => Status::Unknown,
        Ok(true) if command.kind == Kind::Run => Status::Instance,
        Ok(true) => Status::Counterexample,
        Ok(false) if command.kind == Kind::Check && search.budget.stats.fact_instances > 0 => {
            Status::NoCounterexample
        }
        Ok(false) => Status::NoInstance,
    };
    Ok(Analysis {
        command: command.clone(),
        status,
        instance: search.found,
        stats: search.budget.stats,
    })
}
struct Search<'a> {
    module: &'a Module,
    command: &'a Command,
    body: &'a Expr,
    budget: Budget,
    found: Option<Instance>,
    /// Signatures with their own atoms, parents first; then subset ones.
    order: Vec<usize>,
    subset_order: Vec<usize>,
}
struct Slot {
    field: usize,
    source: usize,
    /// Target atoms of a single-column field, or the mask of every tuple of
    /// a multi-column field's space.
    target: u64,
    mult: Mult,
    /// For `A m -> n B`: the atoms of A and B and the multiplicities.
    arrow: Option<(u64, u64, (Mult, Mult))>,
}

impl Search<'_> {
    /// Chooses each signature's atom count, parents before children; an
    /// `extends` child fits in what its earlier siblings leave of the parent,
    /// and the last child of an abstract parent takes the rest (plan 0080).
    fn counts(&mut self, position: usize, counts: &mut Vec<usize>) -> Result<bool, ()> {
        self.budget.tick()?;
        if position < self.order.len() {
            let index = self.order[position];
            let signature = &self.module.signatures[index];
            let bound = &self.command.scope.signatures[index];
            let mut minimum = if bound.exact {
                bound.maximum
            } else if signature.mult == Mult::Some {
                1
            } else {
                0
            };
            let mut maximum = bound.maximum;
            if let Some((Parent::Extends, parent)) = &signature.parent {
                let parent = self.index(parent);
                let siblings = self.order[..position]
                    .iter()
                    .filter(|other| self.extends(**other) == Some(parent))
                    .map(|other| counts[*other])
                    .sum::<usize>();
                let room = counts[parent].saturating_sub(siblings);
                maximum = maximum.min(room);
                let last = !self.order[position + 1..]
                    .iter()
                    .any(|other| self.extends(*other) == Some(parent));
                if last && self.module.signatures[parent].abstract_ {
                    minimum = minimum.max(room);
                }
            }
            for count in minimum..=maximum {
                counts[index] = count;
                let result = self.counts(position + 1, counts)?;
                if result {
                    return Ok(true);
                }
            }
            counts[index] = 0;
            return Ok(false);
        }
        let World {
            mut names,
            mut sets,
            ..
        } = World::new(self.command.scope.integer_bitwidth);
        for (index, signature) in self.module.signatures.iter().enumerate() {
            if signature.parent.is_none() {
                self.allocate(index, counts, &mut names, &mut sets);
            }
        }
        sets.insert(
            "univ".into(),
            if names.len() == 64 {
                u64::MAX
            } else {
                (1u64 << names.len()) - 1
            },
        );
        self.subsets(0, names, sets)
    }
    fn index(&self, name: &str) -> usize {
        self.module
            .signatures
            .iter()
            .position(|signature| signature.name == name)
            .expect("validated signature")
    }
    fn extends(&self, index: usize) -> Option<usize> {
        match &self.module.signatures[index].parent {
            Some((Parent::Extends, parent)) => Some(self.index(parent)),
            _ => None,
        }
    }
    /// Gives a signature a contiguous block of atoms: its `extends` children
    /// first, then the atoms that belong to it alone.
    fn allocate(
        &self,
        index: usize,
        counts: &[usize],
        names: &mut Vec<String>,
        sets: &mut BTreeMap<String, u64>,
    ) {
        let start = names.len();
        let children: Vec<usize> = (0..self.module.signatures.len())
            .filter(|child| self.extends(*child) == Some(index))
            .collect();
        let mut used = 0;
        for child in children {
            used += counts[child];
            self.allocate(child, counts, names, sets);
        }
        let name = &self.module.signatures[index].name;
        for own in 0..counts[index] - used {
            names.push(format!("{name}${own}"));
        }
        let set = (start..names.len()).fold(0u64, |set, atom| set | 1 << atom);
        sets.insert(name.clone(), set);
    }
    /// Chooses every subset signature among its parent's atoms.
    fn subsets(
        &mut self,
        position: usize,
        names: Vec<String>,
        mut sets: BTreeMap<String, u64>,
    ) -> Result<bool, ()> {
        self.budget.tick()?;
        if let Some(&index) = self.subset_order.get(position) {
            let signature = &self.module.signatures[index];
            let Some((_, parent)) = &signature.parent else {
                unreachable!("subset signature")
            };
            let domain = sets[parent];
            let mut choice: u64 = 0;
            loop {
                self.budget.tick()?;
                if admits(signature.mult, choice.count_ones()) {
                    sets.insert(signature.name.clone(), choice);
                    if self.subsets(position + 1, names.clone(), sets.clone())? {
                        return Ok(true);
                    }
                }
                choice = choice.wrapping_sub(domain) & domain;
                if choice == 0 {
                    return Ok(false);
                }
            }
        }
        self.fields(names, sets)
    }
    fn fields(&mut self, names: Vec<String>, sets: BTreeMap<String, u64>) -> Result<bool, ()> {
        let mut world = World {
            fields: vec![vec![0; names.len()]; self.module.fields.len()],
            spaces: Vec::new(),
            names,
            sets,
        };
        let mut slots = Vec::new();
        for (field_index, field) in self.module.fields.iter().enumerate() {
            let (target, space) = if let [column] = field.columns.as_slice() {
                (world.sets[column], None)
            } else {
                let space = world.space(&field.columns);
                let mask = if space.len() == 64 {
                    u64::MAX
                } else {
                    (1u64 << space.len()) - 1
                };
                (mask, Some(space))
            };
            let arrow = field.arrow.map(|arrow| {
                (
                    world.sets[&field.columns[0]],
                    world.sets[&field.columns[1]],
                    arrow,
                )
            });
            world.spaces.push(space);
            for source in bits(world.sets[&field.owner]) {
                slots.push(Slot {
                    field: field_index,
                    source: source as usize,
                    target,
                    mult: field.mult,
                    arrow,
                });
            }
        }
        self.assign(&slots, 0, &mut world)
    }
    fn assign(&mut self, slots: &[Slot], index: usize, world: &mut World) -> Result<bool, ()> {
        self.budget.tick()?;
        if index == slots.len() {
            return self.candidate(world);
        }
        let slot = &slots[index];
        if matches!(slot.mult, Mult::One | Mult::Lone) {
            if slot.mult == Mult::Lone {
                world.fields[slot.field][slot.source] = 0;
                if self.assign(slots, index + 1, world)? {
                    return Ok(true);
                }
            }
            let mut choices = slot.target;
            while choices != 0 {
                self.budget.tick()?;
                let choice = choices.isolate_lowest_one();
                choices &= !choice;
                world.fields[slot.field][slot.source] = choice;
                if self.assign(slots, index + 1, world)? {
                    return Ok(true);
                }
            }
            return Ok(false);
        }
        let mut choice = 0;
        loop {
            self.budget.tick()?;
            let admitted = slot.arrow.is_none_or(|(left, right, arrow)| {
                let space = world.spaces[slot.field]
                    .as_deref()
                    .expect("arrow field space");
                arrow_admits(choice, space, left, right, arrow)
            });
            if (slot.mult == Mult::Set || choice != 0) && admitted {
                world.fields[slot.field][slot.source] = choice;
                if self.assign(slots, index + 1, world)? {
                    return Ok(true);
                }
            }
            choice = choice.wrapping_sub(slot.target) & slot.target;
            if choice == 0 {
                return Ok(false);
            }
        }
    }
    fn candidate(&mut self, world: &World) -> Result<bool, ()> {
        self.budget.tick()?;
        self.budget.stats.candidates += 1;
        let mut evaluator = Evaluator {
            module: self.module,
            world,
            budget: &mut self.budget,
        };
        for fact in &self.module.facts {
            if !evaluator.formula(fact)? {
                return Ok(false);
            }
        }
        evaluator.budget.stats.fact_instances += 1;
        let holds = evaluator.formula(self.body)?;
        if holds != (self.command.kind == Kind::Run) {
            return Ok(false);
        }
        let signatures = self
            .module
            .signatures
            .iter()
            .map(|signature| {
                let atoms = world
                    .names
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| world.sets[&signature.name] & (1 << index) != 0)
                    .map(|(_, name)| name.clone())
                    .collect();
                (signature.name.clone(), atoms)
            })
            .collect();
        let fields = self
            .module
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let mut tuples = Vec::new();
                for (source, row) in world.fields[index].iter().enumerate() {
                    for selected in bits(*row) {
                        let mut tuple = vec![world.names[source].clone()];
                        match &world.spaces[index] {
                            None => tuple.push(world.names[selected as usize].clone()),
                            Some(space) => tuple.extend(
                                space[selected as usize]
                                    .iter()
                                    .map(|atom| world.names[*atom as usize].clone()),
                            ),
                        }
                        tuples.push(tuple);
                    }
                }
                (field.name.clone(), tuples)
            })
            .collect();
        self.found = Some(Instance { signatures, fields });
        Ok(true)
    }
}
