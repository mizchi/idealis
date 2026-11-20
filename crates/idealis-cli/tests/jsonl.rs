use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn compares_observations_after_an_invalid_request() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_idealis-cli"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let request = json!({"version":1, "request":{
        "kind":"compare", "oracle":{"kind":"allowed","values":[1,2]},
        "targets":[
            {"id":"reference-consumer","outcome":{"kind":"value","value":2}},
            {"id":"faulty-consumer","outcome":{"kind":"value","value":3}}
        ]
    }});
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, "invalid JSON\n{request}").unwrap();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let replies: Vec<Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[0]["kind"], "error");
    assert_eq!(replies[1]["kind"], "comparison");
    let result = &replies[1]["result"];
    assert_eq!(result["status"], "mismatch");
    assert_eq!(result["comparisons"][0]["status"], "matched");
    assert_eq!(result["comparisons"][1]["status"], "mismatch");
}
