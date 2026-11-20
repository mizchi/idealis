use crate::ir::Scalar;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

// MurmurHash3 fmix64 by Austin Appleby (public domain).
fn mix_packed(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^ (value >> 33)
}

#[derive(Debug)]
pub(super) struct FunctionDomain {
    pub keys: Box<[Scalar]>,
    pub hash: u64,
    pub depth: u8,
    #[cfg(not(target_arch = "wasm32"))]
    pub root_owned: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub root: Option<Arc<FunctionDomain>>,
}

impl PartialEq for FunctionDomain {
    fn eq(&self, other: &Self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let left = self.root.as_deref().unwrap_or(self);
            let right = other.root.as_deref().unwrap_or(other);
            if std::ptr::eq(left, right) {
                return true;
            }
        }
        self.keys == other.keys
    }
}
impl Eq for FunctionDomain {}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Values {
    Booleans(u64),
    BinaryIntegers(u64),
    Scalars(Vec<Scalar>),
}

impl Values {
    fn packed(values: impl Clone + ExactSizeIterator<Item = Scalar>) -> Option<Self> {
        if values.len() > 64 {
            return None;
        }
        if values.clone().all(|value| matches!(value, Scalar::Bool(_))) {
            return Some(Self::Booleans(
                values.enumerate().fold(0, |bits, (index, value)| {
                    bits | (u64::from(value == Scalar::Bool(true)) << index)
                }),
            ));
        }
        if values
            .clone()
            .all(|value| matches!(value, Scalar::Int(0 | 1)))
        {
            return Some(Self::BinaryIntegers(
                values.enumerate().fold(0, |bits, (index, value)| {
                    bits | (u64::from(value == Scalar::Int(1)) << index)
                }),
            ));
        }
        None
    }
    fn new(values: Vec<Scalar>) -> Self {
        Self::packed(values.iter().copied()).unwrap_or(Self::Scalars(values))
    }
    fn get(&self, index: usize) -> Scalar {
        match self {
            Self::Booleans(bits) => Scalar::Bool(bits & (1_u64 << index) != 0),
            Self::BinaryIntegers(bits) => Scalar::Int(((bits >> index) & 1) as i32),
            Self::Scalars(values) => values[index],
        }
    }
    fn updated(&self, index: usize, value: Scalar, length: usize) -> Self {
        let bit = match (self, value) {
            (Self::Booleans(_), Scalar::Bool(value)) => Some(u64::from(value)),
            (Self::BinaryIntegers(_), Scalar::Int(value @ (0 | 1))) => Some(value as u64),
            _ => None,
        };
        match (self, bit) {
            (Self::Booleans(bits), Some(bit)) => {
                Self::Booleans((bits & !(1_u64 << index)) | (bit << index))
            }
            (Self::BinaryIntegers(bits), Some(bit)) => {
                Self::BinaryIntegers((bits & !(1_u64 << index)) | (bit << index))
            }
            _ => {
                let mut values = match self {
                    Self::Scalars(values) => values.clone(),
                    _ => (0..length).map(|index| self.get(index)).collect(),
                };
                values[index] = value;
                Self::new(values)
            }
        }
    }
}

pub(crate) struct FunctionValues {
    values: Values,
    position: usize,
    length: usize,
}
impl Iterator for FunctionValues {
    type Item = Scalar;
    fn next(&mut self) -> Option<Scalar> {
        if self.position == self.length {
            return None;
        }
        let value = self.values.get(self.position);
        self.position += 1;
        Some(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Function {
    pub domain: Arc<FunctionDomain>,
    values: Values,
}

pub(super) struct FunctionUpdate<'a> {
    domain: &'a Arc<FunctionDomain>,
    values: &'a [Scalar],
    index: usize,
    value: Scalar,
}

impl FunctionUpdate<'_> {
    pub fn matches(&self, function: &Function) -> bool {
        let Values::Scalars(values) = &function.values else {
            return false;
        };
        *self.domain == function.domain
            && values[..self.index] == self.values[..self.index]
            && values[self.index] == self.value
            && values[self.index + 1..] == self.values[self.index + 1..]
    }

    pub fn materialize(self, domain: Arc<FunctionDomain>) -> Function {
        let mut values = self.values.to_vec();
        values[self.index] = self.value;
        Function {
            domain,
            values: Values::Scalars(values),
        }
    }
}

impl Hash for FunctionUpdate<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.values.len().hash(state);
        for (key, value) in self.domain.keys[..self.index]
            .iter()
            .zip(&self.values[..self.index])
        {
            (*key, *value).hash(state);
        }
        (self.domain.keys[self.index], self.value).hash(state);
        for (key, value) in self.domain.keys[self.index + 1..]
            .iter()
            .zip(&self.values[self.index + 1..])
        {
            (*key, *value).hash(state);
        }
    }
}

