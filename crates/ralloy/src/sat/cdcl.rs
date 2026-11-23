//! A small CDCL SAT solver (plan 0085): two watched literals, first-UIP
//! clause learning, non-chronological backjumping, activity-ordered
//! decisions with saved phases and Luby restarts. Every propagated literal
//! and every conflict spends one unit of the analysis budget.
use crate::eval::Budget;

/// `variable * 2 + negated`.
pub(crate) type Lit = u32;
pub(crate) fn var(lit: Lit) -> usize {
    (lit >> 1) as usize
}
pub(crate) fn negate(lit: Lit) -> Lit {
    lit ^ 1
}

pub(crate) struct Solver {
    clauses: Vec<Vec<Lit>>,
    /// Clauses watching each literal; visited when it becomes false.
    watches: Vec<Vec<usize>>,
    /// Per variable: `None` unassigned, `Some(value)`.
    values: Vec<Option<bool>>,
    levels: Vec<usize>,
    reasons: Vec<Option<usize>>,
    trail: Vec<Lit>,
    limits: Vec<usize>,
    head: usize,
    activity: Vec<f64>,
    increment: f64,
    phases: Vec<bool>,
    /// An empty clause or a contradiction among units was added.
    contradiction: bool,
}

impl Solver {
    pub(crate) fn new(variables: usize) -> Self {
        Self {
            clauses: Vec::new(),
            watches: vec![Vec::new(); variables * 2],
            values: vec![None; variables],
            levels: vec![0; variables],
            reasons: vec![None; variables],
            trail: Vec::new(),
            limits: Vec::new(),
            head: 0,
            activity: vec![0.0; variables],
            increment: 1.0,
            phases: vec![false; variables],
            contradiction: false,
        }
    }
    fn value(&self, lit: Lit) -> Option<bool> {
        self.values[var(lit)].map(|value| value != (lit & 1 == 1))
    }
    fn level(&self) -> usize {
        self.limits.len()
    }
    fn assign(&mut self, lit: Lit, reason: Option<usize>) {
        let variable = var(lit);
        self.values[variable] = Some(lit & 1 == 0);
        self.levels[variable] = self.level();
        self.reasons[variable] = reason;
        self.trail.push(lit);
    }
    /// Adds a clause before solving; duplicate literals and tautologies are
    /// removed.
    pub(crate) fn add(&mut self, clause: &[Lit]) {
        let mut lits = clause.to_vec();
        lits.sort_unstable();
        lits.dedup();
        if lits.windows(2).any(|pair| pair[0] == negate(pair[1])) {
            return;
        }
        match lits.len() {
            0 => self.contradiction = true,
            1 => match self.value(lits[0]) {
                Some(true) => {}
                Some(false) => self.contradiction = true,
                None => self.assign(lits[0], None),
            },
            _ => {
                let index = self.clauses.len();
                self.watches[lits[0] as usize].push(index);
                self.watches[lits[1] as usize].push(index);
                self.clauses.push(lits);
            }
        }
    }
    /// Unit propagation from the trail; returns a conflicting clause.
    fn propagate(&mut self, budget: &mut Budget) -> Result<Option<usize>, ()> {
        while self.head < self.trail.len() {
            budget.tick()?;
            let falsified = negate(self.trail[self.head]);
            self.head += 1;
            let watching = std::mem::take(&mut self.watches[falsified as usize]);
            let mut kept = Vec::with_capacity(watching.len());
            let mut conflict = None;
            for (position, &index) in watching.iter().enumerate() {
                if conflict.is_some() {
                    kept.extend_from_slice(&watching[position..]);
                    break;
                }
                let clause = &mut self.clauses[index];
                if clause[0] == falsified {
                    clause.swap(0, 1);
                }
                let other = clause[0];
                if self.values[var(other)].map(|value| value != (other & 1 == 1)) == Some(true) {
                    kept.push(index);
                    continue;
                }
                let replacement = (2..clause.len()).find(|&candidate| {
                    let lit = clause[candidate];
                    self.values[var(lit)].map(|value| value != (lit & 1 == 1)) != Some(false)
                });
                if let Some(candidate) = replacement {
                    clause.swap(1, candidate);
                    let watched = clause[1];
                    self.watches[watched as usize].push(index);
                    continue;
                }
                kept.push(index);
                match self.value(other) {
                    Some(false) => conflict = Some(index),
                    _ => self.assign(other, Some(index)),
                }
            }
            self.watches[falsified as usize] = kept;
            if conflict.is_some() {
                return Ok(conflict);
            }
        }
        Ok(None)
    }
    /// First-UIP learning; returns the learnt clause (asserting literal
    /// first) and the level to jump back to.
    fn analyze(&mut self, conflict: usize) -> (Vec<Lit>, usize) {
        let mut seen = vec![false; self.values.len()];
        let mut learnt = vec![0];
        let mut pending = 0usize;
        let mut implied: Option<Lit> = None;
        let mut clause_index = conflict;
        let mut index = self.trail.len();
        let asserting = loop {
            let clause = self.clauses[clause_index].clone();
            for &lit in &clause {
                let variable = var(lit);
                if implied.is_some_and(|implied| var(implied) == variable)
                    || seen[variable]
                    || self.levels[variable] == 0
                {
                    continue;
                }
                seen[variable] = true;
                self.bump(variable);
                if self.levels[variable] == self.level() {
                    pending += 1;
                } else {
                    learnt.push(lit);
                }
            }
            loop {
                index -= 1;
                if seen[var(self.trail[index])] {
                    break;
                }
            }
            let lit = self.trail[index];
            seen[var(lit)] = false;
            pending -= 1;
            if pending == 0 {
                break lit;
            }
            implied = Some(lit);
            clause_index = self.reasons[var(lit)].expect("implied literal");
        };
        learnt[0] = negate(asserting);
        let jump = learnt[1..]
            .iter()
            .map(|lit| self.levels[var(*lit)])
            .max()
            .unwrap_or(0);
        if let Some(position) = learnt[1..]
            .iter()
            .position(|lit| self.levels[var(*lit)] == jump)
        {
            learnt.swap(1, position + 1);
        }
        self.increment *= 1.05;
        (learnt, jump)
    }
    fn bump(&mut self, variable: usize) {
        self.activity[variable] += self.increment;
        if self.activity[variable] > 1e100 {
            for activity in &mut self.activity {
                *activity *= 1e-100;
            }
            self.increment *= 1e-100;
        }
    }
    fn backjump(&mut self, level: usize) {
        if self.level() <= level {
            return;
        }
        let start = self.limits[level];
        for lit in self.trail.drain(start..) {
            let variable = var(lit);
            self.phases[variable] = lit & 1 == 0;
            self.values[variable] = None;
            self.reasons[variable] = None;
        }
        self.limits.truncate(level);
        self.head = self.trail.len();
    }
    fn decide(&mut self) -> Option<Lit> {
        let variable = (0..self.values.len())
            .filter(|variable| self.values[*variable].is_none())
            .max_by(|left, right| self.activity[*left].total_cmp(&self.activity[*right]))?;
        Some((variable as Lit) << 1 | Lit::from(!self.phases[variable]))
    }
    /// A satisfying assignment, `None` when unsatisfiable, or `Err` when the
    /// budget runs out.
    pub(crate) fn solve(&mut self, budget: &mut Budget) -> Result<Option<Vec<bool>>, ()> {
        if self.contradiction {
            return Ok(None);
        }
        let mut conflicts = 0u64;
        let mut restart = 1u64;
        let mut limit = luby(restart) * 64;
        loop {
            if let Some(conflict) = self.propagate(budget)? {
                budget.tick()?;
                if self.level() == 0 {
                    return Ok(None);
                }
                let (learnt, jump) = self.analyze(conflict);
                self.backjump(jump);
                if learnt.len() == 1 {
                    self.assign(learnt[0], None);
                } else {
                    let index = self.clauses.len();
                    self.watches[learnt[0] as usize].push(index);
                    self.watches[learnt[1] as usize].push(index);
                    let asserting = learnt[0];
                    self.clauses.push(learnt);
                    self.assign(asserting, Some(index));
                }
                conflicts += 1;
                if conflicts >= limit {
                    conflicts = 0;
                    restart += 1;
                    limit = luby(restart) * 64;
                    self.backjump(0);
                }
                continue;
            }
            match self.decide() {
                None => {
                    return Ok(Some(
                        self.values
                            .iter()
                            .map(|value| value == &Some(true))
                            .collect(),
                    ));
                }
                Some(lit) => {
                    self.limits.push(self.trail.len());
                    self.assign(lit, None);
                }
            }
        }
    }
}

