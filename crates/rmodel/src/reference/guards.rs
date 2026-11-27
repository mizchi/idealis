//! Split guards at the first clause that may read an operation argument.
use super::{Scene, Universe};
use crate::{Action, Formula};

pub(super) struct GuardPlan<'a> {
    before: Vec<&'a Formula>,
    after: Vec<&'a Formula>,
}

impl<'a> GuardPlan<'a> {
    pub(super) fn new(action: &'a Action) -> Self {
        let (before, after) = action.guard_parts();
        Self { before, after }
    }

    pub(super) fn before_binding(&self, universe: &Universe<'_>, scene: &Scene<'_>) -> bool {
        self.before
            .iter()
            .all(|clause| universe.formula(clause, scene))
    }

    pub(super) fn after_binding(&self, universe: &Universe<'_>, scene: &Scene<'_>) -> bool {
        self.after
            .iter()
            .all(|clause| universe.formula(clause, scene))
    }
}
