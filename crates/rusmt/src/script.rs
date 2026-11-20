use crate::{Formula, Problem, Result, Solution, parser};
use std::sync::Arc;

struct Query {
    problem: Arc<Problem>,
    todo: Vec<(Arc<Formula>, bool)>,
}
pub struct Script {
    queries: Vec<Query>,
}
impl Script {
    pub fn parse(source: &str) -> Result<Self> {
        let queries = parser::script(source)?
            .into_iter()
            .map(|(problem, assumptions)| {
                Ok(Query {
                    todo: problem.assumption_todo(&assumptions)?,
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
}
