use crate::{Assumption, Error, Problem, Result, Solution, parser};
use std::sync::Arc;

pub struct Solver {
    source: String,
    frames: Vec<usize>,
    problem: Arc<Problem>,
}
impl Solver {
    pub fn parse(source: &str) -> Result<Self> {
        let problem = Problem::parse(source)?;
        Ok(Self {
            source: parser::session_source(source)?,
            frames: vec![],
            problem: Arc::new(problem),
        })
    }
    pub fn assert(&mut self, expression: &str) -> Result<()> {
        let source = format!("{}\n{}", self.source, parser::assertion(expression)?);
        let problem = Problem::parse(&source)?;
        self.source = source;
        self.problem = Arc::new(problem);
        Ok(())
    }
    pub fn push(&mut self, count: u32) -> Result<()> {
        if count > (256 - self.frames.len()) as u32 {
            return Err(Error::new("resource", "scope depth exceeds 256"));
        }
        for _ in 0..count {
            self.frames.push(self.source.len());
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
        let length = self.frames[level];
        let problem = Problem::parse(&self.source[..length])?;
        self.source.truncate(length);
        self.frames.truncate(level);
        self.problem = Arc::new(problem);
        Ok(())
    }
    pub fn check(&self, max_work: u32) -> Solution {
        self.problem.solve(max_work)
    }
    pub fn check_assuming(&self, assumptions: &[Assumption], max_work: u32) -> Result<Solution> {
        self.problem.solve_assuming(assumptions, max_work)
    }
    pub(super) fn snapshot(&self) -> Arc<Problem> {
        self.problem.clone()
    }
}
