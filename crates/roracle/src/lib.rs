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
pub use wire::{dispatch, dispatch_json, dispatch_value, dispatch_value_to_json};
