use crate::{Case, Generator, Work};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeletionRange {
    pub start: usize,
    pub count: usize,
}
pub fn deletion_candidates(length: usize) -> Vec<DeletionRange> {
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    let mut size = length;
    while size > 0 {
        for start in (0..length).step_by(size) {
            let count = size.min(length - start);
            if seen.insert((start, count)) {
                output.push(DeletionRange { start, count });
            }
        }
        size = if size == 1 { 0 } else { size.div_ceil(2) };
    }
    output
}
pub(crate) struct Shrinker {
    pub current: Case,
    pub signature: String,
    pub checks: u32,
    pub maximum: u32,
    pub minimal: bool,
    pub reason: Option<String>,
    pub unknown: bool,
    ranges: Vec<DeletionRange>,
    cursor: usize,
}
impl Shrinker {
    pub fn new(current: Case, signature: String, maximum: u32) -> Self {
        let ranges = deletion_candidates(current.actions.len());
        Self {
            current,
            signature,
            checks: 1,
            maximum,
            minimal: false,
            reason: None,
            unknown: false,
            ranges,
            cursor: 0,
        }
    }
    pub fn reduced(&mut self, case: Case) {
        self.current = case;
        self.ranges = deletion_candidates(self.current.actions.len());
        self.cursor = 0;
    }
    pub fn next(&mut self, generator: &Generator, work: &mut Work) -> Option<Case> {
        if self.current.actions.is_empty() {
            self.minimal = !self.unknown;
            if self.unknown {
                self.reason = Some("unknown-candidate".into());
            }
            return None;
        }
        if self.checks + 1 >= self.maximum {
            self.reason = Some("checks".into());
            return None;
        }
        while let Some(range) = self.ranges.get(self.cursor) {
            self.cursor += 1;
            let actions = self
                .current
                .actions
                .iter()
                .enumerate()
                .filter(|(index, _)| *index < range.start || *index >= range.start + range.count)
                .map(|(_, value)| value.clone())
                .collect::<Vec<_>>();
            match work.tick().and_then(|()| generator.accepts(&actions, work)) {
                Ok(true) => {
                    return Some(Case {
                        actions,
                        ..self.current.clone()
                    });
                }
                Ok(false) => (),
                Err(()) => {
                    self.reason = Some("work".into());
                    return None;
                }
            }
        }
        self.minimal = !self.unknown;
        if self.unknown {
            self.reason = Some("unknown-candidate".into());
        }
        None
    }
}
