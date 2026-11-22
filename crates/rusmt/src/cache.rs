//! Opt-in reuse of complete check answers.
//!
//! An entry maps one query, the SHA-256 of the normalized source plus the
//! ordered assumptions, to the `sat` model or `unsat` answer a fresh search
//! returned. `unknown` and errors are never stored. The namespace is the build
//! digest of the engine; the caller must derive it from the actual binary and
//! everything else that affects solving. Entry digests detect accidental
//! corruption, not forgery.
use crate::{Assumption, Error, Problem, Result, Solution, Stats, Variable};
use num_bigint::BigInt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const FORMAT: &str = "rusmt.check-results";
const VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default)]
pub struct CacheOptions<'a> {
    pub enabled: bool,
    /// Lowercase hex SHA-256 identifying the engine build.
    pub build_digest: &'a str,
    /// Retained entries; 0 disables writes.
    pub max_entries: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheStats {
    pub hits: u32,
    pub misses: u32,
    pub writes: u32,
    /// Entries of another build were dropped before this call.
    pub invalidated: bool,
}

#[derive(Serialize)]
pub struct IncrementalCheck {
    pub result: Solution,
    pub stats: CacheStats,
}

#[derive(Serialize)]
pub struct IncrementalScript {
    pub results: Vec<Solution>,
    pub stats: CacheStats,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    status: String,
    values: BTreeMap<String, String>,
    booleans: BTreeMap<String, bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    #[serde(flatten)]
    answer: Answer,
    digest: String,
    #[serde(skip)]
    verified: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CheckCache {
    format: String,
    version: u32,
    build_digest: String,
    entries: BTreeMap<String, Entry>,
    #[serde(skip)]
    capacity: usize,
    #[serde(skip)]
    stats: CacheStats,
}
impl Default for CheckCache {
    fn default() -> Self {
        Self {
            format: FORMAT.into(),
            version: VERSION,
            build_digest: String::new(),
            entries: BTreeMap::new(),
            capacity: 0,
            stats: CacheStats::default(),
        }
    }
}

/// Key of a query whose normalized source hashes to `source`.
pub(crate) fn key(source: &[u8; 32], assumptions: &[Assumption]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source);
    for assumption in assumptions {
        hasher.update([u8::from(assumption.positive)]);
        hasher.update((assumption.symbol.len() as u64).to_le_bytes());
        hasher.update(assumption.symbol.as_bytes());
    }
    hex(&hasher.finalize())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn encode(value: &impl Serialize) -> Result<String> {
    serde_json::to_string(value).map_err(|error| Error::new("protocol", error.to_string()))
}

impl CheckCache {
    /// Validates the header and options, then drops entries of another build.
    pub(crate) fn bind(&mut self, options: &CacheOptions) -> Result<()> {
        if self.format != FORMAT || self.version != VERSION {
            return Err(Error::new("protocol", "unsupported check cache"));
        }
        let digest = options.build_digest;
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Error::new("protocol", "expected build SHA-256"));
        }
        let invalidated = self.build_digest != digest;
        self.stats = CacheStats {
            invalidated: invalidated && !self.entries.is_empty(),
            ..CacheStats::default()
        };
        if invalidated {
            self.entries.clear();
            self.build_digest = digest.into();
        }
        self.capacity = options.max_entries as usize;
        while self.entries.len() > self.capacity {
            self.entries.pop_last();
        }
        Ok(())
    }
    pub(crate) fn stats(&self) -> CacheStats {
        self.stats
    }
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
    /// Binds a snapshot for in-place use, as the first request of the snapshot
    /// path would, and rejects it whole if any entry digest is wrong.
    pub(crate) fn open(snapshot: Option<Self>, options: &CacheOptions) -> Result<Self> {
        let mut cache = snapshot.unwrap_or_default();
        cache.bind(options)?;
        for (key, entry) in &cache.entries {
            if entry.digest != cache.digest(key, &entry.answer)? {
                return Err(Error::new("protocol", "check cache digest mismatch"));
            }
        }
        Ok(cache)
    }
    fn digest(&self, key: &str, answer: &Answer) -> Result<String> {
        Ok(hex(&Sha256::digest(
            encode(&(FORMAT, VERSION, &self.build_digest, key, answer))?.as_bytes(),
        )))
    }
    /// The stored answer for `key`, checked against `problem` on first use.
    pub(crate) fn lookup(&mut self, key: &str, problem: &Problem) -> Result<Option<Solution>> {
        let Some(entry) = self.entries.get(key) else {
            self.stats.misses += 1;
            return Ok(None);
        };
        if !entry.verified {
            if entry.digest != self.digest(key, &entry.answer)? {
                return Err(Error::new("protocol", "check cache digest mismatch"));
            }
            if !fits(&entry.answer, problem) {
                return Err(Error::new("protocol", "invalid cached answer"));
            }
            self.entries.get_mut(key).expect("present entry").verified = true;
        }
        self.stats.hits += 1;
        let answer = &self.entries[key].answer;
        Ok(Some(Solution {
            status: answer.status.clone(),
            values: answer.values.clone(),
            booleans: answer.booleans.clone(),
            reason: None,
            stats: Stats::default(),
        }))
    }
    /// Stores a complete answer while capacity remains.
    pub(crate) fn record(&mut self, key: String, solution: &Solution) -> Result<()> {
        if !matches!(solution.status.as_str(), "sat" | "unsat")
            || self.entries.len() >= self.capacity
        {
            return Ok(());
        }
        let answer = Answer {
            status: solution.status.clone(),
            values: solution.values.clone(),
            booleans: solution.booleans.clone(),
        };
        let digest = self.digest(&key, &answer)?;
        self.entries.insert(
            key,
            Entry {
                answer,
                digest,
                verified: true,
            },
        );
        self.stats.writes += 1;
        Ok(())
    }
    /// Adds the entries of `other` from the same build, keeping at most
    /// `max_entries` in key order. Corrupt or conflicting entries are rejected.
    pub fn merge(&mut self, other: Self, max_entries: u32) -> Result<()> {
        for cache in [&*self, &other] {
            if cache.format != FORMAT || cache.version != VERSION {
                return Err(Error::new("protocol", "unsupported check cache"));
            }
        }
        if self.build_digest != other.build_digest {
            return Err(Error::new("protocol", "different check cache builds"));
        }
        for (key, entry) in self.entries.iter().chain(&other.entries) {
            if entry.digest != self.digest(key, &entry.answer)? {
                return Err(Error::new("protocol", "check cache digest mismatch"));
            }
        }
        for (key, entry) in &other.entries {
            if self
                .entries
                .get(key)
                .is_some_and(|stored| stored.answer != entry.answer)
            {
                return Err(Error::new("protocol", "conflicting complete checks"));
            }
        }
        self.entries.extend(other.entries);
        self.capacity = max_entries as usize;
        while self.entries.len() > self.capacity {
            self.entries.pop_last();
        }
        Ok(())
    }
}

