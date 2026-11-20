use loom::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use loom::thread;
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::Mutex;

fn main() {
    let mut results = Vec::new();
    for broken in [false, true] {
        let observations = std::sync::Arc::new(Mutex::new((0_usize, BTreeSet::new())));
        let collected = observations.clone();
        let mut builder = loom::model::Builder::new();
        builder.max_threads = 3;
        builder.max_branches = 1000;
        builder.max_permutations = None;
        builder.max_duration = None;
        builder.preemption_bound = None;
        builder.checkpoint_file = None;
        builder.expect_explicit_explore = false;
        builder.check(move || {
            let counter = Arc::new(AtomicUsize::new(0));
            let workers: Vec<_> = (0..2)
                .map(|_| {
                    let counter = counter.clone();
                    thread::spawn(move || {
                        let snapshot = counter.load(Ordering::SeqCst);
                        if broken {
                            counter.store(snapshot + 1, Ordering::SeqCst);
                        } else {
                            counter.fetch_add(1, Ordering::SeqCst);
                        }
                    })
                })
                .collect();
            for worker in workers {
                worker.join().unwrap();
            }
            let mut collected = collected.lock().unwrap();
            collected.0 += 1;
            collected.1.insert(counter.load(Ordering::SeqCst));
        });
        let collected = observations.lock().unwrap();
        let expected = if broken {
            BTreeSet::from([1, 2])
        } else {
            BTreeSet::from([2])
        };
        assert_eq!(collected.1, expected);
        results.push(json!({"broken":broken,"executions":collected.0,"outcomes":collected.1}));
    }
    println!(
        "{}",
        json!({"candidate":"loom-0.7.2","actors":2,"maxThreads":3,"maxBranches":1000,"maxPermutations":null,"maxDuration":null,"preemptionBound":null,"ordering":"SeqCst, with Loom's documented limitations","results":results})
    );
}
