use crate::{Unsupported, bits};
use std::collections::BTreeMap;

/// Unary values use the ordinary atom masks. A binary bit identifies one
/// ordered pair everywhere in the model, independent of its declared shape.
#[derive(Default)]
pub(crate) struct Tuples {
    ids: BTreeMap<(u64, u64), u64>,
    pairs: Vec<(u64, u64)>,
}

pub(crate) enum Operation {
    Collect(Vec<(u64, u64, u64)>),
    Override(Vec<(u64, u64)>),
    WithoutKeys(Vec<(u64, u64)>),
}

impl Operation {
    pub(crate) fn rule_count(&self) -> usize {
        match self {
            Self::Collect(rules) => rules.len(),
            Self::Override(rows) | Self::WithoutKeys(rows) => rows.len(),
        }
    }

    pub(crate) fn evaluate(&self, left: u64, right: u64) -> u64 {
        match self {
            Self::Collect(rules) => rules.iter().fold(0, |value, &(a, b, out)| {
                if left & a != 0 && right & b != 0 {
                    value | out
                } else {
                    value
                }
            }),
            Self::Override(rows) => rows.iter().fold(left | right, |value, &(old, new)| {
                if right & new != 0 {
                    (value & !old) | (right & new)
                } else {
                    value
                }
            }),
            Self::WithoutKeys(rows) => rows.iter().fold(left, |value, &(key, mask)| {
                if right & key != 0 {
                    value & !mask
                } else {
                    value
                }
            }),
        }
    }
}

impl Tuples {
    pub(crate) fn pair(&self, index: usize) -> (u64, u64) {
        self.pairs[index]
    }

    pub(crate) fn insert(&mut self, from: u64, to: u64) -> Result<u64, Unsupported> {
        if let Some(&bit) = self.ids.get(&(from, to)) {
            return Ok(bit);
        }
        let bit = 1_u64.checked_shl(self.pairs.len() as u32).ok_or_else(|| {
            Unsupported::new("binary expressions require more than 64 distinct ordered pairs")
        })?;
        self.pairs.push((from, to));
        self.ids.insert((from, to), bit);
        Ok(bit)
    }

    fn entries(&self, mask: u64) -> impl Iterator<Item = (u64, u64, u64)> + '_ {
        bits::indices(mask).map(|index| {
            let (from, to) = self.pairs[index];
            (1 << index, from, to)
        })
    }

    pub(crate) fn product(&mut self, left: u64, right: u64) -> Result<Operation, Unsupported> {
        let mut rules = Vec::new();
        for from in bits::indices(left).map(|index| 1 << index) {
            for to in bits::indices(right).map(|index| 1 << index) {
                rules.push((from, to, self.insert(from, to)?));
            }
        }
        Ok(Operation::Collect(rules))
    }

    pub(crate) fn image(&self, left: u64, right: u64) -> Operation {
        Operation::Collect(
            self.entries(right)
                .filter_map(|(bit, from, to)| (left & from != 0).then_some((from, bit, to)))
                .collect(),
        )
    }

    pub(crate) fn project(
        &mut self,
        support: u64,
        fields: &[(u64, u64)],
    ) -> Result<Vec<(u64, u64)>, Unsupported> {
        let mut mapping = Vec::new();
        for index in bits::indices(support) {
            let (key, record) = self.pair(index);
            if let Some(&(_, value)) = fields.iter().find(|&&(bit, _)| bit == record) {
                mapping.push((1 << index, self.insert(key, value)?));
            }
        }
        Ok(mapping)
    }

    pub(crate) fn preimage(&self, left: u64, right: u64) -> Operation {
        Operation::Collect(
            self.entries(left)
                .filter_map(|(bit, from, to)| (right & to != 0).then_some((bit, to, from)))
                .collect(),
        )
    }

    pub(crate) fn compose(&mut self, left: u64, right: u64) -> Result<Operation, Unsupported> {
        let sources: Vec<_> = self.entries(left).collect();
        let destinations: Vec<_> = self.entries(right).collect();
        let mut rules = Vec::new();
        for (a, from, middle) in sources {
            for &(b, next, to) in &destinations {
                if middle == next {
                    rules.push((a, b, self.insert(from, to)?));
                }
            }
        }
        Ok(Operation::Collect(rules))
    }

    pub(crate) fn override_rows(&self, left: u64, right: u64) -> Operation {
        let mut rows = BTreeMap::<u64, (u64, u64)>::new();
        for (bit, from, _) in self.entries(left) {
            rows.entry(from).or_default().0 |= bit;
        }
        for (bit, from, _) in self.entries(right) {
            rows.entry(from).or_default().1 |= bit;
        }
        Operation::Override(
            rows.into_values()
                .filter(|&(a, b)| a != 0 && b != 0)
                .collect(),
        )
    }

    pub(crate) fn without_keys(&self, relation: u64, keys: u64) -> Operation {
        let mut rows = BTreeMap::<u64, u64>::new();
        for (bit, from, _) in self.entries(relation) {
            if keys & from != 0 {
                *rows.entry(from).or_default() |= bit;
            }
        }
        Operation::WithoutKeys(rows.into_iter().collect())
    }
}
