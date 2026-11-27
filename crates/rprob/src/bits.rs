use crate::indexed_rows;
use rmodel::Mult;
use std::sync::Arc;

pub(crate) const ROW: usize = 8;
pub(crate) const COLUMN: u64 = 0x0101_0101_0101_0101;

pub(crate) fn indices(mut mask: u64) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        if mask == 0 {
            return None;
        }
        let index = mask.trailing_zeros() as usize;
        mask &= mask - 1;
        Some(index)
    })
}

pub(crate) fn product(left: u64, right: u64) -> u64 {
    indices(left).fold(0, |result, row| result | (right << (ROW * row)))
}

pub(crate) fn image(rows: u64, matrix: u64) -> u64 {
    indices(rows).fold(0, |result, row| result | ((matrix >> (ROW * row)) & 0xff))
}

pub(crate) fn preimage(matrix: u64, columns: u64) -> u64 {
    let selected = matrix & columns.wrapping_mul(COLUMN);
    (0..ROW).fold(0, |result, row| {
        result | (u64::from((selected >> (ROW * row)) & 0xff != 0) << row)
    })
}

pub(crate) fn override_rows(left: u64, right: u64) -> u64 {
    without_keys(left, preimage(right, 0xff)) | right
}

pub(crate) fn without_keys(matrix: u64, keys: u64) -> u64 {
    matrix & !product(keys, 0xff)
}

pub(crate) fn compose(left: u64, right: u64) -> u64 {
    (0..ROW).fold(0, |result, row| {
        let destinations = image((left >> (ROW * row)) & 0xff, right);
        result | (destinations << (ROW * row))
    })
}

pub(crate) fn admits(mult: Mult, value: u64) -> bool {
    match mult {
        Mult::Set => true,
        Mult::Some => value != 0,
        Mult::One => value.is_power_of_two(),
        Mult::Lone => value == 0 || value.is_power_of_two(),
    }
}

/// The same tuple ordinal as the reference enumerator: declared sort order,
/// and, for a relation, the target varying fastest. Physical bit order can
/// differ from that ordinal for sums.
pub(crate) struct Domain {
    pub(crate) mult: Mult,
    pub(crate) rows: Option<Rows>,
    pub(crate) row_width: usize,
    pub(crate) tuples: Vec<u64>,
    pub(crate) integer_mask: Option<u64>,
    pub(crate) ordinal_count: Option<usize>,
}

pub(crate) enum Rows {
    Dense(u64),
    Sparse(Vec<u64>),
    Indexed(Arc<indexed_rows::Layout>),
}

impl Domain {
    pub(crate) fn is_indexed(&self) -> bool {
        self.ordinal_count.is_some() || matches!(self.rows, Some(Rows::Indexed(_)))
    }

    pub(crate) fn admits(&self, value: u64) -> bool {
        if let Some(count) = self.ordinal_count {
            return value <= count as u64 && (value != 0 || self.mult == Mult::Lone);
        }
        if self.mult == Mult::Set {
            return true;
        }
        match &self.rows {
            None => admits(self.mult, value),
            Some(Rows::Dense(rows)) => {
                indices(*rows).all(|row| admits(self.mult, (value >> (ROW * row)) & 0xff))
            }
            Some(Rows::Sparse(rows)) => rows.iter().all(|row| admits(self.mult, value & row)),
            Some(Rows::Indexed(rows)) => rows.admits(value),
        }
    }

    pub(crate) fn candidates(&self) -> Candidates {
        if let Some(count) = self.ordinal_count {
            return Candidates::Ordinals {
                start: usize::from(self.mult == Mult::One),
                count,
            };
        }
        match &self.rows {
            None => match self.mult {
                Mult::One => Candidates::Table(self.tuples.clone()),
                Mult::Lone => Candidates::Table(
                    std::iter::once(0)
                        .chain(self.tuples.iter().copied())
                        .collect(),
                ),
                Mult::Set | Mult::Some => {
                    Candidates::subsets(&self.tuples, usize::from(self.mult == Mult::Some))
                }
            },
            Some(Rows::Indexed(rows)) => Candidates::IndexedRows(Arc::clone(rows)),
            Some(_) if self.row_width == 0 => {
                Candidates::Table(self.admits(0).then_some(0).into_iter().collect())
            }
            Some(_) => {
                let rows: Vec<Vec<u64>> = self
                    .tuples
                    .chunks_exact(self.row_width)
                    .map(|bits| match self.mult {
                        Mult::One => bits.to_vec(),
                        Mult::Lone => std::iter::once(0).chain(bits.iter().copied()).collect(),
                        Mult::Set | Mult::Some => {
                            let start = usize::from(self.mult == Mult::Some);
                            (start..1 << bits.len())
                                .map(|ordinal| subset(bits, ordinal))
                                .collect()
                        }
                    })
                    .collect();
                let count = rows.iter().map(Vec::len).product();
                Candidates::Rows { rows, count }.table_if_small()
            }
        }
    }
}

fn subset(bits: &[u64], ordinal: usize) -> u64 {
    indices(ordinal as u64).fold(0, |value, bit| value | bits[bit])
}

