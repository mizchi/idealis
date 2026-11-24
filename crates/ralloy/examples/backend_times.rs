//! Times both backends on every corpus command the SAT backend covers
//! (plan 0087). Prints one JSON object per command.
#[path = "../tests/alloy/oracle.rs"]
mod oracle;

use ralloy::{Model, Status};
use serde_json::json;
use std::time::Instant;

const BUDGET: u64 = 50_000_000;

/// Median nanoseconds per run and the raw samples, or `None` when the
/// backend exhausts the budget.
fn time(model: &Model, index: usize, sat: bool) -> (Option<f64>, Status, Vec<(u64, u128)>) {
    let run = || model.analyze_backend(index, BUDGET, sat).unwrap().unwrap();
    let first = Instant::now();
    let status = run().status;
    let once = first.elapsed().as_nanos().max(1);
    if status == Status::Unknown {
        return (None, status, vec![(1, once)]);
    }
    for _ in 0..3 {
        run();
    }
    let (samples, iterations) = if once > 200_000_000 {
        (3, 1)
    } else {
        (15, (5_000_000 / once).max(1) as u64)
    };
    let mut raw = Vec::new();
    for _ in 0..samples {
        let start = Instant::now();
        for _ in 0..iterations {
            assert_eq!(run().status, status);
        }
        raw.push((iterations, start.elapsed().as_nanos()));
    }
    let mut per: Vec<f64> = raw.iter().map(|(n, ns)| *ns as f64 / *n as f64).collect();
    per.sort_by(f64::total_cmp);
    let middle = per.len() / 2;
    let median = if per.len() % 2 == 1 {
        per[middle]
    } else {
        (per[middle - 1] + per[middle]) / 2.0
    };
    (Some(median), status, raw)
}

fn main() {
    for group in ["core", "nary", "hierarchy", "predicates", "integers"] {
        for (id, input) in oracle::models(group) {
            let model = Model::parse(&input.source).unwrap();
            for (index, command) in model.commands().iter().enumerate() {
                if model.analyze_backend(index, 1, true).unwrap().is_none() {
                    continue;
                }
                let (enumerated, enumerated_status, enumerated_raw) = time(&model, index, false);
                let (sat, sat_status, sat_raw) = time(&model, index, true);
                assert!(
                    enumerated_status == Status::Unknown || enumerated_status == sat_status,
                    "{group}/{id} {}",
                    command.label
                );
                println!(
                    "{}",
                    json!({
                        "group": group,
                        "model": id,
                        "command": command.label,
                        "width": model.search_width(index).unwrap(),
                        "status": format!("{sat_status:?}"),
                        "enumerateNs": enumerated,
                        "satNs": sat,
                        "enumerateRaw": enumerated_raw,
                        "satRaw": sat_raw,
                    })
                );
            }
        }
    }
}
