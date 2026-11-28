use super::{is_manifest, read_source};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Serialize)]
pub struct StateImpact {
    pub state: String,
    pub location: String,
    pub actions: Vec<ActionImpact>,
    pub checks: Vec<CheckImpact>,
    pub derivations: Vec<DerivationImpact>,
}

#[derive(Serialize)]
pub struct ActionImpact {
    pub name: String,
    pub location: String,
    pub reads: bool,
    pub writes: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckImpact {
    pub name: String,
    pub location: String,
    pub reads: bool,
    pub via_actions: BTreeSet<String>,
}

#[derive(Serialize)]
pub struct DerivationImpact {
    pub name: String,
    pub location: String,
}

/// Find named users of a mutable state declaration in a linked source model.
pub fn state_impact(path: &str, state: &str) -> Result<StateImpact, String> {
    if path.ends_with(".json") && !is_manifest(path) {
        return Err("impact requires model source or a project manifest".into());
    }
    let source = read_source(path)?;
    let located = source.located_model(path)?;
    let model = &located.model;
    if !model.relation(state).is_some_and(|relation| relation.var) {
        return Err(format!("{path}: unknown state variable {state}"));
    }
    let location = |name: &str| source.position(located.declarations[name]);
    let reads = |name: &str| {
        located
            .state_reads
            .get(&located.declarations[name])
            .is_some_and(|states| states.contains(state))
    };
    let actions: Vec<_> = model
        .actions
        .iter()
        .filter_map(|action| {
            let reads = reads(&action.name);
            let writes = action.assignments.iter().any(|(target, _)| target == state);
            (reads || writes).then(|| ActionImpact {
                name: action.name.clone(),
                location: location(&action.name),
                reads,
                writes,
            })
        })
        .collect();
    let affected: BTreeSet<&str> = actions.iter().map(|action| action.name.as_str()).collect();
    let check = |name: &str, via_actions: BTreeSet<String>| {
        let reads = reads(name);
        (reads || !via_actions.is_empty()).then(|| CheckImpact {
            name: name.to_owned(),
            location: location(name),
            reads,
            via_actions,
        })
    };
    let checks = model
        .checks
        .iter()
        .filter_map(|property| check(&property.name, BTreeSet::new()))
        .chain(model.transitions.iter().filter_map(|property| {
            let via_actions = property
                .action
                .iter()
                .filter(|action| affected.contains(action.as_str()))
                .cloned()
                .collect();
            check(&property.name, via_actions)
        }))
        .chain(model.temporal.iter().filter_map(|property| {
            let via_actions = property
                .fairness
                .as_deref()
                .unwrap_or(&model.fairness)
                .iter()
                .filter(|fair| affected.contains(fair.action.as_str()))
                .map(|fair| fair.action.clone())
                .collect();
            check(&property.name, via_actions)
        }))
        .collect();
    let derivations = model
        .derivations
        .iter()
        .filter(|(name, _)| reads(name))
        .map(|(name, _)| DerivationImpact {
            name: name.clone(),
            location: location(name),
        })
        .collect();
    Ok(StateImpact {
        state: state.to_owned(),
        location: location(state),
        actions,
        checks,
        derivations,
    })
}
