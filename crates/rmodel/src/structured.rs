use crate::{Error, INT, Model, NAT, Record};
use std::collections::BTreeMap;

pub const MAX_SEQUENCE_LENGTH: usize = 4;
const MAX_VALUES: usize = 64;

#[derive(Clone, Debug)]
pub struct Sequence {
    pub name: String,
    pub element: String,
    pub max: usize,
}

#[derive(Clone, Debug)]
pub struct Variant {
    pub name: String,
    pub cases: Vec<Case>,
}

#[derive(Clone, Debug)]
pub struct Case {
    pub tag: String,
    pub payload: Option<String>,
}

#[derive(Clone, Copy)]
pub(crate) enum ValueType<'a> {
    Record(&'a Record),
    Sequence(&'a Sequence),
    Variant(&'a Variant),
}

impl<'a> ValueType<'a> {
    pub(crate) fn name(self) -> &'a str {
        match self {
            Self::Record(r) => &r.name,
            Self::Sequence(s) => &s.name,
            Self::Variant(v) => &v.name,
        }
    }

    fn children(self) -> Vec<&'a str> {
        match self {
            Self::Record(r) => r.fields.iter().map(|(_, sort)| sort.as_str()).collect(),
            Self::Sequence(s) => vec![&s.element],
            Self::Variant(v) => v
                .cases
                .iter()
                .filter_map(|case| case.payload.as_deref())
                .collect(),
        }
    }
}

impl Model {
    pub fn sequence(&self, name: &str) -> Option<&Sequence> {
        self.sequences.iter().find(|s| s.name == name)
    }

    pub fn variant(&self, name: &str) -> Option<&Variant> {
        self.variants.iter().find(|v| v.name == name)
    }

    pub fn case(&self, tag: &str) -> Option<(&Variant, &Case)> {
        self.variants.iter().find_map(|variant| {
            variant
                .cases
                .iter()
                .find(|case| case.tag == tag)
                .map(|case| (variant, case))
        })
    }

    pub fn has_structures(&self) -> bool {
        !self.sequences.is_empty() || !self.variants.is_empty()
    }

    pub(crate) fn value_order(&self) -> Result<Vec<ValueType<'_>>, Error> {
        let mut types: Vec<_> = self
            .records
            .iter()
            .map(ValueType::Record)
            .chain(self.sequences.iter().map(ValueType::Sequence))
            .chain(self.variants.iter().map(ValueType::Variant))
            .collect();
        let mut sizes = BTreeMap::new();
        for name in &self.sigs {
            sizes.insert(name.as_str(), self.scope.sizes[name]);
        }
        for (name, members) in &self.enums {
            sizes.insert(name.as_str(), members.len());
        }
        for (name, children) in &self.sums {
            sizes.insert(
                name.as_str(),
                children.iter().map(|child| sizes[child.as_str()]).sum(),
            );
        }
        if let Some((low, high)) = self.scope.ints {
            sizes.insert(INT, (high - low + 1) as usize);
            sizes.insert(NAT, (high - low.max(0) + 1).max(0) as usize);
        }

        let mut ordered = Vec::new();
        while !types.is_empty() {
            let index = types
                .iter()
                .position(|ty| ty.children().iter().all(|t| sizes.contains_key(t)))
                .ok_or_else(|| {
                    Error::kind(format!(
                        "{}: unknown or recursive value type",
                        types[0].name()
                    ))
                })?;
            let ty = types.remove(index);
            let size = match ty {
                ValueType::Record(record) => {
                    record.fields.iter().fold(1usize, |size, (_, sort)| {
                        size.saturating_mul(sizes[sort.as_str()])
                    })
                }
                ValueType::Sequence(seq) => {
                    if !(1..=MAX_SEQUENCE_LENGTH).contains(&seq.max) {
                        return Err(Error::kind(format!(
                            "{}: sequence max must be 1..={MAX_SEQUENCE_LENGTH}",
                            seq.name
                        )));
                    }
                    let elements = sizes[seq.element.as_str()];
                    (0..=seq.max).fold(0usize, |total, length| {
                        total.saturating_add(elements.saturating_pow(length as u32))
                    })
                }
                ValueType::Variant(v) => {
                    if v.cases.is_empty() {
                        return Err(Error::kind(format!("{}: variant needs a case", v.name)));
                    }
                    v.cases.iter().fold(0usize, |size, case| {
                        size.saturating_add(case.payload.as_deref().map_or(1, |t| sizes[t]))
                    })
                }
            };
            if self.has_structures() && size > MAX_VALUES {
                return Err(Error::kind(format!(
                    "{}: value domain exceeds {MAX_VALUES} values",
                    ty.name()
                )));
            }
            sizes.insert(ty.name(), size);
            ordered.push(ty);
        }
        Ok(ordered)
    }
}
