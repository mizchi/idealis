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
    pub(crate) fn select(&mut self, test: Gate, yes: Gate, no: Gate) -> Gate {
        if yes == no {
            return yes;
        }
        let yes = self.and(test, yes);
        let no = self.and(test ^ 1, no);
        self.or(yes, no)
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
    /// A two's complement constant of `width` bits, least significant first.
    pub(crate) fn constant(value: i64, width: usize) -> Vec<Gate> {
        (0..width)
            .map(|bit| if (value >> bit) & 1 == 1 { TRUE } else { FALSE })
            .collect()
    }
    /// Sum modulo 2^width; the carry out of the top bit is dropped.
    pub(crate) fn add(&mut self, left: &[Gate], right: &[Gate]) -> Vec<Gate> {
        let mut carry = FALSE;
        let mut sum = Vec::with_capacity(left.len());
        for (&a, &b) in left.iter().zip(right) {
            let half = self.xor(a, b);
            sum.push(self.xor(half, carry));
            let both = self.and(a, b);
            let propagated = self.and(half, carry);
            carry = self.or(both, propagated);
        }
        sum
    }
    pub(crate) fn xor(&mut self, left: Gate, right: Gate) -> Gate {
        self.iff(left, right) ^ 1
    }
    pub(crate) fn negative(&mut self, value: &[Gate]) -> Vec<Gate> {
        let inverted: Vec<Gate> = value.iter().map(|gate| gate ^ 1).collect();
        let one = Self::constant(1, value.len());
        self.add(&inverted, &one)
    }
    pub(crate) fn subtract(&mut self, left: &[Gate], right: &[Gate]) -> Vec<Gate> {
        let negative = self.negative(right);
        self.add(left, &negative)
    }
    /// Product modulo 2^width by shifted additions.
    pub(crate) fn multiply(&mut self, left: &[Gate], right: &[Gate]) -> Vec<Gate> {
        let width = left.len();
        let mut product = Self::constant(0, width);
        for (shift, &bit) in right.iter().enumerate() {
            let partial: Vec<Gate> = (0..width)
                .map(|index| {
                    if index < shift {
                        FALSE
                    } else {
                        self.and(left[index - shift], bit)
                    }
                })
                .collect();
            product = self.add(&product, &partial);
        }
        product
    }
    pub(crate) fn equal(&mut self, left: &[Gate], right: &[Gate]) -> Gate {
        let mut all = TRUE;
        for (&a, &b) in left.iter().zip(right) {
            let same = self.iff(a, b);
            all = self.and(all, same);
        }
        all
    }
    /// Unsigned `left < right`.
    pub(crate) fn below(&mut self, left: &[Gate], right: &[Gate]) -> Gate {
        let mut less = FALSE;
        for (&a, &b) in left.iter().zip(right) {
            let smaller = self.and(a ^ 1, b);
            let same = self.iff(a, b);
            let kept = self.and(same, less);
            less = self.or(smaller, kept);
        }
        less
    }
    /// Signed `left < right`: unsigned after flipping the sign bits.
    pub(crate) fn less(&mut self, left: &[Gate], right: &[Gate]) -> Gate {
        let flip = |value: &[Gate]| {
            let mut value = value.to_vec();
            if let Some(sign) = value.last_mut() {
                *sign ^= 1;
            }
            value
        };
        let (left, right) = (flip(left), flip(right));
        self.below(&left, &right)
    }
    /// How many of `gates` hold, modulo 2^width.
    pub(crate) fn count(&mut self, gates: &[Gate], width: usize) -> Vec<Gate> {
        let mut total = Self::constant(0, width);
        for &gate in gates {
            let mut one = Self::constant(0, width);
            if let Some(low) = one.first_mut() {
                *low = gate;
            }
            total = self.add(&total, &one);
        }
        total
    }
    /// At most `bound` of `gates` hold, without wrapping.
    pub(crate) fn at_most(&mut self, gates: &[Gate], bound: usize) -> Gate {
        if bound >= gates.len() {
            return TRUE;
        }
        let width = usize::BITS as usize - gates.len().leading_zeros() as usize + 1;
        let total = self.count(gates, width);
        let limit = Self::constant(bound as i64 + 1, width);
        self.below(&total, &limit)
    }
    /// Exactly `target` of `gates` hold.
    pub(crate) fn exactly(&mut self, gates: &[Gate], target: usize) -> Gate {
        if target > gates.len() {
            return FALSE;
        }
        let width = usize::BITS as usize - gates.len().leading_zeros() as usize + 1;
        let total = self.count(gates, width);
        let wanted = Self::constant(target as i64, width);
        self.equal(&total, &wanted)
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

    /// Bit-vector operations on constants fold to the enumerator's wrapped
    /// arithmetic at every width 1-4 and every input pair.
    #[test]
    fn bit_vectors_wrap_like_the_enumerator() {
        let wrap = |value: i64, width: usize| {
            let modulus = 1i64 << width;
            (value + modulus / 2).rem_euclid(modulus) - modulus / 2
        };
        let read = |bits: &[Gate]| -> i64 {
            let mut value = 0i64;
            for (index, bit) in bits.iter().enumerate() {
                assert!(*bit == TRUE || *bit == FALSE, "constant folding");
                if *bit == TRUE {
                    value |= 1 << index;
                }
            }
            let width = bits.len();
            if width > 0 && value >> (width - 1) & 1 == 1 {
                value -= 1 << width;
            }
            value
        };
        for width in 1..=4 {
            let range = -(1i64 << (width - 1))..(1i64 << (width - 1));
            for a in range.clone() {
                for b in range.clone() {
                    let mut circuit = Circuit::new();
                    let (x, y) = (Circuit::constant(a, width), Circuit::constant(b, width));
                    assert_eq!(read(&circuit.add(&x, &y)), wrap(a + b, width));
                    assert_eq!(read(&circuit.subtract(&x, &y)), wrap(a - b, width));
                    assert_eq!(read(&circuit.multiply(&x, &y)), wrap(a * b, width));
                    assert_eq!(read(&circuit.negative(&x)), wrap(-a, width));
                    assert_eq!(circuit.less(&x, &y) == TRUE, a < b, "{a} {b} {width}");
                    assert_eq!(circuit.equal(&x, &y) == TRUE, a == b);
                }
            }
            for ones in 0..=6usize {
                let mut circuit = Circuit::new();
                let gates = vec![TRUE; ones];
                assert_eq!(
                    read(&circuit.count(&gates, width)),
                    wrap(ones as i64, width)
                );
                for bound in 0..=6 {
                    assert_eq!(circuit.at_most(&gates, bound) == TRUE, ones <= bound);
                    assert_eq!(circuit.exactly(&gates, bound) == TRUE, ones == bound);
                }
            }
        }
    }
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
