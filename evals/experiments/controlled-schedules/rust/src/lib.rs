use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::thread;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Action {
    actor: String,
    step: usize,
    operation: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    actors: usize,
    actions: Vec<Action>,
    broken: bool,
}

pub fn dispatch(bytes: &[u8]) -> Value {
    if bytes.len() > 1_048_576 {
        return json!({"kind":"error","message":"input exceeds 1 MiB"});
    }
    match serde_json::from_slice::<Input>(bytes) {
        Ok(input) => evaluate(input),
        Err(error) => json!({"kind":"error","message":error.to_string()}),
    }
}
fn evaluate(input: Input) -> Value {
    let invalid =
        || json!({"verdict":{"status":"invalid"},"observations":[],"actual":0,"expected":0});
    if !(1..=8).contains(&input.actors) || input.actions.len() > 128 {
        return invalid();
    }
    let mut positions = vec![0; input.actors];
    let mut order = Vec::new();
    for action in &input.actions {
        let Some(actor) = action
            .actor
            .strip_prefix("actor-")
            .and_then(|name| name.parse::<usize>().ok())
        else {
            return invalid();
        };
        if actor >= input.actors
            || action.actor != format!("actor-{actor}")
            || positions[actor] != action.step
            || action.step > 1
            || action.operation != ["read", "write"][action.step]
        {
            return invalid();
        }
        positions[actor] += 1;
        order.push(actor);
    }
    let counter = Arc::new(AtomicUsize::new(0));
    let (acknowledge, completed) = mpsc::sync_channel(0);
    let mut gates = Vec::new();
    let mut workers = Vec::new();
    for _ in 0..input.actors {
        let (gate, permission) = mpsc::sync_channel(0);
        gates.push(gate);
        let counter = counter.clone();
        let acknowledge = acknowledge.clone();
        let broken = input.broken;
        workers.push(thread::spawn(move || {
            if permission.recv().is_err() {
                return;
            }
            let snapshot = counter.load(Ordering::SeqCst);
            if acknowledge.send(counter.load(Ordering::SeqCst)).is_err()
                || permission.recv().is_err()
            {
                return;
            }
            if broken {
                counter.store(snapshot + 1, Ordering::SeqCst);
            } else {
                counter.fetch_add(1, Ordering::SeqCst);
            }
            let _ = acknowledge.send(counter.load(Ordering::SeqCst));
        }));
    }
    drop(acknowledge);
    let mut observations = Vec::new();
    let mut expected = 0;
    let mut mismatch = false;
    for (actor, action) in order.iter().zip(&input.actions) {
        gates[*actor].send(()).expect("live worker");
        let actual = completed.recv().expect("worker acknowledgement");
        expected += usize::from(action.operation == "write");
        mismatch |= actual != expected;
        observations.push(actual);
    }
    drop(gates);
    for worker in workers {
        worker.join().expect("gated worker");
    }
    json!({
        "verdict": if mismatch { json!({"status":"mismatch","signature":"lost-update"}) } else { json!({"status":"matched"}) },
        "observations": observations, "actual": counter.load(Ordering::SeqCst), "expected": expected,
    })
}
