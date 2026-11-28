//! Packed functions from atom keys to ordinal values.
use idealis_syntax::Mult;
use std::sync::Arc;

pub(crate) struct Layout {
    /// Least significant row is the first declared key.
    rows: Vec<(u64, u64)>,
    radix: u64,
    limit: u64,
    count: usize,
    required: bool,
    allowed: Option<Arc<[u64]>>,
}

impl Layout {
    pub(crate) fn with_refinement(
        keys: &[u64],
        values: usize,
        mult: Mult,
        allowed: Option<Arc<[u64]>>,
    ) -> Option<Self> {
        let radix = u64::try_from(values).ok()?.checked_add(1)?;
        let limit = radix.checked_pow(keys.len() as u32)?;
        let required = mult == Mult::One;
        let choices = allowed.as_ref().map_or(values, |codes| codes.len());
        let count = (choices + usize::from(!required)).checked_pow(keys.len() as u32)?;
        let rows = keys
            .iter()
            .copied()
            .enumerate()
            .map(|(index, key)| (key, radix.pow(index as u32)))
            .collect();
        Some(Self {
            rows,
            radix,
            limit,
            count,
            required,
            allowed,
        })
    }

    pub(crate) fn key_mask(&self) -> u64 {
        self.rows.iter().fold(0, |mask, (key, _)| mask | key)
    }

    fn digit(&self, word: u64, weight: u64) -> u64 {
        (word / weight) % self.radix
    }

    pub(crate) fn values(&self, word: u64) -> impl Iterator<Item = (u64, u64)> + '_ {
        self.rows.iter().filter_map(move |&(key, weight)| {
            let value = self.digit(word, weight);
            (value != 0).then_some((key, value))
        })
    }

    pub(crate) fn keys(&self, word: u64) -> u64 {
        self.values(word).fold(0, |mask, (key, _)| mask | key)
    }

    pub(crate) fn row(&self, word: u64, key: u64) -> u64 {
        self.rows
            .iter()
            .find(|(own, _)| *own == key)
            .map_or(0, |(_, weight)| self.digit(word, *weight))
    }

    pub(crate) fn admits(&self, word: u64) -> bool {
        if word >= self.limit {
            return false;
        }
        let Some(allowed) = &self.allowed else {
            return !self.required || self.values(word).count() == self.rows.len();
        };
        self.rows.iter().all(|(_, weight)| {
            let digit = self.digit(word, *weight);
            if digit == 0 {
                !self.required
            } else {
                allowed.binary_search(&digit).is_ok()
            }
        })
    }

    pub(crate) fn candidate_count(&self) -> usize {
        self.count
    }

    pub(crate) fn candidate(&self, ordinal: usize) -> u64 {
        if let Some(codes) = &self.allowed {
            let base = codes.len() + usize::from(!self.required);
            let mut remaining = ordinal;
            return self.rows.iter().fold(0, |code, (_, weight)| {
                let digit = (remaining % base + usize::from(self.required))
                    .checked_sub(1)
                    .map_or(0, |index| codes[index]);
                remaining /= base;
                code + digit * weight
            });
        }
        if !self.required {
            return ordinal as u64;
        }
        let base = self.radix - 1;
        let mut remaining = ordinal as u64;
        self.rows.iter().fold(0, |code, (_, weight)| {
            let digit = remaining % base + 1;
            remaining /= base;
            code + digit * weight
        })
    }

    pub(crate) fn contains(&self, left: u64, right: u64) -> bool {
        self.rows.iter().all(|(_, weight)| {
            let value = self.digit(left, *weight);
            value == 0 || value == self.digit(right, *weight)
        })
    }

    pub(crate) fn evaluate(&self, op: Operation, left: u64, right: u64) -> u64 {
        match op {
            Operation::Product => self
                .rows
                .iter()
                .filter(|(key, _)| left & key != 0)
                .map(|(_, weight)| right * weight)
                .sum(),
            Operation::WithoutKeys => self
                .rows
                .iter()
                .filter(|(key, _)| right & key == 0)
                .map(|(_, weight)| self.digit(left, *weight) * weight)
                .sum(),
            Operation::Override => self
                .rows
                .iter()
                .map(|(_, weight)| {
                    let update = self.digit(right, *weight);
                    let value = if update == 0 {
                        self.digit(left, *weight)
                    } else {
                        update
                    };
                    value * weight
                })
                .sum(),
            Operation::Difference | Operation::Intersection => self
                .rows
                .iter()
                .filter(|(_, weight)| {
                    let equal = self.digit(left, *weight) == self.digit(right, *weight);
                    equal == matches!(op, Operation::Intersection)
                })
                .map(|(_, weight)| self.digit(left, *weight) * weight)
                .sum(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Operation {
    Product,
    WithoutKeys,
    Override,
    Difference,
    Intersection,
}
