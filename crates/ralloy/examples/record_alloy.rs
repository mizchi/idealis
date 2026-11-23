//! Records Alloy's answers for a corpus group as a roracle report.
//!
//! cargo run --example record_alloy -- <group> <java> <alloy.dist.jar>
//!
//! Each model runs once under `java -jar <jar> exec` with SAT4J. The report
//! keeps ralloy's observation at recording time too, and is written to
//! `tests/alloy/<group>.json` next to a provenance file. The replay test
//! reruns only ralloy against the saved Alloy answers.
#[path = "../tests/alloy/oracle.rs"]
mod oracle;

use oracle::{AlloyModel, Outcomes};
use roracle::{Observation, OracleGroup};
use serde_json::{Value, json};
use std::process::Command;

fn alloy(java: &str, jar: &str, model: &AlloyModel) -> Observation<Outcomes> {
    let directory = std::env::temp_dir().join(format!(
        "ralloy-oracle-{}-{}",
        std::process::id(),
        sha256(model.source.as_bytes())
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("model.als");
    std::fs::write(&path, &model.source).unwrap();
    let output = Command::new(java)
        .args([
            "-jar",
            jar,
            "exec",
            "--quiet",
            "--command",
            "*",
            "--solver",
            "sat4j",
        ])
        .args(["--type", "json", "--output"])
        .arg(directory.join("output"))
        .arg(&path)
        .output()
        .unwrap();
    if !output.status.success() {
        return Observation::Error {
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        };
    }
    let receipt: Value =
        serde_json::from_slice(&std::fs::read(directory.join("output/receipt.json")).unwrap())
            .unwrap();
    let Some(commands) = receipt["commands"].as_object() else {
        return Observation::Error {
            message: "receipt without commands".into(),
        };
    };
    Observation::value(
        commands
            .iter()
            .map(|(name, command)| {
                let solved = command["solution"]
                    .as_array()
                    .is_some_and(|solutions| !solutions.is_empty());
                (name.clone(), solved)
            })
            .collect(),
    )
}

fn sha256(bytes: &[u8]) -> String {
    let output = Command::new("shasum")
        .args(["-a", "256"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(bytes)?;
            child.wait_with_output()
        })
        .unwrap();
    String::from_utf8(output.stdout).unwrap()[..64].to_string()
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [group, java, jar] = arguments.as_slice() else {
        panic!("usage: record_alloy <group> <java> <alloy.dist.jar>");
    };
    let models = oracle::models(group);
    let mut oracle_group = OracleGroup::new(group.as_str());
    for (id, model) in &models {
        oracle_group.case(id.as_str(), model.clone());
    }
    oracle_group
        .reference("alloy-6.2.0", |model| alloy(java, jar, model))
        .test_function("ralloy", oracle::ralloy);
    let report = oracle_group.check().unwrap();
    let root = oracle::root();
    std::fs::write(
        root.join(format!("{group}.json")),
        serde_json::to_string_pretty(&report).unwrap() + "\n",
    )
    .unwrap();
    let java_version = Command::new(java).arg("-version").output().unwrap();
    let provenance = json!({
        "group": group,
        "reference": "alloy-6.2.0",
        "jar": std::path::Path::new(jar).file_name().map(|name| name.to_string_lossy().into_owned()),
        "jarSha256": sha256(&std::fs::read(jar).unwrap()),
        "java": String::from_utf8_lossy(&java_version.stderr).lines().next(),
        "solver": "sat4j",
        "models": models.iter().map(|(id, model)| json!({"id": id, "sha256": sha256(model.source.as_bytes())})).collect::<Vec<_>>(),
    });
    std::fs::write(
        root.join(format!("{group}.provenance.json")),
        serde_json::to_string_pretty(&provenance).unwrap() + "\n",
    )
    .unwrap();
    println!("{group}: {:?}", report.status);
    for case in &report.cases {
        for comparison in &case.comparisons {
            if comparison.status != roracle::Status::Matched {
                println!(
                    "  {}: {:?} reference={:?} actual={:?}",
                    case.case.id, comparison.status, case.reference, comparison.actual
                );
            }
        }
    }
}
