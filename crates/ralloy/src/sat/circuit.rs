//! And-inverter graph with structural hashing and constant folding, and its
//! Tseitin encoding (plan 0085).
use super::cdcl::{Lit, Solver, negate};
use std::collections::HashMap;

/// A circuit literal: `node * 2 + negated`; node 0 is the constant false.
pub(crate) type Gate = u32;
pub(crate) const FALSE: Gate = 0;
pub(crate) const TRUE: Gate = 1;

#[derive(Default)]
pub(crate) struct Circuit {
    /// Per node after the constant: `None` for an input, or its two inputs.
    nodes: Vec<Option<(Gate, Gate)>>,
    table: HashMap<(Gate, Gate), Gate>,
    /// Per node: its position among the inputs, if it is one.
    positions: Vec<Option<usize>>,
    inputs: usize,
}

impl Circuit {
    pub(crate) fn new() -> Self {
        Self {
            nodes: vec![None],
            table: HashMap::new(),
            positions: vec![None],
            inputs: 0,
        }
    }
    /// The position of an input gate among the inputs, in creation order.
    pub(crate) fn input_index(&self, gate: Gate) -> Option<usize> {
        self.positions[(gate >> 1) as usize]
    }
    pub(crate) fn input(&mut self) -> Gate {
        self.positions.push(Some(self.inputs));
        self.inputs += 1;
        self.nodes.push(None);
        ((self.nodes.len() - 1) as Gate) << 1
    }
    pub(crate) fn and(&mut self, left: Gate, right: Gate) -> Gate {
        let (left, right) = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        if left == FALSE || left == negate(right) {
            return FALSE;
        }
        if left == TRUE || left == right {
            return right;
        }
        if let Some(gate) = self.table.get(&(left, right)) {
            return *gate;
        }
        self.nodes.push(Some((left, right)));
        self.positions.push(None);
        let gate = ((self.nodes.len() - 1) as Gate) << 1;
        self.table.insert((left, right), gate);
        gate
    }
    pub(crate) fn or(&mut self, left: Gate, right: Gate) -> Gate {
        let both = self.and(left ^ 1, right ^ 1);
        both ^ 1
    }
    pub(crate) fn implies(&mut self, left: Gate, right: Gate) -> Gate {
        self.or(left ^ 1, right)
    }
    pub(crate) fn iff(&mut self, left: Gate, right: Gate) -> Gate {
        let forward = self.implies(left, right);
        let backward = self.implies(right, left);
        self.and(forward, backward)
    }
    pub(crate) fn all(&mut self, gates: impl IntoIterator<Item = Gate>) -> Gate {
        gates
            .into_iter()
            .fold(TRUE, |all, gate| self.and(all, gate))
    }
    pub(crate) fn any(&mut self, gates: impl IntoIterator<Item = Gate>) -> Gate {
        gates
            .into_iter()
            .fold(FALSE, |any, gate| self.or(any, gate))
    }
    /// At most one of `gates` holds (pairwise).
    pub(crate) fn at_most_one(&mut self, gates: &[Gate]) -> Gate {
        let mut result = TRUE;
        for (index, &left) in gates.iter().enumerate() {
            for &right in &gates[index + 1..] {
                let both = self.and(left, right);
                result = self.and(result, both ^ 1);
            }
        }
        result
    }
    pub(crate) fn exactly_one(&mut self, gates: &[Gate]) -> Gate {
        let some = self.any(gates.iter().copied());
        let lone = self.at_most_one(gates);
        self.and(some, lone)
    }

