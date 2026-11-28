use idealis_check::conform::Driver;
use serde_json::Value;

pub fn assert_bank_trace(driver: &mut dyn Driver) {
    let trace: Value =
        serde_json::from_str(include_str!("../../../../fixtures/rcheck/bank.trace.json")).unwrap();
    assert_eq!(driver.reset(&trace["config"]).unwrap(), trace["initial"]);
    for (index, step) in trace["steps"].as_array().unwrap().iter().enumerate() {
        let (accepted, state) = driver.apply(&step["command"]).unwrap();
        assert_eq!(Value::Bool(accepted), step["accepted"], "step {index}");
        assert_eq!(state, step["state"], "step {index}");
    }
}
