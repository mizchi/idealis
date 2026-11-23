use crate::{
    Assumption, CacheOptions, CacheStats, CheckCache, Error, IncrementalCheck, Problem, Result,
    Solution, cache, session::Session, source::SourceHash,
};
use std::sync::Arc;

pub struct Solver {
    session: Session,
    /// The context saved by each open scope; `push(n)` shares one snapshot.
    frames: Vec<Arc<Session>>,
}
impl Solver {
    pub fn parse(source: &str) -> Result<Self> {
        Ok(Self {
            session: Session::parse(source)?,
            frames: vec![],
        })
    }
    pub fn assert(&mut self, expression: &str) -> Result<()> {
        self.session.assert(expression)
    }
    pub fn push(&mut self, count: u32) -> Result<()> {
        if count > (256 - self.frames.len()) as u32 {
            return Err(Error::new("resource", "scope depth exceeds 256"));
        }
        if count > 0 {
            let frame = Arc::new(self.session.clone());
            self.frames
                .extend(std::iter::repeat_n(frame, count as usize));
        }
        Ok(())
    }
    pub fn pop(&mut self, count: u32) -> Result<()> {
        if count as usize > self.frames.len() {
            return Err(Error::new("protocol", "pop exceeds scope depth"));
        }
        if count == 0 {
            return Ok(());
        }
        let level = self.frames.len() - count as usize;
        self.session = Session::clone(&self.frames[level]);
        self.frames.truncate(level);
        Ok(())
    }
    pub fn check(&self, max_work: u32) -> Solution {
        self.session.problem().solve(max_work)
    }
    pub fn check_assuming(&self, assumptions: &[Assumption], max_work: u32) -> Result<Solution> {
        self.session.problem().solve_assuming(assumptions, max_work)
    }
    /// Checks like `check_assuming`, reusing complete answers from `cache` when
    /// `options.enabled`. Disabled options neither read nor update the cache.
    pub fn check_incremental(
        &self,
        assumptions: &[Assumption],
        max_work: u32,
        cache: &mut CheckCache,
        options: CacheOptions,
    ) -> Result<IncrementalCheck> {
        if !options.enabled {
            return Ok(IncrementalCheck {
                result: self.check_assuming(assumptions, max_work)?,
                stats: CacheStats::default(),
            });
        }
        cache.bind(&options)?;
        let todo = self.session.problem().assumption_todo(assumptions)?;
        let result = cached_solve(
            cache,
            self.session.problem(),
            cache::key(&self.session.source().digest(), assumptions),
            todo,
            max_work,
        )?;
        Ok(IncrementalCheck {
            result,
            stats: cache.stats(),
        })
    }
    pub(super) fn snapshot(&self) -> (Arc<Problem>, SourceHash) {
        (
            self.session.problem().clone(),
            self.session.source().clone(),
        )
    }
}

pub(crate) fn cached_solve(
    cache: &mut CheckCache,
    problem: &Problem,
    key: String,
    todo: Vec<(Arc<crate::Formula>, bool)>,
    max_work: u32,
) -> Result<Solution> {
    if let Some(solution) = cache.lookup(&key, problem)? {
        return Ok(solution);
    }
    let solution = problem.solve_with(max_work, todo);
    cache.record(key, &solution)?;
    Ok(solution)
}
