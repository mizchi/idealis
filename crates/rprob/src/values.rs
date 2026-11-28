use idealis_syntax::Record;
use std::{collections::BTreeMap, sync::Arc};

pub(crate) type Records<'a> = BTreeMap<&'a str, (&'a Record, Arc<RecordLayout>)>;

/// Record values follow the same order as their fields' Cartesian product,
/// with the last field varying fastest.
pub(crate) struct RecordLayout {
    pub(crate) values: Vec<u64>,
    pub(crate) fields: Vec<Vec<u64>>,
}

impl RecordLayout {
    pub(crate) fn encode(&self, fields: impl Iterator<Item = Option<u64>>) -> Option<u64> {
        let mut rank = 0;
        for (value, domain) in fields.zip(&self.fields) {
            let value = value?;
            rank = rank * domain.len() + domain.iter().position(|&bit| bit == value)?;
        }
        self.values.get(rank).copied()
    }

    pub(crate) fn field(&self, rank: usize, field: usize) -> u64 {
        let divisor: usize = self.fields[field + 1..].iter().map(Vec::len).product();
        let domain = &self.fields[field];
        domain[(rank / divisor) % domain.len()]
    }
}

pub(crate) fn project(input: u64, mapping: &[(u64, u64)]) -> u64 {
    mapping
        .iter()
        .filter(|(from, _)| input & from != 0)
        .fold(0, |bits, (_, to)| bits | to)
}

pub(crate) type Cases<'a> = BTreeMap<&'a str, Arc<CaseLayout>>;

pub(crate) struct CaseLayout {
    pub(crate) mask: u64,
    pub(crate) numeric: bool,
    /// Pairs of the tagged bit and its payload bit. Unit cases have no pairs.
    pub(crate) payloads: Vec<(u64, u64)>,
}

impl CaseLayout {
    pub(crate) fn encode(&self, payload: u64) -> Option<u64> {
        self.payloads
            .iter()
            .find_map(|&(tagged, value)| (value == payload).then_some(tagged))
    }
}
