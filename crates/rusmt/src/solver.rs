use crate::{Assumption, Error, Problem, Result, Solution, session::Session};
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
    pub(super) fn snapshot(&self) -> Arc<Problem> {
        self.session.problem().clone()
    }
}
