//! induction <model>...: inductive invariant checking of rmodel models
//! with z3, a prototype (plan 0106). For each invariant and Declarations it
//! prints whether every obligation is unsat (inductive), and for a sat one
//! the smallest concrete counterexample, re-checked by the reference
//! evaluator. `--generated <first> <last>` runs the differential test's
//! generated models instead and compares with the reference verdicts.
mod recheck;
mod smt;
mod vc;

#[path = "../../../../crates/rmodel/examples/support/generator.rs"]
mod generator;

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
enum Verdict {
    /// Every obligation of every check is unsat: the conjunction of the
    /// invariants and Declarations is inductive.
    Inductive,
    /// This check's obligations are unsat, but they assume the other
    /// checks, and some of those are not inductive (found by seed 9130).
    Relative,
    NotInductive {
        step: String,
        confirmed: Result<(), String>,
    },
    Unknown(String),
}

fn check(model: &rmodel::Model) -> BTreeMap<String, Verdict> {
    let mut verdicts: BTreeMap<String, Verdict> = BTreeMap::new();
    for obligation in vc::obligations(model) {
        if verdicts
            .get(&obligation.check)
            .is_some_and(|v| *v != Verdict::Inductive)
        {
            continue;
        }
        let (answer, _) = smt::run(&obligation.smt, 20);
        let verdict = match answer {
            smt::Answer::Unsat => Verdict::Inductive,
            smt::Answer::Unknown(reason) => Verdict::Unknown(reason),
            smt::Answer::Sat => {
                let params = model
                    .actions
                    .iter()
                    .find(|a| a.name == obligation.step)
                    .map(|a| a.params.clone())
                    .unwrap_or_default();
                let confirmed = match smt::concrete(model, &obligation.smt, &params, 3) {
                    Some(cti) => {
                        recheck::confirms(model, &cti, &obligation.check, &obligation.step)
                    }
                    None => Err("no counterexample within 3 atoms per sort".into()),
                };
                Verdict::NotInductive {
                    step: obligation.step.clone(),
                    confirmed,
                }
            }
        };
        verdicts.insert(obligation.check.clone(), verdict);
    }
    if verdicts.values().any(|v| *v != Verdict::Inductive) {
        for verdict in verdicts.values_mut() {
            if *verdict == Verdict::Inductive {
                *verdict = Verdict::Relative;
            }
        }
    }
    verdicts
}

fn word(outcome: rmodel::reference::Outcome) -> &'static str {
    use rmodel::reference::Outcome;
    match outcome {
        Outcome::Empty => "empty",
        Outcome::Holds => "holds",
        Outcome::Violated { .. } => "violated",
        Outcome::Reachable { .. } => "reachable",
        Outcome::Unreachable => "unreachable",
        Outcome::Unknown => "unknown",
    }
}

/// One JSON line per invariant: the induction verdict and the reference's.
fn compare(label: &str, model: &rmodel::Model) -> Vec<String> {
    let reference = rmodel::reference::analyze(model, 200_000);
    let mut lines = Vec::new();
    for (name, verdict) in check(model) {
        let reference = if name == rmodel::DECLARATIONS {
            reference.declarations
        } else {
            reference
                .verdicts
                .iter()
                .find(|v| v.check == name)
                .unwrap()
                .outcome
        };
        let (induction, step, confirmed) = match &verdict {
            Verdict::Inductive => ("inductive", String::new(), String::new()),
            Verdict::Relative => ("relative", String::new(), String::new()),
            Verdict::NotInductive { step, confirmed } => (
                "not-inductive",
                step.clone(),
                match confirmed {
                    Ok(()) => "confirmed".into(),
                    Err(reason) => format!("unconfirmed: {}", reason.lines().next().unwrap_or("")),
                },
            ),
            Verdict::Unknown(reason) => ("unknown", String::new(), reason.clone()),
        };
        // Soundness against the reference within its scope.
        let contradiction = (induction == "inductive" && word(reference) == "violated")
            || (induction == "not-inductive" && !confirmed.starts_with("confirmed"));
        lines.push(format!(
            "{{\"model\":{label:?},\"check\":{name:?},\"induction\":{induction:?},\"step\":{step:?},\"recheck\":{confirmed:?},\"reference\":{:?},\"contradiction\":{contradiction}}}",
            word(reference)
        ));
    }
    lines
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, seed] = arguments.as_slice()
        && flag == "--source"
    {
        print!("{}", generator::generate(seed.parse().unwrap()));
        return;
    }
    if let [flag, first, last] = arguments.as_slice()
        && flag == "--generated"
    {
        let (first, last): (u64, u64) = (first.parse().unwrap(), last.parse().unwrap());
        for seed in first..=last {
            let source = generator::generate(seed);
            let Ok(model) = rmodel::parse(&source) else {
                continue;
            };
            if let Err(reason) = vc::supported(&model) {
                println!("{{\"model\":\"{seed}\",\"unsupported\":{reason:?}}}");
                continue;
            }
            for line in compare(&seed.to_string(), &model) {
                println!("{line}");
            }
        }
        return;
    }
    for path in arguments {
        let source = std::fs::read_to_string(&path).unwrap();
        let model = rmodel::parse(&source).unwrap();
        if let Err(reason) = vc::supported(&model) {
            println!("{{\"model\":{path:?},\"unsupported\":{reason:?}}}");
            continue;
        }
        for line in compare(&path, &model) {
            println!("{line}");
        }
    }
}
