use crate::{
    Assumption, CacheOptions, CacheStats, CheckCache, Formula, IncrementalScript, Problem, Result,
    Solution, cache, parser, solver::cached_solve, source::SourceHash,
};
use std::sync::{Arc, OnceLock};

struct Query {
    problem: Arc<Problem>,
    todo: Vec<(Arc<Formula>, bool)>,
    /// The normalized source, hashed only when caching, so plain parsing and
    /// solving pay nothing for the cache key.
    source: SourceHash,
    assumptions: Vec<Assumption>,
    /// The cache key, computed on the first cached solve and kept afterwards.
    key: OnceLock<String>,
}
impl Query {
    fn key(&self) -> &str {
        self.key
            .get_or_init(|| cache::key(&self.source.digest(), &self.assumptions))
    }
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
                    problem,
                    source,
                    assumptions,
                    key: OnceLock::new(),
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
                query.key().to_owned(),
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
