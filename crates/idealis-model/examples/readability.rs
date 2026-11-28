//! readability <first-seed> <count> <batch-size> <output-dir>: picks the
//! first `count` generated models from `first-seed` on whose every verdict
//! is decided and not empty, prints them in the v0 Quint-like syntax, and
//! writes reader prompts (batch-<n>.md) and the answer key (key.json) from
//! the reference evaluator (plan 0098). The prompts carry no description of
//! the syntax.
#[path = "support/generator.rs"]
mod generator;

use idealis_model::reference::Outcome;
use std::fmt::Write as _;

const INSTRUCTIONS: &str = "You are reading models written in a specification language you may not have seen. Read each model and answer every question by reasoning about the text alone; do not run tools or look anything up. Reply with one JSON object that maps each question id to exactly one of the options listed for it, and nothing else.\n";

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [first, count, batch, output] = arguments.as_slice() else {
        panic!("usage: readability <first-seed> <count> <batch-size> <output-dir>");
    };
    let (first, count, batch): (u64, usize, usize) = (
        first.parse().unwrap(),
        count.parse().unwrap(),
        batch.parse().unwrap(),
    );
    std::fs::create_dir_all(output).unwrap();
    let mut picked = Vec::new();
    let mut seed = first;
    while picked.len() < count {
        let source = generator::generate(seed);
        let model = idealis_model::parse(&source).unwrap();
        let report = idealis_model::reference::analyze(&model, 200_000);
        let decided = report
            .verdicts
            .iter()
            .all(|v| !matches!(v.outcome, Outcome::Empty | Outcome::Unknown))
            && report.enabled.iter().all(|(_, enabled)| enabled.is_some())
            && matches!(
                report.declarations,
                Outcome::Holds | Outcome::Violated { .. }
            );
        if decided {
            picked.push((seed, model, report));
        }
        seed += 1;
    }
    let mut key = String::from("{");
    let mut entries = Vec::new();
    for (index, chunk) in picked.chunks(batch).enumerate() {
        let mut prompt = String::from(INSTRUCTIONS);
        for (seed, model, report) in chunk {
            let _ = write!(
                prompt,
                "\n## Model {seed}\n\n```\n{}```\n\nQuestions:\n",
                idealis_model::quint_like(model)
            );
            for (check, verdict) in model.checks.iter().zip(&report.verdicts) {
                let (question, options, answer) = match check.kind {
                    idealis_model::CheckKind::Invariant => (
                        format!(
                            "Does invariant {} hold in every reachable state, for every allowed value of the constants?",
                            check.name
                        ),
                        "holds | violated | empty (no initial state exists)",
                        if matches!(verdict.outcome, Outcome::Holds) {
                            "holds"
                        } else {
                            "violated"
                        },
                    ),
                    idealis_model::CheckKind::Reachable => (
                        format!(
                            "Is a state satisfying {} reachable, for some allowed value of the constants?",
                            check.name
                        ),
                        "reachable | unreachable | empty (no initial state exists)",
                        if matches!(verdict.outcome, Outcome::Reachable { .. }) {
                            "reachable"
                        } else {
                            "unreachable"
                        },
                    ),
                };
                let id = format!("{seed}.{}", check.name);
                let _ = writeln!(prompt, "- `{id}`: {question} Options: {options}");
                entries.push(format!("{id:?}:{answer:?}"));
            }
            // Plan 0099: a step that breaks a declared multiplicity is a
            // reported violation.
            let id = format!("{seed}.declarations");
            let _ = writeln!(
                prompt,
                "- `{id}`: Can some step reach a state that breaks one of the declared limits noted in the comments (at most one, exactly one, at least one)? Options: yes | no"
            );
            let answer = if matches!(report.declarations, Outcome::Violated { .. }) {
                "yes"
            } else {
                "no"
            };
            entries.push(format!("{id:?}:{answer:?}"));
            for (action, enabled) in &report.enabled {
                let id = format!("{seed}.action.{action}");
                let _ = writeln!(
                    prompt,
                    "- `{id}`: Can action {action} take a step from some reachable state? Options: yes | no"
                );
                let answer = if enabled.unwrap() { "yes" } else { "no" };
                entries.push(format!("{id:?}:{answer:?}"));
            }
        }
        std::fs::write(format!("{output}/batch-{index}.md"), prompt).unwrap();
    }
    key.push_str(&entries.join(","));
    key.push_str("}\n");
    std::fs::write(format!("{output}/key.json"), key).unwrap();
    let seeds: Vec<String> = picked.iter().map(|(seed, _, _)| seed.to_string()).collect();
    println!("seeds {}", seeds.join(","));
}
