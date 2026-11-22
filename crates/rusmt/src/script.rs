use crate::{
    CacheOptions, CacheStats, CheckCache, Formula, IncrementalScript, Problem, Result, Solution,
    cache, parser, solver::cached_solve,
};
use std::sync::Arc;

struct Query {
    problem: Arc<Problem>,
    todo: Vec<(Arc<Formula>, bool)>,
    key: String,
}
pub struct Script {
    queries: Vec<Query>,
}
impl Script {
    pub fn parse(source: &str) -> Result<Self> {
        let queries = parser::script(source)?
            .into_iter()
            .map(|((problem, source), assumptions)| {
                Ok(Query {
                    todo: problem.assumption_todo(&assumptions)?,
                    key: cache::key(&source, &assumptions),
                    problem,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { queries })
    }
    pub fn solve(&self, max_work: u32) -> Vec<Solution> {
        let mut remaining = max_work;
        self.queries
            .iter()
            .map(|query| {
                let result = query.problem.solve_with(remaining, query.todo.clone());
                remaining -= result.stats.work;
                result
            })
            .collect()
    }
    /// Solves like `solve`, reusing complete answers from `cache` when
    /// `options.enabled`. A hit spends no work, so the shared budget left for
    /// later checks can exceed that of `solve`.
    pub fn solve_incremental(
        &self,
        max_work: u32,
        cache: &mut CheckCache,
        options: CacheOptions,
    ) -> Result<IncrementalScript> {
        if !options.enabled {
            return Ok(IncrementalScript {
                results: self.solve(max_work),
                stats: CacheStats::default(),
            });
        }
        cache.bind(&options)?;
        let mut remaining = max_work;
        let mut results = Vec::with_capacity(self.queries.len());
        for query in &self.queries {
            let result = cached_solve(
                cache,
                &query.problem,
                query.key.clone(),
                query.todo.clone(),
                remaining,
            )?;
            remaining -= result.stats.work;
            results.push(result);
        }
        Ok(IncrementalScript {
            results,
            stats: cache.stats(),
        })
    }
}
