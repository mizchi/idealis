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
    /// An engine-side store; excludes the snapshot fields above.
    #[serde(default)]
    cache_handle: Option<u32>,
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
/// A cache kept between requests with the options it was opened with.
struct Store {
    cache: CheckCache,
    build_digest: String,
    max_entries: u32,
}
/// The cache a request uses: its own snapshot, or a store kept in place.
enum Target<'a> {
    Snapshot(Box<(CheckCache, CacheRequest)>),
    Store(u32, &'a mut Store),
}
impl Target<'_> {
    fn parts(&mut self) -> (&mut CheckCache, CacheOptions<'_>) {
        match self {
            Self::Snapshot(snapshot) => {
                let (cache, request) = &mut **snapshot;
                (cache, request.options())
            }
            Self::Store(_, store) => (
                &mut store.cache,
                CacheOptions {
                    enabled: true,
                    build_digest: &store.build_digest,
                    max_entries: store.max_entries,
                },
            ),
        }
    }
    /// The reply field naming the cache after the request.
    fn reply(self) -> (&'static str, Value) {
        match self {
            Self::Snapshot(snapshot) => ("cache", json!(snapshot.0)),
            Self::Store(handle, _) => ("cacheHandle", json!(handle)),
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
    #[serde(rename_all = "camelCase")]
    CacheOpen {
        build_digest: String,
        max_entries: u32,
        #[serde(default)]
        cache: Option<CheckCache>,
    },
    #[serde(rename_all = "camelCase")]
    CacheExport {
        cache_handle: u32,
    },
    #[serde(rename_all = "camelCase")]
    CacheClose {
        cache_handle: u32,
    },
}
#[derive(Default)]
pub struct Engine {
    next: u32,
    problems: BTreeMap<u32, Solver>,
    next_cache: u32,
    caches: BTreeMap<u32, Store>,
}
fn target(
    caches: &mut BTreeMap<u32, Store>,
    mut request: CacheRequest,
) -> Result<Target<'_>, Error> {
    let Some(handle) = request.cache_handle else {
        let cache = request.cache.take().unwrap_or_default();
        return Ok(Target::Snapshot(Box::new((cache, request))));
    };
    if request.cache.is_some()
        || request.enabled
        || !request.build_digest.is_empty()
        || request.max_entries != 0
    {
        return Err(Error::new(
            "protocol",
            "cacheHandle excludes snapshot fields",
        ));
    }
    let store = caches
        .get_mut(&handle)
        .ok_or_else(|| Error::new("protocol", "unknown cache handle"))?;
    Ok(Target::Store(handle, store))
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
                cache,
            } => {
                let mut target = target(&mut self.caches, cache)?;
                let script = Script::parse(&source)?;
                let (store, options) = target.parts();
                let reply = script.solve_incremental(max_work, store, options)?;
                let (field, value) = target.reply();
                let mut reply = json!({
                    "kind": "incremental-script-results",
                    "results": reply.results,
                    "stats": reply.stats,
                });
                reply[field] = value;
                Ok(reply)
            }
            Request::CheckIncremental {
                handle,
                max_work,
                assumptions,
                cache,
            } => {
                let problem = self
                    .problems
                    .get(&handle)
                    .ok_or_else(|| Error::new("protocol", "unknown handle"))?;
                let mut target = target(&mut self.caches, cache)?;
                let (store, options) = target.parts();
                let reply = problem.check_incremental(&assumptions, max_work, store, options)?;
                let (field, value) = target.reply();
                let mut reply = json!({
                    "kind": "incremental-solution",
                    "result": reply.result,
                    "stats": reply.stats,
                });
                reply[field] = value;
                Ok(reply)
            }
            Request::Release { handle } => {
                if self.problems.remove(&handle).is_none() {
                    return Err(Error::new("protocol", "unknown handle"));
                }
                Ok(json!({"kind":"released"}))
            }
            Request::CacheOpen {
                build_digest,
                max_entries,
                cache,
            } => {
                let options = CacheOptions {
                    enabled: true,
                    build_digest: &build_digest,
                    max_entries,
                };
                let cache = CheckCache::open(cache, &options)?;
                let next = self
                    .next_cache
                    .checked_add(1)
                    .ok_or_else(|| Error::new("resource", "cache handles exhausted"))?;
                let reply = json!({
                    "kind": "cache-opened",
                    "cacheHandle": next,
                    "entries": cache.len(),
                    "invalidated": cache.stats().invalidated,
                });
                self.next_cache = next;
                self.caches.insert(
                    next,
                    Store {
                        cache,
                        build_digest,
                        max_entries,
                    },
                );
                Ok(reply)
            }
            Request::CacheExport { cache_handle } => {
                let store = self
                    .caches
                    .get(&cache_handle)
                    .ok_or_else(|| Error::new("protocol", "unknown cache handle"))?;
                Ok(json!({"kind":"cache-snapshot","cache":store.cache}))
            }
            Request::CacheClose { cache_handle } => {
                if self.caches.remove(&cache_handle).is_none() {
                    return Err(Error::new("protocol", "unknown cache handle"));
                }
                Ok(json!({"kind":"released"}))
            }
        }
    }
}
