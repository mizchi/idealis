use crate::{Value, ir::SafetyViolation};
use std::fmt::Write;

fn values(items: &[Value]) -> String {
    items.iter().map(render).collect::<Vec<_>>().join(", ")
}

fn record_name(name: &str) -> bool {
    name.chars().all(|c| c == '_' || c.is_alphanumeric())
        && name.chars().any(char::is_alphabetic)
        && !(name.len() > 3 && (name.starts_with("WF_") || name.starts_with("SF_")))
}

fn render(value: &Value) -> String {
    match value {
        Value::Bool(true) => "TRUE".into(),
        Value::Bool(false) => "FALSE".into(),
        Value::Int(n) | Value::ModelValue(n) => n.clone(),
        Value::String(s) => serde_json::to_string(s).expect("serializable string"),
        Value::Set(items) => format!("{{{}}}", values(items)),
        Value::List(items) => format!("<<{}>>", values(items)),
        Value::Record(fields) if !fields.iter().all(|(name, _)| record_name(name)) => {
            let entries = fields
                .iter()
                .map(|(name, v)| {
                    let key = serde_json::to_string(name).expect("serializable string key");
                    format!("{key} :> {}", render(v))
                })
                .collect::<Vec<_>>();
            format!("({})", entries.join(" @@ "))
        }
        Value::Record(fields) => {
            let entries = fields
                .iter()
                .map(|(name, v)| format!("{name} |-> {}", render(v)))
                .collect::<Vec<_>>();
            format!("[{}]", entries.join(", "))
        }
        Value::Map(entries) => {
            let entries = entries
                .iter()
                .map(|(k, v)| format!("{} :> {}", render(k), render(v)))
                .collect::<Vec<_>>();
            format!("({})", entries.join(" @@ "))
        }
        // Portable-only values; the TLA evaluator never produces them.
        Value::Null | Value::Bytes(_) => serde_json::to_string(value).expect("serializable value"),
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&render(self))
    }
}

impl SafetyViolation {
    /// Render a concrete counterexample using the variable ordering of its program.
    pub fn format_trace(&self, variables: &[String], status: &str, diff: bool) -> String {
        let mut output = String::new();
        let steps = &self.trace;
        let prefix = if variables.len() == 1 { "" } else { "/\\ " };
        for (index, step) in steps.iter().enumerate() {
            let label = step.action.as_deref().unwrap_or("Initial predicate");
            if !diff || !(status == "invariant-violation" && steps.len() == 1) {
                writeln!(output, "State {}: <{label}>", index + 1).unwrap();
            }
            // TLCTrace prints the final safety state in full except for a successor
            // of the initial state. A lasso keeps predecessor-relative diffs.
            let full_final = self.loop_start.is_none()
                && index + 1 == steps.len()
                && (status == "deadlock" || index > 1);
            for (slot, (name, value)) in variables.iter().zip(&step.values).enumerate() {
                if !diff
                    || index == 0
                    || full_final
                    || steps[index - 1].values.get(slot) != Some(value)
                {
                    writeln!(output, "{prefix}{name} = {value}").unwrap();
                }
            }
            output.push('\n');
        }
        if let Some(start) = self.loop_start {
            let action = self.loop_action.as_deref().unwrap_or_default();
            if start + 1 == steps.len() {
                writeln!(output, "State {}: Stuttering", steps.len() + 1).unwrap();
            } else {
                writeln!(output, "Back to state {}: {action}", start + 1).unwrap();
            }
        }
        output
    }
}
