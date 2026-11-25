//! Plan 0108: adapters write the kernel IR from other surface languages,
//! and roracle compares their verdicts with the rmodel source's. The Pkl
//! adapter's outputs are recorded under fixtures/rcheck/adapters (`just
//! rcheck-adapters` regenerates and compares them); two of its faults are
//! recorded the same way and must be found.
use roracle::{Observation, OracleGroup, Status};

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The observed checks of each model of plan 0107 (SPEC names that both
/// the rmodel source and the adapter's language can state).
const CASES: &[(&str, &[&str])] = &[
    (
        "permissions",
        &["NoLeak", "OnlyGranted", "SecretRead", "UnclearedSecret"],
    ),
    ("bank", &["NonNegative", "Rich"]),
    ("order", &["FraudCancelled"]),
    (
        "sessions",
        &[
            "Capacity",
            "LockedByAdmin",
            "LockedIsActive",
            "Full",
            "ThreeActive",
        ],
    ),
    ("service", &["Disjoint"]),
];

type Verdicts = Vec<(String, String)>;

fn verdicts(model: &rmodel::Model, observed: &[String]) -> Observation<Verdicts> {
    use rmodel::reference::Outcome;
    let report = rmodel::reference::analyze(model, 2_000_000);
    let mut out = Vec::new();
    for name in observed {
        let Some(verdict) = report.verdicts.iter().find(|v| &v.check == name) else {
            return Observation::Unknown {
                reason: format!("no check {name}"),
            };
        };
        let word = match verdict.outcome {
            Outcome::Empty => "empty".to_owned(),
            Outcome::Holds => "holds".to_owned(),
            Outcome::Violated { depth } => format!("violated@{depth}"),
            Outcome::Reachable { depth } => format!("reachable@{depth}"),
            Outcome::Unreachable => "unreachable".to_owned(),
            Outcome::Unknown => "unknown".to_owned(),
        };
        out.push((name.clone(), word));
    }
    Observation::value(out)
}

fn rmodel_source(input: &(String, Vec<String>)) -> Observation<Verdicts> {
    let path = root().join(format!(
        "evals/experiments/surface-languages/rmodel/{}.model",
        input.0
    ));
    match rmodel::parse(&std::fs::read_to_string(path).unwrap()) {
        Ok(model) => verdicts(&model, &input.1),
        Err(error) => Observation::Error {
            message: error.to_string(),
        },
    }
}

/// An adapter's recorded output: the IR, or `{"unsupported": reason}`.
fn adapter(dir: &str, input: &(String, Vec<String>)) -> Observation<Verdicts> {
    let path = root().join(format!("fixtures/rcheck/adapters/{dir}/{}.json", input.0));
    let text = std::fs::read_to_string(path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    if let Some(reason) = json.get("unsupported") {
        return Observation::Unknown {
            reason: format!("unsupported: {reason}"),
        };
    }
    match rcheck::ir::parse(&text) {
        Ok((model, omitted)) => {
            if let Some(part) = omitted.iter().find(|p| input.1.contains(&p.name)) {
                return Observation::Unknown {
                    reason: format!("omitted {}: {}", part.name, part.reason),
                };
            }
            verdicts(&model, &input.1)
        }
        Err(message) => Observation::Error { message },
    }
}

fn check(dir: &'static str) -> roracle::CheckReport<(String, Vec<String>), Verdicts> {
    let mut group = OracleGroup::new(format!("adapters/{dir}"));
    for (name, observed) in CASES {
        group.case(
            *name,
            (
                name.to_string(),
                observed.iter().map(|s| s.to_string()).collect(),
            ),
        );
    }
    group
        .reference("rmodel", rmodel_source)
        .test_function(dir, move |input| adapter(dir, input));
    group.check().expect("a valid group")
}

fn statuses(
    report: &roracle::CheckReport<(String, Vec<String>), Verdicts>,
) -> Vec<(String, Status)> {
    report
        .cases
        .iter()
        .map(|case| (case.case.id.clone(), case.comparisons[0].status))
        .collect()
}

#[test]
fn the_pkl_adapter_agrees_where_it_can_write_the_model() {
    let report = check("pkl");
    let statuses = statuses(&report);
    for (case, status) in &statuses {
        // order needs data with fields: unsupported, never a match.
        let expected = if case == "order" {
            Status::Unknown
        } else {
            Status::Matched
        };
        assert_eq!(*status, expected, "{case}: {report:#?}");
    }
}

#[test]
fn adapter_faults_are_found() {
    for fault in ["pkl-swap-insert-remove", "pkl-flip-join"] {
        let report = check(fault);
        // A mismatch of verdicts, or an IR the type checker rejects (a join
        // the wrong way round is ill-sorted): never a match.
        let caught: Vec<_> = statuses(&report)
            .into_iter()
            .filter(|(_, status)| matches!(status, Status::Mismatch | Status::Error))
            .collect();
        assert!(!caught.is_empty(), "{fault}: {:?}", statuses(&report));
        println!("{fault}: caught in {caught:?}");
    }
}
