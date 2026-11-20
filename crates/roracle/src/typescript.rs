use crate::wire::{Input, Request, Response};
use crate::wire_observation::WireObservation;
use crate::{Case, CaseReport, CheckReport, Comparison, Observation, Status};
use serde_json::Value;
use ts_rs::{Config, TS};

pub fn definitions() -> String {
    let config = Config::default();
    let declarations = [
        Value::decl(&config),
        Status::decl(&config),
        Observation::<Value>::decl(&config),
        WireObservation::decl(&config),
        Case::<Value>::decl(&config),
        Comparison::<Value>::decl(&config),
        CaseReport::<Value, Value>::decl(&config),
        CheckReport::<Value, Value>::decl(&config),
        Input::decl(&config),
        Request::decl(&config),
        Response::decl(&config),
    ];
    format!(
        "// Generated from roracle Rust contracts. Run just oracle-types.\n{}\n",
        declarations.map(|line| format!("export {line}")).join("\n")
    )
}
