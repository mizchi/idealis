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
    Ok(analyze_with(module, command, body, max_work, true, false).0)
}
/// The fields and signature sets of each candidate that reached evaluation.
type Trace = Vec<(Vec<u64>, Vec<Vec<u64>>)>;
/// `incremental` prunes during the search; otherwise every leaf is compared
/// against every swap before evaluation (plan 0083), as a reference.
fn analyze_with(
    module: &Module,
    command: &Command,
    body: &Expr,
    max_work: u64,
    incremental: bool,
    tracing: bool,
) -> (Analysis, Option<Trace>) {
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
        parents: Vec::new(),
        children: vec![Vec::new(); module.signatures.len()],
        symmetry: Vec::new(),
        lex: Vec::new(),
        undo: Vec::new(),
        incremental,
        separating: Vec::new(),
        positions: Vec::new(),
        trace: tracing.then(Vec::new),
    };
    search.parents = module
        .signatures
        .iter()
        .map(|signature| match &signature.parent {
            Some((Parent::Extends, parent)) => module
                .signatures
                .iter()
                .position(|candidate| candidate.name == *parent),
            _ => None,
        })
        .collect();
    for (child, parent) in search.parents.iter().enumerate() {
        if let Some(parent) = parent {
            search.children[*parent].push(child);
        }
    }
    let (subsets, own): (Vec<usize>, Vec<usize>) = module
        .hierarchy
        .iter()
        .copied()
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
    let trace = search.trace.take();
    (
        Analysis {
            command: command.clone(),
            status,
            instance: search.found,
            stats: search.budget.stats,
        },
        trace,
    )
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
    /// The `extends` parent of each signature and the children of each,
    /// computed once per analysis (plan 0080a).
    parents: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    /// Interchangeable atom pairs of the current world (plans 0083, 0084).
    symmetry: Vec<Symmetry>,
    /// Per pair: slots known equal under the swap, and whether the swap is
    /// already lexicographically larger.
    lex: Vec<(usize, bool)>,
    undo: Vec<(usize, (usize, bool))>,
    incremental: bool,
    /// Per sorted set name: whether membership separates atoms.
    separating: Vec<bool>,
    /// (field, owner atom) of each slot, in search order.
    positions: Vec<(usize, usize)>,
    trace: Option<Trace>,
}
struct Symmetry {
    first: usize,
    second: usize,
    /// The slot that each slot's owner maps to.
    partner: Vec<usize>,
    /// For multi-column fields, the tuple index each tuple maps to.
    permutations: Vec<Option<Vec<u8>>>,
}
/// The index each tuple of `space` maps to when `first` and `second` swap.
fn tuple_permutation(space: &[Vec<u8>], first: usize, second: usize) -> Vec<u8> {
    let swap = |atom: u8| match atom as usize {
        atom if atom == first => second as u8,
        atom if atom == second => first as u8,
        _ => atom,
    };
    space
        .iter()
        .map(|tuple| {
            let image: Vec<u8> = tuple.iter().map(|atom| swap(*atom)).collect();
            space.iter().position(|other| *other == image).unwrap() as u8
        })
        .collect()
}
impl Symmetry {
    /// A slot value with the two atoms swapped.
    fn image(&self, field: usize, value: u64) -> u64 {
        match &self.permutations[field] {
            None => {
                if (value >> self.first) & 1 == (value >> self.second) & 1 {
                    value
                } else {
                    value ^ (1 << self.first | 1 << self.second)
                }
            }
            Some(permutation) => {
                bits(value).fold(0, |image, index| image | 1 << permutation[index as usize])
            }
        }
    }
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
            if let Some(parent) = self.parents[index] {
                let siblings = self.order[..position]
                    .iter()
                    .filter(|other| self.parents[**other] == Some(parent))
                    .map(|other| counts[*other])
                    .sum::<usize>();
                let room = counts[parent].saturating_sub(siblings);
                maximum = maximum.min(room);
                let last = !self.order[position + 1..]
                    .iter()
                    .any(|other| self.parents[*other] == Some(parent));
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
        let mut used = 0;
        for &child in &self.children[index] {
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
            bitwidth: self.command.scope.integer_bitwidth,
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
        self.symmetries(&slots, &world);
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
                if self.descend(slots, index, world)? {
                    return Ok(true);
                }
            }
            let mut choices = slot.target;
            while choices != 0 {
                self.budget.tick()?;
                let choice = choices.isolate_lowest_one();
                choices &= !choice;
                world.fields[slot.field][slot.source] = choice;
                if self.descend(slots, index, world)? {
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
                if self.descend(slots, index, world)? {
                    return Ok(true);
                }
            }
            choice = choice.wrapping_sub(slot.target) & slot.target;
            if choice == 0 {
                return Ok(false);
            }
        }
    }
    /// Interchangeable atom pairs of this world and, per pair, the slot each
    /// slot maps to and the tuple permutation of multi-column fields.
    ///
    /// Formulas see atoms only through signatures and fields, so swapping two
    /// atoms that no fixed set tells apart maps solutions to solutions. Subset
    /// signatures are chosen by the search, so their membership is ignored
    /// unless they own a field or type one of its columns (plan 0084).
    fn symmetries(&mut self, slots: &[Slot], world: &World) {
        self.symmetry.clear();
        self.lex.clear();
        // Which sets separate atoms, aligned with the sorted set names; the
        // names are the same in every world of one analysis (plan 0084a).
        if self.separating.len() != world.sets.len() {
            self.separating = world
                .sets
                .keys()
                .map(|name| {
                    let subset = self.module.signatures.iter().any(|signature| {
                        &signature.name == name && matches!(signature.parent, Some((Parent::In, _)))
                    });
                    let used = self.module.fields.iter().any(|field| {
                        &field.owner == name || field.columns.iter().any(|column| column == name)
                    });
                    !subset || used
                })
                .collect();
        }
        let integers = if world.bitwidth == 0 {
            0
        } else {
            1 << world.bitwidth
        };
        let atoms = world.names.len();
        let pairs: Vec<usize> = (integers..atoms.saturating_sub(1))
            .filter(|&first| {
                world
                    .sets
                    .values()
                    .zip(&self.separating)
                    .filter(|(_, separating)| **separating)
                    .all(|(set, _)| (set >> first) & 1 == (set >> (first + 1)) & 1)
            })
            .collect();
        if pairs.is_empty() {
            return;
        }
        self.positions = slots.iter().map(|slot| (slot.field, slot.source)).collect();
        let mut position = vec![usize::MAX; self.module.fields.len() * atoms];
        for (index, slot) in slots.iter().enumerate() {
            position[slot.field * atoms + slot.source] = index;
        }
        for first in pairs {
            let second = first + 1;
            let swap = |atom: usize| match atom {
                atom if atom == first => second,
                atom if atom == second => first,
                atom => atom,
            };
            let partner = slots
                .iter()
                .map(|slot| position[slot.field * atoms + swap(slot.source)])
                .collect();
            let permutations = world
                .spaces
                .iter()
                .map(|space| {
                    space
                        .as_ref()
                        .map(|space| tuple_permutation(space, first, second))
                })
                .collect();
            self.symmetry.push(Symmetry {
                first,
                second,
                partner,
                permutations,
            });
            self.lex.push((0, false));
        }
    }
    /// Recurses past slot `index` unless some swap already makes the
    /// assignment lexicographically smaller; every orbit keeps its least
    /// member. The comparison state is restored on return.
    fn descend(&mut self, slots: &[Slot], index: usize, world: &mut World) -> Result<bool, ()> {
        let mark = self.undo.len();
        let pruned = if self.incremental {
            self.advance(slots, index, world)
        } else {
            Ok(false)
        };
        let result = match pruned {
            Ok(true) => Ok(false),
            Ok(false) => self.assign(slots, index + 1, world),
            Err(()) => Err(()),
        };
        while self.undo.len() > mark {
            let (symmetry, state) = self.undo.pop().expect("undo entry");
            self.lex[symmetry] = state;
        }
        result
    }
    /// The reference: whether some swap makes the complete assignment
    /// lexicographically smaller.
    fn redundant(&mut self, world: &World) -> Result<bool, ()> {
        for symmetry in &self.symmetry {
            self.budget.tick()?;
            for (position, &(field, source)) in self.positions.iter().enumerate() {
                let (partner_field, partner_source) = self.positions[symmetry.partner[position]];
                let actual = world.fields[field][source];
                let swapped = symmetry.image(field, world.fields[partner_field][partner_source]);
                if swapped != actual {
                    if swapped < actual {
                        return Ok(true);
                    }
                    break;
                }
            }
        }
        Ok(false)
    }
    /// Advances each swap's equal prefix over the slots assigned so far.
    fn advance(&mut self, slots: &[Slot], index: usize, world: &World) -> Result<bool, ()> {
        for symmetry in 0..self.symmetry.len() {
            let (mut frontier, done) = self.lex[symmetry];
            if done {
                continue;
            }
            let start = frontier;
            let mut decided = false;
            while frontier <= index {
                let partner = self.symmetry[symmetry].partner[frontier];
                if partner > index {
                    break;
                }
                self.budget.tick()?;
                let slot = &slots[frontier];
                let actual = world.fields[slot.field][slot.source];
                let source = &slots[partner];
                let swapped = self.symmetry[symmetry]
                    .image(slot.field, world.fields[source.field][source.source]);
                if swapped < actual {
                    self.undo.push((symmetry, (start, false)));
                    self.lex[symmetry] = (frontier, false);
                    return Ok(true);
                }
                if swapped > actual {
                    decided = true;
                    break;
                }
                frontier += 1;
            }
            if frontier != start || decided {
                self.undo.push((symmetry, (start, false)));
                self.lex[symmetry] = (frontier, decided);
            }
        }
        Ok(false)
    }
    fn candidate(&mut self, world: &World) -> Result<bool, ()> {
        self.budget.tick()?;
        if !self.incremental && self.redundant(world)? {
            return Ok(false);
        }
        if let Some(trace) = &mut self.trace {
            trace.push((world.sets.values().copied().collect(), world.fields.clone()));
        }
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

#[cfg(test)]
mod tests {
    use super::{Symmetry, analyze_with, tuple_permutation};
    use crate::eval::{World, bits};
    use crate::{Model, Status};
    use std::collections::BTreeSet;

    /// The image of a mask selects exactly the swapped tuples, checked
    /// against swapping the atoms of each tuple directly.
    #[test]
    fn images_of_multi_column_masks_swap_the_atoms_of_each_tuple() {
        let mut world = World::new(0);
        world.names = vec!["a".into(), "b".into(), "c".into()];
        world.sets.insert("A".into(), 0b111);
        let space = world.space(&["A".into(), "A".into()]);
        assert_eq!(space.len(), 9);
        for (first, second) in [(0, 1), (1, 2)] {
            let symmetry = Symmetry {
                first,
                second,
                partner: Vec::new(),
                permutations: vec![Some(tuple_permutation(&space, first, second))],
            };
            let swap = |atom: u8| match atom as usize {
                atom if atom == first => second as u8,
                atom if atom == second => first as u8,
                _ => atom,
            };
            for mask in 0u64..1 << space.len() {
                let swapped: BTreeSet<Vec<u8>> = bits(mask)
                    .map(|index| {
                        space[index as usize]
                            .iter()
                            .map(|atom| swap(*atom))
                            .collect()
                    })
                    .collect();
                let image: BTreeSet<Vec<u8>> = bits(symmetry.image(0, mask))
                    .map(|index| space[index as usize].clone())
                    .collect();
                assert_eq!(image, swapped, "{first} {second} {mask:b}");
            }
        }
    }

    /// Pruning during the search must reach exactly the leaves that the
    /// complete comparison at the leaf keeps, in the same order.
    #[test]
    fn incremental_pruning_visits_exactly_the_canonical_leaves() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/alloy");
        let mut compared = 0;
        let mut groups: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .collect();
        groups.sort();
        for group in groups {
            let mut models: Vec<_> = std::fs::read_dir(&group)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect();
            models.sort();
            for path in models {
                let model = Model::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
                for index in 0..model.commands().len() {
                    let (command, body) = model.get_command(index).unwrap();
                    let run = |incremental| {
                        analyze_with(&model.syntax, command, body, 3_000_000, incremental, true)
                    };
                    let (pruned, pruned_trace) = run(true);
                    let (reference, reference_trace) = run(false);
                    if pruned.status == Status::Unknown || reference.status == Status::Unknown {
                        continue;
                    }
                    let label = format!("{} {}", path.display(), command.label);
                    assert_eq!(pruned.status, reference.status, "{label}");
                    assert_eq!(pruned_trace, reference_trace, "{label}");
                    compared += 1;
                }
            }
        }
        assert!(compared > 150, "{compared}");
    }
}
