//! Ordinals for scalar structured values whose full domain cannot fit in bits.
//! Zero denotes absence. Every other code is one plus its domain ordinal.
use idealis_semantics::reference::Data;
use std::sync::Arc;

pub(crate) enum Field {
    Bits(Vec<u64>),
    Indexed(Arc<Layout>),
}

impl Field {
    pub(crate) fn size(&self) -> usize {
        match self {
            Self::Bits(bits) => bits.len(),
            Self::Indexed(layout) => layout.count,
        }
    }

    fn ordinal(&self, code: u64) -> Option<usize> {
        match self {
            Self::Bits(bits) => bits.iter().position(|&bit| bit == code),
            Self::Indexed(layout) => {
                let ordinal = usize::try_from(code.checked_sub(1)?).ok()?;
                (ordinal < layout.count).then_some(ordinal)
            }
        }
    }

    fn code(&self, ordinal: usize) -> u64 {
        match self {
            Self::Bits(bits) => bits[ordinal],
            Self::Indexed(_) => ordinal as u64 + 1,
        }
    }

    pub(crate) fn bit_mask(&self) -> Option<u64> {
        match self {
            Self::Bits(bits) => Some(bits.iter().fold(0, |mask, bit| mask | bit)),
            Self::Indexed(_) => None,
        }
    }

    fn decode(&self, ordinal: usize, atoms: &[Data]) -> Data {
        match self {
            Self::Bits(bits) => atoms[bits[ordinal].trailing_zeros() as usize].clone(),
            Self::Indexed(layout) => layout.decode(ordinal, atoms),
        }
    }
}

pub(crate) struct Layout {
    pub(crate) count: usize,
    kind: Kind,
}

enum Kind {
    Sequence {
        element: Field,
        /// Offset of each length, plus the total as a sentinel.
        offsets: Vec<usize>,
    },
    Record {
        name: String,
        fields: Vec<(String, Field)>,
    },
}

pub(crate) struct Projection {
    layout: Arc<Layout>,
    field: usize,
    stride: usize,
}

impl Projection {
    pub(crate) fn evaluate(&self, code: u64) -> u64 {
        let Some(rank) = code.checked_sub(1) else {
            return 0;
        };
        let Kind::Record { fields, .. } = &self.layout.kind else {
            unreachable!("projection of a record")
        };
        let domain = &fields[self.field].1;
        domain.code((rank as usize / self.stride) % domain.size())
    }
}

impl Layout {
    pub(crate) fn project(self: &Arc<Self>, name: &str) -> Option<(Projection, Option<u64>)> {
        let Kind::Record { fields, .. } = &self.kind else {
            return None;
        };
        let field = fields.iter().position(|(key, _)| key == name)?;
        // Empty record domains have no nonzero code, so their stride is unused.
        let stride = if self.count == 0 {
            1
        } else {
            fields[field + 1..]
                .iter()
                .map(|(_, domain)| domain.size())
                .product()
        };
        Some((
            Projection {
                layout: Arc::clone(self),
                field,
                stride,
            },
            fields[field].1.bit_mask(),
        ))
    }

    pub(crate) fn record_field(&self, code: u64, index: usize) -> u64 {
        let Kind::Record { fields, .. } = &self.kind else {
            unreachable!("a record layout")
        };
        let stride: usize = fields[index + 1..]
            .iter()
            .map(|(_, field)| field.size())
            .product();
        let domain = &fields[index].1;
        domain.code(((code - 1) as usize / stride) % domain.size())
    }

    pub(crate) fn sequence_element(&self) -> Option<(&Field, usize)> {
        match &self.kind {
            Kind::Sequence { element, offsets } => Some((element, offsets.len() - 2)),
            Kind::Record { .. } => None,
        }
    }

    fn sequence_digits(&self, ordinal: usize) -> (&Field, impl Iterator<Item = usize>) {
        let Kind::Sequence { element, offsets } = &self.kind else {
            unreachable!("sequence layout")
        };
        let len = offsets.partition_point(|&offset| offset <= ordinal) - 1;
        let mut rank = ordinal - offsets[len];
        let digits = (0..len).map(move |_| {
            let digit = rank % element.size();
            rank /= element.size();
            digit
        });
        (element, digits)
    }

    pub(crate) fn sequence_codes(&self, code: u64) -> Vec<u64> {
        let (element, digits) = self.sequence_digits((code - 1) as usize);
        let mut items: Vec<_> = digits.map(|ordinal| element.code(ordinal)).collect();
        items.reverse();
        items
    }

    pub(crate) fn sequence(element: Field, max: usize) -> Option<Self> {
        let mut offsets = vec![0usize];
        let mut block = 1usize;
        for length in 0..=max {
            offsets.push(offsets.last()?.checked_add(block)?);
            if length < max {
                block = block.checked_mul(element.size())?;
            }
        }
        let count = *offsets.last()?;
        count.checked_add(1)?;
        Some(Self {
            count,
            kind: Kind::Sequence { element, offsets },
        })
    }

    pub(crate) fn record(name: String, fields: Vec<(String, Field)>) -> Option<Self> {
        let count = fields.iter().try_fold(1usize, |count, (_, domain)| {
            count.checked_mul(domain.size())
        })?;
        count.checked_add(1)?;
        Some(Self {
            count,
            kind: Kind::Record { name, fields },
        })
    }

    pub(crate) fn encode(&self, values: impl ExactSizeIterator<Item = Option<u64>>) -> Option<u64> {
        let rank = match &self.kind {
            Kind::Sequence { element, offsets } => {
                let len = values.len();
                if len >= offsets.len() - 1 {
                    return None;
                }
                let within = values.into_iter().try_fold(0, |rank, code| {
                    Some(rank * element.size() + element.ordinal(code?)?)
                })?;
                offsets[len] + within
            }
            Kind::Record { fields, .. } => {
                if values.len() != fields.len() {
                    return None;
                }
                values
                    .zip(fields)
                    .try_fold(0, |rank, (code, (_, domain))| {
                        Some(rank * domain.size() + domain.ordinal(code?)?)
                    })?
            }
        };
        (rank < self.count).then_some(rank as u64 + 1)
    }

    pub(crate) fn decode(&self, ordinal: usize, atoms: &[Data]) -> Data {
        match &self.kind {
            Kind::Sequence { .. } => {
                let (element, digits) = self.sequence_digits(ordinal);
                let mut items: Vec<_> = digits
                    .map(|ordinal| element.decode(ordinal, atoms))
                    .collect();
                items.reverse();
                Data::Sequence(items)
            }
            Kind::Record { name, fields } => {
                let mut rank = ordinal;
                let mut values: Vec<_> = fields
                    .iter()
                    .rev()
                    .map(|(name, domain)| {
                        let value = domain.decode(rank % domain.size(), atoms);
                        rank /= domain.size();
                        (name.clone(), value)
                    })
                    .collect();
                values.reverse();
                Data::Record(name.clone(), values)
            }
        }
    }
}
