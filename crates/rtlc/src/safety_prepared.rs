use crate::ir::{Branch, Definition, Expression, FoldOrder, SafetyProgram, TemporalFormula};
use crate::safety_expressions::register_branch_memberships;
use crate::safety_expressions::{BindRun, plan_bind_runs};
use crate::safety_fold_order::{OrderIndependence, classify};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::OnceLock;

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
    bind_runs: HashMap<usize, Vec<Option<BindRun>>, BuildHasherDefault<AddressHasher>>,
    fold_orders: HashMap<usize, OnceLock<OrderIndependence>, BuildHasherDefault<AddressHasher>>,
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
        let mut branches: Vec<&[Branch]> = vec![&program.initial, &program.next];
        for definition in program.definitions.iter().flatten() {
            if let crate::ir::DefinitionBody::Action(action) = &definition.body {
                branches.push(action);
            }
        }
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
        Self::from_parts(roots, branches)
    }

    #[cfg(test)]
    pub fn from_roots(roots: Vec<&Expression>) -> Self {
        Self::from_parts(roots, vec![])
    }

    fn from_parts(mut roots: Vec<&Expression>, mut branches: Vec<&[Branch]>) -> Self {
        let mut prepared = Self {
            slots: HashMap::default(),
            bind_runs: HashMap::default(),
            fold_orders: HashMap::default(),
        };
        while let Some(expression) = roots.pop() {
            if let Expression::Fold {
                order: FoldOrder::SetMembers,
                ..
            } = expression
            {
                prepared
                    .fold_orders
                    .entry(std::ptr::from_ref(expression) as usize)
                    .or_default();
            }
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
                prepared.register_bind_runs(branches);
            }
            roots.extend(expression.children());
        }
        while let Some(group) = branches.pop() {
            prepared.register_bind_runs(group);
        }
        prepared
    }

    fn register_bind_runs(&mut self, branches: &[Branch]) {
        plan_bind_runs(branches, &mut |branch, position, run| {
            let runs = self
                .bind_runs
                .entry(std::ptr::from_ref(branch) as usize)
                .or_insert_with(|| (0..branch.steps.len()).map(|_| None).collect());
            runs[position] = Some(run);
        });
    }

    pub fn bind_run(&self, branch: &Branch, position: usize) -> Option<&BindRun> {
        self.bind_runs
            .get(&(std::ptr::from_ref(branch) as usize))?
            .get(position)?
            .as_ref()
    }

    pub fn fold_independence(
        &self,
        expression: &Expression,
        definitions: &[Definition],
    ) -> OrderIndependence {
        let Expression::Fold {
            accumulator, body, ..
        } = expression
        else {
            return OrderIndependence::Dependent;
        };
        match self
            .fold_orders
            .get(&(std::ptr::from_ref(expression) as usize))
        {
            Some(class) => *class.get_or_init(|| classify(*accumulator, body, definitions)),
            None => classify(*accumulator, body, definitions),
        }
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

#[cfg(test)]
mod tests {
    use super::PreparedProgram;
    use crate::ir::{Expression, SafetyProgram, Step};
    use crate::safety_fold_order::OrderIndependence;
    use serde_json::json;

    fn fold_program() -> SafetyProgram {
        let literal = |value: &str| json!({"op":"literal","value":{"type":"int","value":value}});
        let fold = json!({"op":"fold","accumulator":0,"initial":{"op":"set","values":[]},"collection":{"op":"set","values":[literal("1")]},"order":"set-members","body":{"op":"cup","left":{"op":"bound","slot":0},"right":{"op":"set","values":[{"op":"bound","slot":1}]}}});
        serde_json::from_value(json!({
            "format":"modelt.safety-program","version":1,"name":"Fold","variables":["x"],
            "initial":[{"name":"Init","steps":[{"kind":"assign","slot":0,"value":literal("0")}]}],
            "next":[{"name":"Next","steps":[{"kind":"assign","slot":0,"value":literal("0")}]}],
            "invariants":[{"name":"Safe","expression":{"op":"eq","left":fold,"right":{"op":"set","values":[literal("1")]}}}],
            "assumptions":[],"checkDeadlock":false
        }))
        .unwrap()
    }

    #[test]
    fn fold_order_classes_are_computed_on_first_use_and_only_once() {
        let program = fold_program();
        let prepared = PreparedProgram::new(&program);
        let fold = match &program.invariants[0].expression {
            Expression::Eq { left, .. } => left.as_ref(),
            _ => unreachable!(),
        };
        let cell = prepared
            .fold_orders
            .get(&(std::ptr::from_ref(fold) as usize))
            .unwrap();
        assert!(cell.get().is_none());
        assert_eq!(
            prepared.fold_independence(fold, &[]),
            OrderIndependence::Union
        );
        assert_eq!(cell.get(), Some(&OrderIndependence::Union));
        let copy = fold.clone();
        assert_eq!(
            prepared.fold_independence(&copy, &[]),
            OrderIndependence::Union
        );
        assert_eq!(prepared.fold_orders.len(), 1);
    }

    #[test]
    fn bind_plans_are_found_only_for_the_program_own_branches() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../tests/dead-bindings-fixtures.json")).unwrap();
        let case = fixtures["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["spec"] == "dead" && case["variant"] == "complete")
            .unwrap();
        let program: SafetyProgram = serde_json::from_value(case["program"].clone()).unwrap();
        let prepared = PreparedProgram::new(&program);
        let branch = &program.next[0];
        let position = branch
            .steps
            .iter()
            .position(|step| matches!(step, Step::Bind { .. }))
            .unwrap();
        assert!(prepared.bind_run(branch, position).is_some());
        let copy = branch.clone();
        assert!(prepared.bind_run(&copy, position).is_none());
    }
}
