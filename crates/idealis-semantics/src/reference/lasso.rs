//! Truth on a concrete, replayed ultimately periodic behavior.
use super::{Atom, Next, Runtime, Scene, Snapshot, bindings};
use idealis_syntax::{Action, Fairness, FairnessKind, Formula};
use std::collections::BTreeMap;

#[derive(Debug)]
pub enum LassoError {
    Invalid(String),
    Unverified(String),
}

type Binding = BTreeMap<String, Atom>;

fn charge(work: &mut u64) -> Result<(), LassoError> {
    *work = work.checked_sub(1).ok_or_else(|| {
        LassoError::Unverified("temporal verification work budget exhausted".into())
    })?;
    Ok(())
}

impl Runtime<'_> {
    /// Check a replayed lasso, including the repeated closing state at the end.
    pub fn verify_temporal_lasso(
        &self,
        configuration: &Snapshot,
        states: &[Snapshot],
        loop_start: usize,
        name: &str,
        mut work: u64,
    ) -> Result<(), LassoError> {
        let Some(cycle) = states.get(loop_start..).filter(|cycle| cycle.len() >= 2) else {
            return Err(LassoError::Invalid(
                "loopStart must name a state before the closing edge".into(),
            ));
        };
        if cycle.last() != cycle.first() {
            return Err(LassoError::Invalid(
                "last replayed step does not close the loop".into(),
            ));
        }
        let check = self
            .model
            .temporal
            .iter()
            .find(|check| check.name == name)
            .ok_or_else(|| LassoError::Invalid(format!("no temporal property named {name}")))?;
        for fairness in check.fairness.as_ref().unwrap_or(&self.model.fairness) {
            let action = self
                .model
                .actions
                .iter()
                .find(|action| action.name == fairness.action)
                .expect("checked fairness action");
            self.check_groundings(&action.params)?;
            let mut sample = |pair: &[Snapshot], bound: &Binding| {
                charge(&mut work)?;
                Ok(self.fair_step(configuration, pair, action, bound))
            };
            let fair = if fairness.each {
                let mut all_fair = true;
                for bound in bindings(&self.universe, &action.params) {
                    if !fair_cycle(fairness, cycle, |pair| sample(pair, &bound))? {
                        all_fair = false;
                        break;
                    }
                }
                all_fair
            } else {
                fair_cycle(fairness, cycle, |pair| {
                    let mut enabled = false;
                    for bound in bindings(&self.universe, &action.params) {
                        let (candidate_enabled, taken) = sample(pair, &bound)?;
                        if taken {
                            return Ok((true, true));
                        }
                        enabled |= candidate_enabled;
                    }
                    Ok((enabled, false))
                })?
            };
            if !fair {
                return Err(LassoError::Invalid(format!(
                    "lasso does not satisfy fairness for {}",
                    action.name
                )));
            }
        }
        self.check_groundings(&check.params)?;
        for bound in bindings(&self.universe, &check.params) {
            let mut holds = |state: &Snapshot, formula: &Formula| {
                charge(&mut work)?;
                let relations = self.relations(&configuration.0, &state.0);
                Ok(self.universe.formula(
                    formula,
                    &Scene {
                        relations: &relations,
                        after: None,
                        bound: &bound,
                    },
                ))
            };
            let mut unfulfilled = check.trigger.is_none();
            for (index, state) in states[..states.len() - 1].iter().enumerate().rev() {
                if holds(state, &check.formula)? {
                    // A response in the repeated part satisfies every trigger.
                    if index >= loop_start || check.trigger.is_none() {
                        unfulfilled = false;
                    }
                    break;
                }
                if let Some(trigger) = &check.trigger
                    && holds(state, trigger)?
                {
                    unfulfilled = true;
                }
            }
            if unfulfilled {
                return Ok(());
            }
        }
        Err(LassoError::Invalid(format!(
            "lasso does not violate {name}"
        )))
    }

    fn check_groundings(&self, params: &[(String, String)]) -> Result<(), LassoError> {
        match self.universe.argument_count(params) {
            Some(0) => Ok(()),
            Some(_) if params.iter().all(|(_, sort)| self.universe.can_index(sort)) => Ok(()),
            _ => Err(LassoError::Unverified(
                "temporal parameter domains exceed the reference enumeration range".into(),
            )),
        }
    }

    fn fair_step(
        &self,
        configuration: &Snapshot,
        pair: &[Snapshot],
        action: &Action,
        bound: &Binding,
    ) -> (bool, bool) {
        let before = &pair[0];
        let after = &pair[1];
        let relations = self.relations(&configuration.0, &before.0);
        let scene = Scene {
            relations: &relations,
            after: None,
            bound,
        };
        let Some(next) = self.next(action, &scene, before) else {
            return (false, false);
        };
        // Fair_A in the lowering restricts the pre-state. Even a successor
        // outside the replayable domain can enable its nonstuttering action.
        let candidate = match next {
            Next::State(state) | Next::Outside(state) | Next::Broken(state) => state,
        };
        let enabled = candidate != *before;
        (enabled, enabled && candidate == *after)
    }
}

fn fair_cycle(
    fairness: &Fairness,
    cycle: &[Snapshot],
    mut sample: impl FnMut(&[Snapshot]) -> Result<(bool, bool), LassoError>,
) -> Result<bool, LassoError> {
    let mut ever_enabled = false;
    for pair in cycle.windows(2) {
        let (enabled, taken) = sample(pair)?;
        if taken || fairness.kind == FairnessKind::Weak && !enabled {
            return Ok(true);
        }
        ever_enabled |= enabled;
    }
    Ok(!ever_enabled)
}
