//! Bind one supplied input using its declared types.
use super::{Atom, Data, Next, Runtime, Scene, Snapshot, Value};
use idealis_syntax::{Action, INT, NAT, Relation, Shape};
use std::collections::BTreeMap;

/// One operation with arguments that have been admitted to their value domains.
pub struct BoundCommand<'a> {
    action: &'a Action,
    args: BTreeMap<String, Atom>,
}

impl<'a> Runtime<'a> {
    /// Read only the supplied configuration and check the model facts.
    pub fn read_configuration(
        &self,
        tuples: impl FnMut(&Relation) -> Option<Vec<Vec<Data>>>,
    ) -> Option<Snapshot> {
        self.accept_configuration(self.read_snapshot(&self.statics, tuples)?)
    }

    /// Read only the supplied state and check the model's initial conditions.
    pub fn read_initial(
        &self,
        configuration: &Snapshot,
        tuples: impl FnMut(&Relation) -> Option<Vec<Vec<Data>>>,
    ) -> Option<Snapshot> {
        self.accept_initial(configuration, self.read_snapshot(&self.vars, tuples)?)
    }

    fn read_snapshot(
        &self,
        relations: &[&Relation],
        mut tuples: impl FnMut(&Relation) -> Option<Vec<Vec<Data>>>,
    ) -> Option<Snapshot> {
        relations
            .iter()
            .map(|relation| {
                let tuples = tuples(relation)?;
                let count = tuples.len();
                let mut value = Value::new();
                for tuple in tuples {
                    let decoded = match (&relation.shape, tuple.as_slice()) {
                        (Shape::Unary { sig, .. }, [atom]) => {
                            vec![self.read_atom(sig, atom)?]
                        }
                        (Shape::Binary { from, to, .. }, [key, atom]) => {
                            vec![self.read_atom(from, key)?, self.read_atom(to, atom)?]
                        }
                        _ => return None,
                    };
                    value.insert(decoded);
                }
                (value.len() == count && self.universe.admits(&relation.shape, &value))
                    .then_some(value)
            })
            .collect::<Option<_>>()
            .map(Snapshot)
    }

    /// Bind one command. The argument callback gets its model name and sort.
    pub fn read_command(
        &self,
        name: &str,
        mut argument: impl FnMut(&str, &str) -> Option<Data>,
    ) -> Option<BoundCommand<'a>> {
        let action = self
            .model
            .actions
            .iter()
            .find(|action| action.name == name)?;
        let args = action
            .params
            .iter()
            .map(|(name, sort)| {
                let atom = self.read_atom(sort, &argument(name, sort)?)?;
                Some((name.clone(), atom))
            })
            .collect::<Option<_>>()?;
        Some(BoundCommand { action, args })
    }

    /// Apply a command bound by this runtime when its guards hold.
    pub fn execute(
        &self,
        configuration: &Snapshot,
        state: &Snapshot,
        command: &BoundCommand<'_>,
    ) -> Option<Next> {
        let relations = self.relations(&configuration.0, &state.0);
        let scene = Scene {
            relations: &relations,
            after: None,
            bound: &command.args,
        };
        self.next(command.action, &scene, state)
    }

    /// Check all transition properties that apply to an accepted command.
    pub fn violated_command_transition(
        &self,
        configuration: &Snapshot,
        before: &Snapshot,
        after: &Snapshot,
        command: &BoundCommand<'_>,
    ) -> Option<&str> {
        self.violated_transition_for(
            configuration,
            before,
            after,
            Some(&command.action.name),
            &command.args,
        )
    }

    fn read_atom(&self, sort: &str, data: &Data) -> Option<Atom> {
        match data {
            Data::Atom(name) => self
                .universe
                .sorts
                .get(sort)?
                .iter()
                .find(|atom| matches!(atom, Atom::Id(id) if &self.names[id] == name))
                .cloned(),
            Data::Int(value) if matches!(sort, INT | NAT) => {
                let atom = Atom::Int(*value);
                self.universe.contains(sort, &atom).then_some(atom)
            }
            Data::Sequence(items) => {
                let seq = self.model.sequence(sort)?;
                if items.len() > seq.max {
                    return None;
                }
                let values = items
                    .iter()
                    .map(|data| self.read_atom(&seq.element, data))
                    .collect::<Option<_>>()?;
                Some(Atom::Sequence(values))
            }
            Data::Record(name, fields) if name == sort => {
                let record = self.model.record(sort)?;
                if fields.len() != record.fields.len() {
                    return None;
                }
                let values = record
                    .fields
                    .iter()
                    .zip(fields)
                    .map(|((name, sort), (field, data))| {
                        (field == name)
                            .then(|| self.read_atom(sort, data))
                            .flatten()
                    })
                    .collect::<Option<Vec<_>>>()?;
                let atom = Atom::Record(self.universe.record_index(sort), values.into());
                self.universe.satisfies_refinements(&atom).then_some(atom)
            }
            Data::Variant(tag, payload) => {
                let variant = self.model.variant(sort)?;
                let case = variant.cases.iter().find(|case| &case.tag == tag)?;
                let payload = match (&case.payload, payload) {
                    (Some(sort), Some(data)) => Some(Box::new(self.read_atom(sort, data)?)),
                    (None, None) => None,
                    _ => return None,
                };
                Some(Atom::Variant(tag.clone(), payload))
            }
            _ => None,
        }
    }
}