/// The Luby sequence 1, 1, 2, 1, 1, 2, 4, ...
fn luby(mut index: u64) -> u64 {
    loop {
        let mut size = 1u64;
        while size < index + 1 {
            size = size * 2 + 1;
        }
        let half = size / 2;
        if index + 1 == size {
            return half + 1;
        }
        index -= half;
    }
}

#[cfg(test)]
mod tests {
    use super::{Lit, Solver};
    use crate::{Stats, eval::Budget};

    fn satisfies(clauses: &[Vec<Lit>], model: &[bool]) -> bool {
        clauses.iter().all(|clause| {
            clause
                .iter()
                .any(|lit| model[(lit >> 1) as usize] != (lit & 1 == 1))
        })
    }

    /// Random formulas against exhaustive enumeration of all assignments.
    #[test]
    fn agrees_with_exhaustive_enumeration_on_random_formulas() {
        let mut state = 0x0085_u64;
        let mut next = |bound: u64| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) % bound
        };
        let (mut satisfiable, mut unsatisfiable) = (0, 0);
        for _ in 0..2000 {
            let variables = 1 + next(10) as usize;
            let count = next(45) as usize;
            let clauses: Vec<Vec<Lit>> = (0..count)
                .map(|_| {
                    (0..1 + next(4))
                        .map(|_| (next(variables as u64) as Lit) << 1 | next(2) as Lit)
                        .collect()
                })
                .collect();
            let exhaustive = (0u32..1 << variables).any(|bits| {
                let model: Vec<bool> = (0..variables).map(|v| bits >> v & 1 == 1).collect();
                satisfies(&clauses, &model)
            });
            let mut solver = Solver::new(variables);
            for clause in &clauses {
                solver.add(clause);
            }
            let mut budget = Budget {
                stats: Stats::default(),
                limit: 10_000_000,
            };
            match solver.solve(&mut budget).unwrap() {
                Some(model) => {
                    assert!(exhaustive, "{clauses:?}");
                    assert!(satisfies(&clauses, &model), "{clauses:?}");
                    satisfiable += 1;
                }
                None => {
                    assert!(!exhaustive, "{clauses:?}");
                    unsatisfiable += 1;
                }
            }
        }
        assert!(
            satisfiable > 300 && unsatisfiable > 300,
            "{satisfiable} {unsatisfiable}"
        );
    }
}
