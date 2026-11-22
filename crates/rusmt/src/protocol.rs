use crate::{Assumption, CacheOptions, CheckCache, Error, Problem, Script, Solver};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    version: u32,
    request: Request,
}
/// Cache fields shared by the incremental requests; disabled when omitted.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CacheRequest {
    #[serde(default)]
    cache: Option<CheckCache>,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    build_digest: String,
    #[serde(default)]
    max_entries: u32,
}
impl CacheRequest {
    fn options(&self) -> CacheOptions<'_> {
        CacheOptions {
            enabled: self.enabled,
            build_digest: &self.build_digest,
            max_entries: self.max_entries,
        }
    }
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Request {
    RunScriptIncremental {
        source: String,
        #[serde(rename = "maxWork")]
        max_work: u32,
        #[serde(flatten)]
        cache: CacheRequest,
    },
    CheckIncremental {
        handle: u32,
        #[serde(rename = "maxWork")]
        max_work: u32,
        #[serde(default)]
        assumptions: Vec<Assumption>,
        #[serde(flatten)]
        cache: CacheRequest,
    },
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
            Request::RunScriptIncremental {
                source,
                max_work,
                mut cache,
            } => {
                let mut store = cache.cache.take().unwrap_or_default();
                let reply = Script::parse(&source)?.solve_incremental(
                    max_work,
                    &mut store,
                    cache.options(),
                )?;
                Ok(json!({
                    "kind": "incremental-script-results",
                    "results": reply.results,
                    "stats": reply.stats,
                    "cache": store,
                }))
            }
            Request::CheckIncremental {
                handle,
                max_work,
                assumptions,
                mut cache,
            } => {
                let problem = self
                    .problems
                    .get(&handle)
                    .ok_or_else(|| Error::new("protocol", "unknown handle"))?;
                let mut store = cache.cache.take().unwrap_or_default();
                let reply = problem.check_incremental(
                    &assumptions,
                    max_work,
                    &mut store,
                    cache.options(),
                )?;
                Ok(json!({
                    "kind": "incremental-solution",
                    "result": reply.result,
                    "stats": reply.stats,
                    "cache": store,
                }))
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
