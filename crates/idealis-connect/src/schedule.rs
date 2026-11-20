use crate::{ActionEdge, Error, Generator, text};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleActor {
    pub id: String,
    pub operations: Vec<Value>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    pub id: String,
    pub version: String,
    pub actors: Vec<ScheduleActor>,
}
fn key(positions: &[usize]) -> String {
    positions
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",")
}
impl Schedule {
    pub fn generator(self) -> Result<Generator, Error> {
        text(&self.id)?;
        text(&self.version)?;
        if self.actors.is_empty() {
            return Err(Error::protocol("schedule needs an actor"));
        }
        if self.actors.len() > 8 {
            return Err(Error::resource("schedule exceeds 8 actors"));
        }
        let mut identities = BTreeSet::new();
        let mut state_count = 1_usize;
        let mut operation_count = 0_usize;
        for actor in &self.actors {
            text(&actor.id)?;
            if !identities.insert(&actor.id) {
                return Err(Error::protocol("duplicate schedule actor"));
            }
            operation_count = operation_count
                .checked_add(actor.operations.len())
                .ok_or_else(|| Error::resource("too many schedule operations"))?;
            if operation_count > 128 {
                return Err(Error::resource("schedule exceeds 128 operations"));
            }
            state_count *= actor.operations.len() + 1;
            if state_count > 1024 {
                return Err(Error::resource("schedule exceeds 1024 states"));
            }
        }
        let mut states = BTreeMap::new();
        for index in 0..state_count {
            let mut remaining = index;
            let positions: Vec<_> = self
                .actors
                .iter()
                .map(|actor| {
                    let position = remaining % (actor.operations.len() + 1);
                    remaining /= actor.operations.len() + 1;
                    position
                })
                .collect();
            let edges =
                self.actors
                    .iter()
                    .enumerate()
                    .filter_map(|(index, actor)| {
                        let position = positions[index];
                        actor.operations.get(position).map(|operation| {
                    let mut target = positions.clone(); target[index] += 1;
                    ActionEdge {
                        action: json!({"actor":actor.id,"step":position,"operation":operation}),
                        target: key(&target),
                    }
                })
                    })
                    .collect();
            states.insert(key(&positions), edges);
        }
        Generator {
            id: self.id,
            version: self.version,
            initial: vec![key(&vec![0; self.actors.len()])],
            states,
            min_length: 0,
            max_length: operation_count,
        }
        .normalize()
    }
}