/// Indexable legal candidates, ordered as the reference's filtered mask
/// enumeration. In a binary relation the first declared row advances
/// fastest. Each row owns only its own choices.
pub(crate) enum Candidates {
    Table(Vec<u64>),
    IndexedRows(Arc<indexed_rows::Layout>),
    Ordinals {
        start: usize,
        count: usize,
    },
    Subsets {
        bits: Vec<u64>,
        start: usize,
    },
    Packed {
        shift: u32,
        width: usize,
        start: usize,
    },
    Rows {
        rows: Vec<Vec<u64>>,
        count: usize,
    },
}

#[derive(Clone, Default)]
pub(crate) struct Cursor {
    ordinal: usize,
    rows: [u8; ROW],
}

impl Candidates {
    fn subsets(bits: &[u64], start: usize) -> Self {
        if let Some(first) = bits.first()
            && bits
                .windows(2)
                .all(|pair| pair[0].checked_mul(2) == Some(pair[1]))
        {
            Self::Packed {
                shift: first.trailing_zeros(),
                width: bits.len(),
                start,
            }
        } else {
            Self::Subsets {
                bits: bits.to_vec(),
                start,
            }
        }
        .table_if_small()
    }

    /// Advance one slot, wrapping to its first value on carry. For a
    /// relation, only the rows affected by the carry need updating.
    pub(crate) fn advance(&self, cursor: &mut Cursor, value: &mut u64) -> bool {
        match self {
            Self::Rows { rows, .. } if rows.len() <= ROW => {
                for (choices, digit) in rows.iter().zip(&mut cursor.rows) {
                    let old = usize::from(*digit);
                    let next = old + 1;
                    *value ^= choices[old];
                    if next < choices.len() {
                        *digit = next as u8;
                        *value |= choices[next];
                        return true;
                    }
                    *digit = 0;
                    *value |= choices[0];
                }
                false
            }
            _ => {
                let next = cursor.ordinal + 1;
                let found = self.get(next);
                cursor.ordinal = if found.is_some() { next } else { 0 };
                *value = found.unwrap_or_else(|| self.get(0).expect("nonempty choices"));
                found.is_some()
            }
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Table(values) => values.len(),
            Self::IndexedRows(layout) => layout.candidate_count(),
            Self::Ordinals { start, count } => count + 1 - start,
            Self::Subsets { bits, start } => (1 << bits.len()) - start,
            Self::Packed { width, start, .. } => (1 << width) - start,
            Self::Rows { count, .. } => *count,
        }
    }

    pub(crate) fn get(&self, position: usize) -> Option<u64> {
        if position >= self.len() {
            return None;
        }
        Some(match self {
            Self::Table(values) => values[position],
            Self::IndexedRows(layout) => layout.candidate(position),
            Self::Ordinals { start, .. } => (position + start) as u64,
            Self::Subsets { bits, start } => subset(bits, position + start),
            Self::Packed { shift, start, .. } => ((position + start) as u64) << shift,
            Self::Rows { rows, .. } => {
                let mut remaining = position;
                let mut value = 0;
                for choices in rows {
                    value |= choices[remaining % choices.len()];
                    remaining /= choices.len();
                }
                value
            }
        })
    }

    fn table_if_small(self) -> Self {
        if self.len() <= 256 {
            Self::Table(
                (0..self.len())
                    .map(|index| self.get(index).expect("valid position"))
                    .collect(),
            )
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subset_cursor_keeps_the_first_and_last_values_in_declared_order() {
        let ascending: Vec<_> = (55..64).map(|offset| 1 << offset).collect();
        let reversed: Vec<_> = ascending.iter().rev().copied().collect();
        for bits in [ascending, reversed] {
            for start in [0, 1] {
                let choices = Candidates::subsets(&bits, start);
                let mut cursor = Cursor::default();
                let mut value = choices.get(0).unwrap();
                for ordinal in start..1 << bits.len() {
                    assert_eq!(value, subset(&bits, ordinal));
                    assert_eq!(
                        choices.advance(&mut cursor, &mut value),
                        ordinal + 1 < 1 << bits.len()
                    );
                }
                assert_eq!(value, subset(&bits, start));
            }
        }
    }

    #[test]
    fn rows_follow_declared_tuple_order_for_each_multiplicity() {
        // Four by four is large enough to exercise both tables and lazy rows.
        // The declared sort order need not match the physical bit positions.
        let order = [3, 1, 0, 2];
        let tuples: Vec<u64> = order
            .iter()
            .flat_map(|row| order.iter().map(move |column| 1 << (ROW * row + column)))
            .collect();
        for mult in [Mult::Set, Mult::Some, Mult::One, Mult::Lone] {
            let domain = Domain {
                mult,
                rows: Some(Rows::Dense(0b1111)),
                row_width: order.len(),
                tuples: tuples.clone(),
                integer_mask: None,
                ordinal_count: None,
            };
            let choices = domain.candidates();
            let expected = (0..1 << tuples.len()).filter_map(|ordinal| {
                let relation = subset(&tuples, ordinal);
                domain.admits(relation).then_some(relation)
            });
            assert!(
                (0..choices.len())
                    .map(|position| choices.get(position).unwrap())
                    .eq(expected),
                "{mult:?}"
            );
            assert_eq!(choices.get(choices.len()), None);

            let first = choices.get(0).unwrap();
            let mut value = first;
            let mut cursor = Cursor::default();
            for position in 1..choices.len() {
                assert!(choices.advance(&mut cursor, &mut value));
                assert_eq!(Some(value), choices.get(position));
            }
            assert!(!choices.advance(&mut cursor, &mut value));
            assert_eq!(value, first);
        }
    }
}
