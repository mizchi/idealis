use crate::{Error, Work, canonical, text};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionEdge {
    pub action: Value,
    pub target: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Generator {
    pub id: String,
    pub version: String,
    pub initial: Vec<String>,
    pub states: BTreeMap<String, Vec<ActionEdge>>,
    pub min_length: usize,
    pub max_length: usize,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Case {
    pub format: String,
    pub version: u32,
    pub rng: String,
    pub generator_id: String,
    pub generator_version: String,
    pub seed: u32,
    pub index: u32,
    pub actions: Vec<Value>,
}
impl Case {
    pub(crate) fn normalize(mut self) -> Result<Self, Error> {
        if self.format != "idealis.test-case"
            || self.version != 1
            || self.rng != "xorshift32-v1"
            || self.seed == 0
        {
            return Err(Error::protocol("unsupported case identity or seed"));
        }
        text(&self.generator_id)?;
        text(&self.generator_version)?;
        if self.actions.len() > 128
            || serde_json::to_vec(&self).expect("serializable case").len() > 1_048_576
        {
            return Err(Error::resource("case exceeds 128 actions or 1 MiB"));
        }
        self.actions = self
            .actions
            .iter()
            .map(|action| canonical(action, 0))
            .collect::<Result<_, _>>()?;
        Ok(self)
    }
}
pub(crate) struct Random(pub u32);
impl Random {
    fn choose(&mut self, maximum: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as usize % maximum
    }
}
impl Generator {
    pub(crate) fn normalize(mut self) -> Result<Self, Error> {
        text(&self.id)?;
        text(&self.version)?;
        if self.min_length > self.max_length || self.max_length > 128 {
            return Err(Error::resource(
                "generator length must satisfy min <= max <= 128",
            ));
        }
        if self.states.len() > 1024 || self.states.values().map(Vec::len).sum::<usize>() > 10_000 {
            return Err(Error::resource(
                "generator exceeds 1024 states or 10000 edges; reduce model domains or use a verified state projection",
            ));
        }
        if self.initial.is_empty()
            || self.initial.iter().collect::<BTreeSet<_>>().len() != self.initial.len()
            || self
                .initial
                .iter()
                .any(|key| !self.states.contains_key(key))
        {
            return Err(Error::protocol(
                "initial states must be nonempty, unique and declared",
            ));
        }
        for (key, edges) in &self.states {
            text(key)?;
            for edge in edges {
                if !self.states.contains_key(&edge.target) {
                    return Err(Error::protocol("unknown generator target"));
                }
            }
        }
        for edges in self.states.values_mut() {
            for edge in edges {
                if serde_json::to_vec(&edge.action)
                    .expect("serializable action")
                    .len()
                    > 8192
                {
                    return Err(Error::resource("action exceeds 8 KiB"));
                }
                edge.action = canonical(&edge.action, 0)?;
            }
        }
        Ok(self)
    }
    pub(crate) fn generate(
        &self,
        random: &mut Random,
        seed: u32,
        index: u32,
        work: &mut Work,
    ) -> Result<Option<Case>, ()> {
        work.tick()?;
        let length = self.min_length + random.choose(self.max_length - self.min_length + 1);
        let mut state = &self.initial[random.choose(self.initial.len())];
        let mut actions = Vec::new();
        for _ in 0..length {
            work.tick()?;
            let edges = &self.states[state];
            if edges.is_empty() {
                break;
            }
            let edge = &edges[random.choose(edges.len())];
            actions.push(edge.action.clone());
            state = &edge.target;
        }
        Ok((actions.len() >= self.min_length).then(|| Case {
            format: "idealis.test-case".into(),
            version: 1,
            rng: "xorshift32-v1".into(),
            generator_id: self.id.clone(),
            generator_version: self.version.clone(),
            seed,
            index,
            actions,
        }))
    }
    pub(crate) fn accepts(&self, actions: &[Value], work: &mut Work) -> Result<bool, ()> {
        if actions.len() < self.min_length || actions.len() > self.max_length {
            return Ok(false);
        }
        let mut current: BTreeSet<_> = self.initial.iter().collect();
        for action in actions {
            let mut next = BTreeSet::new();
            for state in current {
                for edge in &self.states[state] {
                    work.tick()?;
                    if edge.action == *action {
                        next.insert(&edge.target);
                    }
                }
            }
            if next.is_empty() {
                return Ok(false);
            }
            current = next;
        }
        Ok(true)
    }
}