/// A stored answer must have the shape a fresh solve of `problem` produces.
fn fits(answer: &Answer, problem: &Problem) -> bool {
    match answer.status.as_str() {
        "unsat" => answer.values.is_empty() && answer.booleans.is_empty(),
        "sat" => {
            let integers = problem
                .variables
                .iter()
                .filter(|(_, (variable, _))| matches!(variable, Variable::Int(_)));
            let booleans = problem
                .variables
                .iter()
                .filter(|(_, (variable, _))| matches!(variable, Variable::Bool(_)));
            integers.clone().count() == answer.values.len()
                && integers
                    .zip(&answer.values)
                    .all(|((name, _), (stored, value))| {
                        name == stored
                            && value
                                .parse::<BigInt>()
                                .is_ok_and(|parsed| parsed.to_string() == *value)
                    })
                && booleans.clone().count() == answer.booleans.len()
                && booleans
                    .zip(&answer.booleans)
                    .all(|((name, _), (stored, _))| name == stored)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILD: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn cache_with(answer: &str) -> CheckCache {
        let mut cache = CheckCache::default();
        cache
            .bind(&CacheOptions {
                enabled: true,
                build_digest: BUILD,
                max_entries: 4,
            })
            .unwrap();
        let solution = Solution {
            status: "sat".into(),
            values: BTreeMap::from([("x".into(), answer.into())]),
            booleans: BTreeMap::new(),
            reason: None,
            stats: Stats::default(),
        };
        cache.record(key(&[0; 32], &[]), &solution).unwrap();
        cache
    }

    #[test]
    fn merge_rejects_validly_signed_conflicting_answers() {
        let mut left = cache_with("1");
        left.merge(cache_with("1"), 4).unwrap();
        let error = left.merge(cache_with("2"), 4).err().unwrap();
        assert_eq!(error.message, "conflicting complete checks");
    }

    #[test]
    fn validly_signed_answers_must_fit_the_problem() {
        let problem = Problem::parse("(declare-const x Int)(declare-const b Bool)").unwrap();
        for (status, values, booleans) in [
            ("sat", vec![("x", "1")], vec![]),
            ("sat", vec![("y", "1")], vec![("b", true)]),
            ("sat", vec![("x", "01")], vec![("b", true)]),
            ("unsat", vec![("x", "1")], vec![]),
            ("unknown", vec![], vec![]),
        ] {
            let mut cache = cache_with("1");
            let answer = Answer {
                status: status.into(),
                values: values
                    .into_iter()
                    .map(|(name, value)| (name.into(), value.into()))
                    .collect(),
                booleans: booleans
                    .into_iter()
                    .map(|(name, value)| (name.into(), value))
                    .collect(),
            };
            let key = key(&[0; 32], &[]);
            let digest = cache.digest(&key, &answer).unwrap();
            cache.entries.insert(
                key.clone(),
                Entry {
                    answer,
                    digest,
                    verified: false,
                },
            );
            let error = cache.lookup(&key, &problem).err().unwrap();
            assert_eq!(error.message, "invalid cached answer", "{status}");
        }
        let mut cache = cache_with("1");
        let key = key(&[0; 32], &[]);
        // Control: the same re-signed entry with a fitting model is accepted.
        let mut answer = cache.entries[&key].answer.clone();
        answer.booleans = BTreeMap::from([("b".into(), true)]);
        let digest = cache.digest(&key, &answer).unwrap();
        cache.entries.insert(
            key.clone(),
            Entry {
                answer,
                digest,
                verified: false,
            },
        );
        assert_eq!(cache.lookup(&key, &problem).unwrap().unwrap().status, "sat");
    }

    #[test]
    fn keys_separate_assumption_order_polarity_and_symbols() {
        let literal = |symbol: &str, positive| Assumption {
            symbol: symbol.into(),
            positive,
        };
        let keys = [
            key(&[0; 32], &[]),
            key(&[1; 32], &[]),
            key(&[0; 32], &[literal("a", true)]),
            key(&[0; 32], &[literal("a", false)]),
            key(&[0; 32], &[literal("a", true), literal("b", true)]),
            key(&[0; 32], &[literal("b", true), literal("a", true)]),
            key(&[0; 32], &[literal("ab", true)]),
        ];
        let unique: std::collections::BTreeSet<_> = keys.iter().collect();
        assert_eq!(unique.len(), keys.len());
    }
}
