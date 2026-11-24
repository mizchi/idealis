//! Differential tests and comparative benchmarks against an explicit oracle.
mod bench;
mod group;
mod report;
#[cfg(feature = "typescript")]
pub mod typescript;
#[cfg(target_arch = "wasm32")]
mod wasm;
mod wire;
mod wire_observation;

pub use bench::{BenchCase, BenchConfig, BenchFailure, BenchReport, Measurement, Statistics};
pub use group::OracleGroup;
pub use report::{Case, CaseReport, CheckReport, Comparison, Error, Observation, Status};

/// The comparison rules, for tests that ground them against models (plans
/// 0093, 0094). Not a stable API.
#[doc(hidden)]
pub mod grounding {
    use crate::{Observation, Status};
    pub fn aggregate(statuses: impl Iterator<Item = Status>) -> Status {
        Status::aggregate(statuses)
    }
    pub fn compare<T: PartialEq>(reference: &Observation<T>, actual: &Observation<T>) -> Status {
        crate::report::compare(reference, actual)
    }
}
pub use wire::{dispatch, dispatch_json, dispatch_value, dispatch_value_to_json};
