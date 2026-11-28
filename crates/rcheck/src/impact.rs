use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};

pub(crate) fn run(path: &str, state: &str, flags: &[String]) -> Result<Value, String> {
    let options = match flags {
        [] => None,
        [flag, spec] if flag == "--spec" => Some((spec, Path::new("."))),
        [flag, spec, root_flag, root] if flag == "--spec" && root_flag == "--root" => {
            Some((spec, Path::new(root)))
        }
        _ => crate::usage(),
    };
    let impact = rcheck::project::impact::state_impact(path, state)?;
    let mut report = json!({
        "model": path,
        "state": impact.state,
        "location": impact.location,
        "actions": impact.actions,
        "checks": impact.checks,
        "derivations": impact.derivations,
    });
    if let Some((spec_path, root)) = options {
        let source =
            fs::read_to_string(spec_path).map_err(|error| format!("{spec_path}: {error}"))?;
        let spec = idealis_spec::from_json(&source)?;
        let model = Path::new(path)
            .canonicalize()
            .map_err(|error| format!("{path}: {error}"))?;
        let same_model = |name: &str| {
            root.join(name)
                .canonicalize()
                .is_ok_and(|path| path == model)
        };
        let checks: BTreeSet<&str> = impact
            .checks
            .iter()
            .map(|check| check.name.as_str())
            .collect();
        let rules: Vec<_> = spec
            .rules
            .iter()
            .filter_map(|rule| {
                let linked: BTreeSet<&str> = rule
                    .all_claims()
                    .filter(|claim| {
                        checks.contains(claim.check.as_str()) && same_model(&claim.model)
                    })
                    .map(|claim| claim.check.as_str())
                    .collect();
                (!linked.is_empty()).then(|| json!({"id": rule.id, "checks": linked}))
            })
            .collect();
        let groundings: Vec<_> = spec.groundings.iter()
            .filter(|link| same_model(&link.model))
            .map(|link| json!({
                "implementation": {"path": link.implementation.path, "symbol": link.implementation.symbol},
                "test": {"path": link.test.path, "symbol": link.test.symbol},
            }))
            .collect();
        report["rules"] = json!(rules);
        report["groundings"] = json!(groundings);
    }
    Ok(report)
}