    /// Tseitin encoding of `root` asserted true. Returns the solver and, per
    /// circuit input in creation order, its solver variable.
    pub(crate) fn encode(&self, root: Gate) -> (Solver, Vec<Option<usize>>) {
        let mut variables: Vec<Option<usize>> = vec![None; self.nodes.len()];
        let mut order = Vec::new();
        let mut stack = vec![(root >> 1) as usize];
        while let Some(node) = stack.pop() {
            if node == 0 || variables[node].is_some() {
                continue;
            }
            variables[node] = Some(order.len());
            order.push(node);
            if let Some((left, right)) = self.nodes[node] {
                stack.push((left >> 1) as usize);
                stack.push((right >> 1) as usize);
            }
        }
        let literal = |gate: Gate| -> Option<Lit> {
            let node = (gate >> 1) as usize;
            variables[node].map(|variable| (variable as Lit) << 1 | (gate & 1))
        };
        let mut solver = Solver::new(order.len());
        match root {
            FALSE => solver.add(&[]),
            TRUE => {}
            _ => solver.add(&[literal(root).expect("encoded root")]),
        }
        for &node in &order {
            if let Some((left, right)) = self.nodes[node] {
                let output = (variables[node].expect("encoded node") as Lit) << 1;
                // Inputs of a folded gate are never constants.
                let (left, right) = (
                    literal(left).expect("encoded input"),
                    literal(right).expect("encoded input"),
                );
                solver.add(&[negate(output), left]);
                solver.add(&[negate(output), right]);
                solver.add(&[output, negate(left), negate(right)]);
            }
        }
        let inputs = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(node, gates)| *node > 0 && gates.is_none())
            .map(|(node, _)| variables[node])
            .collect();
        (solver, inputs)
    }
}

#[cfg(test)]
mod tests {
    use super::{Circuit, FALSE, Gate, TRUE};
    use crate::{Stats, eval::Budget};

    fn evaluate(circuit: &Circuit, gate: Gate, inputs: &[bool]) -> bool {
        let node = (gate >> 1) as usize;
        let value = if node == 0 {
            false
        } else {
            match circuit.nodes[node] {
                None => {
                    let position = circuit.nodes[..node]
                        .iter()
                        .skip(1)
                        .filter(|n| n.is_none())
                        .count();
                    inputs[position]
                }
                Some((left, right)) => {
                    evaluate(circuit, left, inputs) && evaluate(circuit, right, inputs)
                }
            }
        };
        value != (gate & 1 == 1)
    }

    /// Random circuits: the encoding is satisfiable exactly when some input
    /// assignment makes the root true, and its model does.
    #[test]
    fn encodings_agree_with_evaluating_every_input_assignment() {
        let mut state = 0x0c1c_u64;
        let mut next = |bound: u64| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) % bound
        };
        for _ in 0..1000 {
            let mut circuit = Circuit::new();
            let count = 1 + next(6) as usize;
            let mut gates: Vec<Gate> = (0..count).map(|_| circuit.input()).collect();
            gates.extend([FALSE, TRUE]);
            for _ in 0..next(12) {
                let left = gates[next(gates.len() as u64) as usize] ^ next(2) as Gate;
                let right = gates[next(gates.len() as u64) as usize] ^ next(2) as Gate;
                let gate = match next(4) {
                    0 => circuit.and(left, right),
                    1 => circuit.or(left, right),
                    2 => circuit.iff(left, right),
                    _ => circuit.exactly_one(&[left, right, gates[0]]),
                };
                gates.push(gate);
            }
            let root = *gates.last().unwrap() ^ next(2) as Gate;
            let exhaustive = (0u32..1 << count).any(|bits| {
                let inputs: Vec<bool> = (0..count).map(|i| bits >> i & 1 == 1).collect();
                evaluate(&circuit, root, &inputs)
            });
            let (mut solver, variables) = circuit.encode(root);
            let mut budget = Budget {
                stats: Stats::default(),
                limit: 1_000_000,
            };
            match solver.solve(&mut budget).unwrap() {
                Some(model) => {
                    assert!(exhaustive);
                    let inputs: Vec<bool> = variables
                        .iter()
                        .map(|variable| variable.is_some_and(|variable| model[variable]))
                        .collect();
                    assert!(evaluate(&circuit, root, &inputs));
                }
                None => assert!(!exhaustive),
            }
        }
    }
}
