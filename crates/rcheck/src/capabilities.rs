use rcheck::{Kind, Support};
use serde_json::{Value, json};

fn witness_support(support: &Support) -> Value {
    match support {
        Support::Yes(_) => json!({"status": "supported"}),
        Support::No(reason) => json!({"status": "unsupported", "reason": reason}),
    }
}

pub(super) fn report(model: &rmodel::Model) -> Value {
    let checkers = rcheck::checkers();
    let finite_rprob = match rprob::safety_support(model) {
        Ok(()) => Support::Yes(rcheck::Coverage::Complete),
        Err(error) => Support::No(error.to_string()),
    };
    let questions = rcheck::questions(model)
        .into_iter()
        .map(|question| {
            let mut witness = Value::Null;
            let checkers = checkers
                .iter()
                .map(|checker| {
                    let name = checker.name();
                    let support = if name == "rprob" && question.kind != Kind::Temporal {
                        finite_rprob.clone()
                    } else {
                        checker.supports(model, &question)
                    };
                    if name == "rprob" {
                        witness = witness_support(&support);
                    }
                    match support {
                        Support::Yes(coverage) => {
                            json!({"checker": name, "status": "supported", "coverage": coverage})
                        }
                        Support::No(reason) => {
                            json!({"checker": name, "status": "unsupported", "reason": reason})
                        }
                    }
                })
                .collect::<Vec<_>>();
            json!({"question": question, "checkers": checkers, "rprobWitness": witness})
        })
        .collect::<Vec<_>>();
    json!({
        "questions": questions,
        "scopeWitness": {
            "question": rmodel::SCOPE,
            "reference": {"status": "supported"},
            "rprob": witness_support(&finite_rprob)
        }
    })
}