impl Function {
    pub fn from_entries(domain: Arc<FunctionDomain>, entries: Vec<(Scalar, Scalar)>) -> Self {
        debug_assert_eq!(domain.keys.len(), entries.len());
        let values = Values::packed(entries.iter().map(|entry| entry.1)).unwrap_or_else(|| {
            let mut values = Vec::with_capacity(entries.len());
            values.extend(entries.into_iter().map(|entry| entry.1));
            Values::Scalars(values)
        });
        Self { domain, values }
    }
    pub fn len(&self) -> usize {
        self.domain.keys.len()
    }
    pub fn is_empty(&self) -> bool {
        self.domain.keys.is_empty()
    }
    pub fn position(&self, key: Scalar) -> Option<usize> {
        if let Scalar::Int(integer) = key
            && integer > 0
        {
            let index = integer as usize - 1;
            if self.domain.keys.get(index) == Some(&key) {
                return Some(index);
            }
        }
        self.domain
            .keys
            .iter()
            .position(|candidate| *candidate == key)
    }
    pub fn value(&self, index: usize) -> Scalar {
        self.values.get(index)
    }
    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (Scalar, Scalar)> + ExactSizeIterator + '_ {
        self.domain
            .keys
            .iter()
            .enumerate()
            .map(|(index, key)| (*key, self.values.get(index)))
    }
    pub fn values(&self) -> FunctionValues {
        FunctionValues {
            values: self.values.clone(),
            position: 0,
            length: self.len(),
        }
    }
    pub fn binary_kind(&self) -> Option<bool> {
        match self.values {
            Values::Booleans(_) => Some(false),
            Values::BinaryIntegers(_) => Some(true),
            _ => None,
        }
    }
    pub fn updated(&self, index: usize, value: Scalar, domain: Arc<FunctionDomain>) -> Self {
        Self {
            domain,
            values: self.values.updated(index, value, self.len()),
        }
    }
    #[inline]
    pub fn scalar_update(&self, index: usize, value: Scalar) -> Option<FunctionUpdate<'_>> {
        let Values::Scalars(values) = &self.values else {
            return None;
        };
        if Values::packed(values.iter().enumerate().map(|(position, previous)| {
            if position == index { value } else { *previous }
        }))
        .is_some()
        {
            return None;
        }
        Some(FunctionUpdate {
            domain: &self.domain,
            values,
            index,
            value,
        })
    }
    pub fn depth(&self, mut depth: impl FnMut(Scalar) -> u8) -> u8 {
        let values = match &self.values {
            Values::Scalars(values) => values.iter().map(|value| depth(*value)).max().unwrap_or(0),
            _ => 0,
        };
        1 + self.domain.depth.max(values)
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn remap(self, domain: Arc<FunctionDomain>, mapping: &super::ValueRemap) -> Self {
        let values = match self.values {
            Values::Scalars(values) => {
                Values::new(values.into_iter().map(|value| mapping.get(value)).collect())
            }
            values => values,
        };
        Self { domain, values }
    }
}

impl Hash for Function {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.values {
            Values::Scalars(_) => {
                self.len().hash(state);
                for entry in self.iter() {
                    entry.hash(state);
                }
            }
            Values::Booleans(bits) | Values::BinaryIntegers(bits) => {
                let kind = if matches!(self.values, Values::BinaryIntegers(_)) {
                    0x9e37_79b9_7f4a_7c15
                } else {
                    0
                };
                mix_packed(self.domain.hash ^ bits ^ kind).hash(state);
            }
        }
    }
}

#[cfg(test)]
mod packed_hash_tests {
    use super::mix_packed;

    #[test]
    fn finalizer_matches_upstream_cpp_vectors() {
        for (input, expected) in [
            (0, 0),
            (1, 0xb456_bcfc_34c2_cb2c),
            (0x8000_0000_0000_0000, 0x8f78_0810_af31_a493),
            (u64::MAX, 0x64b5_720b_4b82_5f21),
            (0x0123_4567_89ab_cdef, 0x87cb_fbfe_8902_2cea),
            (0x9e37_79b9_7f4a_7c15, 0x9ca0_66f1_a4ab_2eea),
            (0x181c_6c6b_0000_0000, 0x29d3_0c42_04df_2565),
        ] {
            assert_eq!(mix_packed(input), expected);
        }
    }
}
