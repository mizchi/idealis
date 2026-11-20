use crate::{Assumption, Error, Problem, Script, Solver};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    version: u32,
    request: Request,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Request {
    RunScript {
        source: String,
        #[serde(rename = "maxWork")]
        max_work: u32,
    },
    Solve {
        source: String,
        #[serde(rename = "maxWork")]
        max_work: u32,
    },
    Prepare {
        source: String,
    },
    Check {
        handle: u32,
        #[serde(rename = "maxWork")]
        max_work: u32,
        #[serde(default)]
        assumptions: Vec<Assumption>,
    },
    Assert {
        handle: u32,
        expression: String,
    },
    Push {
        handle: u32,
        count: u32,
    },
    Pop {
        handle: u32,
        count: u32,
    },
    Release {
        handle: u32,
    },
}
#[derive(Default)]
pub struct Engine {
    next: u32,
    problems: BTreeMap<u32, Solver>,
}
impl Engine {
    pub fn dispatch(&mut self, bytes: &[u8]) -> Value {
        match self.execute(bytes) {
            Ok(value) => value,
            Err(error) => json!({"kind":"error","code":error.code,"message":error.message}),
        }
    }
    fn execute(&mut self, bytes: &[u8]) -> Result<Value, Error> {
        let command: Command = serde_json::from_slice(bytes)
            .map_err(|error| Error::new("protocol", error.to_string()))?;
        if command.version != 1 {
            return Err(Error::new("protocol", "unsupported version"));
        }
        match command.request {
            Request::RunScript { source, max_work } => Ok(
                json!({"kind":"script-results","results":Script::parse(&source)?.solve(max_work)}),
            ),
            Request::Solve { source, max_work } => {
                Ok(json!({"kind":"solution","result":Problem::parse(&source)?.solve(max_work)}))
            }
            Request::Prepare { source } => {
                let problem = Solver::parse(&source)?;
                self.next = self
                    .next
                    .checked_add(1)
                    .ok_or_else(|| Error::new("resource", "handles exhausted"))?;
                self.problems.insert(self.next, problem);
                Ok(json!({"kind":"prepared","handle":self.next}))
            }
            Request::Check {
                handle,
                max_work,
                assumptions,
            } => {
                let problem = self
                    .problems
                    .get(&handle)
                    .ok_or_else(|| Error::new("protocol", "unknown handle"))?;
                Ok(
                    json!({"kind":"solution","result":problem.check_assuming(&assumptions,max_work)?}),
                )
            }
            Request::Assert { handle, expression } => {
                self.problems
                    .get_mut(&handle)
                    .ok_or_else(|| Error::new("protocol", "unknown handle"))?
                    .assert(&expression)?;
                Ok(json!({"kind":"updated"}))
            }
            Request::Push { handle, count } => {
                self.problems
                    .get_mut(&handle)
                    .ok_or_else(|| Error::new("protocol", "unknown handle"))?
                    .push(count)?;
                Ok(json!({"kind":"updated"}))
            }
            Request::Pop { handle, count } => {
                self.problems
                    .get_mut(&handle)
                    .ok_or_else(|| Error::new("protocol", "unknown handle"))?
                    .pop(count)?;
                Ok(json!({"kind":"updated"}))
            }
            Request::Release { handle } => {
                if self.problems.remove(&handle).is_none() {
                    return Err(Error::new("protocol", "unknown handle"));
                }
                Ok(json!({"kind":"released"}))
            }
        }
    }
}
