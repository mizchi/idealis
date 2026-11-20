use crate::ir::{Expression, SafetyProgram, TemporalFormula};
use crate::safety_expressions::register_branch_memberships;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

#[derive(Default)]
struct AddressHasher(u64);

impl Hasher for AddressHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3) ^ u64::from(*byte);
        }
    }

    fn write_usize(&mut self, value: usize) {
        // MurmurHash3 fmix64 by Austin Appleby (public domain), for internal addresses only.
        let mut hash = value as u64;
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        self.0 = hash ^ (hash >> 33);
    }
}

pub(crate) struct PreparedProgram {
    slots: HashMap<usize, usize, BuildHasherDefault<AddressHasher>>,
}

impl PreparedProgram {
    pub fn new(program: &SafetyProgram) -> Self {
        let mut roots = Vec::new();
        for definition in program.definitions.iter().flatten() {
            match &definition.body {
                crate::ir::DefinitionBody::Temporal(body) => {
                    crate::safety_expressions::register_formula_memberships(
                        body,
                        &mut |expression| roots.push(expression),
                    );
                }
                crate::ir::DefinitionBody::Expression(body) => roots.push(body),
                crate::ir::DefinitionBody::Action(branches) => {
                    register_branch_memberships(branches, &mut |expression| roots.push(expression))
                }
            }
        }
        register_branch_memberships(&program.initial, &mut |expression| roots.push(expression));
        register_branch_memberships(&program.next, &mut |expression| roots.push(expression));
        roots.extend(
            program
                .invariants
                .iter()
                .chain(&program.assumptions)
                .chain(&program.initial_properties)
                .chain(&program.action_properties)
                .chain(&program.constraints)
                .chain(&program.action_constraints)
                .map(|predicate| &predicate.expression),
        );
        let mut formulas = program
            .properties
            .iter()
            .map(|property| &property.formula)
            .chain(&program.fairness)
            .collect::<Vec<_>>();
        while let Some(formula) = formulas.pop() {
            match formula {
                TemporalFormula::Call {
                    arguments,
                    captures,
                    ..
                } => roots.extend(arguments.iter().chain(captures)),
                TemporalFormula::Atom { expression } => roots.push(expression),
                TemporalFormula::Not { formula }
                | TemporalFormula::Always { formula }
                | TemporalFormula::Eventually { formula } => formulas.push(formula),
                TemporalFormula::And { left, right } | TemporalFormula::Or { left, right } => {
                    formulas.extend([left.as_ref(), right.as_ref()])
                }
                TemporalFormula::Forall { bounds, formula }
                | TemporalFormula::Exists { bounds, formula } => {
                    for bound in bounds {
                        roots.extend(bound.domain.children());
                    }
                    formulas.push(formula);
                }
                TemporalFormula::Frame {
                    arguments,
                    captures,
                    formula,
                    ..
                } => {
                    roots.extend(arguments.iter().chain(captures));
                    formulas.push(formula);
                }
            }
        }
        Self::from_roots(roots)
    }

    pub fn from_roots(mut roots: Vec<&Expression>) -> Self {
        let mut prepared = Self {
            slots: HashMap::default(),
        };
        while let Some(expression) = roots.pop() {
            if let Expression::Data { .. } = expression {
                let address = std::ptr::from_ref(expression) as usize;
                let slot = prepared.slots.len();
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    prepared.slots.entry(address)
                {
                    entry.insert(slot);
                }
            }
            if let Expression::Enabled {
                branches,
                subscript,
            } = expression
            {
                register_branch_memberships(branches, &mut |expression| roots.push(expression));
                roots.extend(subscript.iter().map(Box::as_ref));
            }
            roots.extend(expression.children());
        }
        prepared
    }

    pub fn slot(&self, expression: &Expression) -> Option<usize> {
        self.slots
            .get(&(std::ptr::from_ref(expression) as usize))
            .copied()
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }
}
