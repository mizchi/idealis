//! Driver for zust's HashMapModel.tla. The implementation receives operations only.
use idealis_connect::{
    Generator, Plan, TestStatus,
    tla::{self, ConnectSpec, Driver, SafetyBounds, Value},
};
use std::{collections::HashMap, error::Error};

#[derive(Default)]
struct Store(HashMap<Value, Value>);
impl Driver for Store {
    fn reset(&mut self) -> Result<Value, String> {
        self.0.clear();
        Ok(Value::List(vec![Value::String("ok".into())]))
    }
    fn step(&mut self, action: &Value) -> Result<Value, String> {
        let Value::List(args) = action else {
            return Err("expected tuple".into());
        };
        let Some((Value::String(op), args)) = args.split_first() else {
            return Err("missing operation".into());
        };
        let (tag, value) = match (op.as_str(), args) {
            ("insert", [key, value]) => ("prev", self.0.insert(key.clone(), value.clone())),
            ("remove", [key]) => ("prev", self.0.remove(key)),
            ("get", [key]) => ("val", self.0.get(key).cloned()),
            ("len", []) => ("len", Some(Value::Int(self.0.len().to_string()))),
            _ => return Err(format!("unknown operation: {action}")),
        };
        Ok(Value::List(vec![
            Value::String(tag.into()),
            value.unwrap_or_else(|| Value::Int("0".into())),
        ]))
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let paths: Vec<_> = std::env::args().skip(1).collect();
    let [tla, cfg] = paths.as_slice() else {
        return Err("Usage: tla_store HashMapModel.tla HashMapModel.cfg".into());
    };
    let bounds = SafetyBounds {
        max_states: 100_000,
        max_depth: 100,
        max_work: 200_000_000,
        max_temporal_nodes: None,
    };
    let model = tla::finite_model(
        &std::fs::read_to_string(tla)?,
        &std::fs::read_to_string(cfg)?,
        ConnectSpec::default(),
        bounds,
    )?;
    let plan = Plan {
        generator: Generator::from_finite_model(&model, "1", 1, 16)?,
        seed: 42,
        cases: 64,
        max_attempts: 256,
        max_shrink_checks: 64,
        max_work: 1_000_000,
    };
    let report = tla::check(&model, plan, &mut Store::default())?;
    println!(
        "{} states, {} transitions, {} cases: {:?}",
        model.states.len(),
        model
            .states
            .iter()
            .map(|s| s.transitions.len())
            .sum::<usize>(),
        report.result.cases,
        report.result.status
    );
    if report.result.status != TestStatus::Passed {
        return Err(format!("{:?}", report.counterexample).into());
    }
    Ok(())
}
