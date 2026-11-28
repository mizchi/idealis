//! File protocol for external checkers and replayable evidence.
use crate::{Budget, Coverage, Kind, Question, ir, questions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const VERSION: u32 = 1;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    version: u32,
    request_id: String,
    ir: ir::Ir,
    question: Question,
    budget: Budget,
}

impl Request {
    fn identity(&self) -> String {
        let model = serde_json::to_vec(&(self.version, &self.ir, &self.question, self.budget))
            .expect("serializable checker request");
        format!("{:x}", Sha256::digest(model))
    }
}

pub fn request(
    model: &idealis_model::Model,
    name: &str,
    budget: Budget,
) -> Result<Request, String> {
    let question = select_question(model, name)?;
    let mut request = Request {
        version: VERSION,
        request_id: String::new(),
        ir: ir::to_ir(model)?,
        question,
        budget,
    };
    request.request_id = request.identity();
    Ok(request)
}

fn select_question(model: &idealis_model::Model, name: &str) -> Result<Question, String> {
    questions(model)
        .into_iter()
        .find(|question| question.name == name)
        .ok_or_else(|| format!("no question named {name}"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reply {
    version: u32,
    request_id: String,
    verdict: Claim,
    coverage: Option<Coverage>,
    reason: Option<String>,
    witness: Option<Witness>,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Claim {
    Reachable,
    Violated,
    Holds,
    Unreachable,
    Empty,
    Unknown,
    Unsupported,
    Error,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Witness {
    pub config: Value,
    pub initial: Value,
    pub commands: Vec<Value>,
    pub final_state: Value,
    pub loop_start: Option<usize>,
}

/// Verify a reply's identity and supplied path or lasso. Preserve claims without evidence.
pub fn verify(request: Request, reply: Reply) -> Result<Value, String> {
    if request.version != VERSION || reply.version != VERSION {
        return Err(format!(
            "external checker protocol version must be {VERSION}"
        ));
    }
    if request.request_id != request.identity() {
        return Err("requestId does not match the request content".into());
    }
    if reply.request_id != request.request_id {
        return Err("reply requestId does not match the request".into());
    }
    if !request.ir.omitted.is_empty() {
        return Err("external checker verification requires a complete IR".into());
    }
    let model = ir::from_ir(&request.ir)?;
    let question = select_question(&model, &request.question.name)?;
    if question != request.question {
        return Err("request question kind does not match the model".into());
    }
    match reply.verdict {
        Claim::Reachable | Claim::Unreachable if question.kind != Kind::Reachable => {
            return Err("reply verdict does not fit the question".into());
        }
        Claim::Violated | Claim::Holds if question.kind == Kind::Reachable => {
            return Err("reply verdict does not fit the question".into());
        }
        _ => {}
    }
    let coverage = reply.coverage;
    if matches!(reply.verdict, Claim::Unsupported | Claim::Error) {
        if coverage.is_some() {
            return Err("unsupported or failed replies must omit coverage".into());
        }
    } else if coverage.is_none() || matches!(coverage, Some(Coverage::Bounded { steps: 0 })) {
        return Err("reply requires a valid coverage declaration".into());
    }
    let mut report =
        json!({"status":"reported", "verdict":reply.verdict, "question":question.name});
    if let Some(coverage) = coverage {
        report["coverage"] = json!(coverage);
    }
    if let Some(reason) = reply.reason {
        report["reason"] = json!(reason);
    }
    match (reply.verdict, reply.witness) {
        (Claim::Reachable | Claim::Violated, Some(witness)) => {
            let depth = witness.commands.len();
            let temporal = question.kind == Kind::Temporal;
            if let Some(Coverage::Bounded { steps }) = coverage
                && (depth > steps || !temporal && depth == steps)
            {
                return Err("witness lies outside declared coverage".into());
            }
            if temporal {
                use idealis_model::reference::LassoError;
                let loop_start = witness
                    .loop_start
                    .ok_or("temporal witness requires loopStart")?;
                match super::validation::verify_lasso(
                    &model,
                    &question,
                    &witness,
                    loop_start,
                    request.budget.work,
                ) {
                    Ok(()) => report["loopStart"] = json!(loop_start),
                    Err(LassoError::Invalid(reason)) => return Err(reason),
                    Err(LassoError::Unverified(reason)) => {
                        report["verificationReason"] = json!(reason);
                        return Ok(report);
                    }
                }
            } else {
                if witness.loop_start.is_some() {
                    return Err("finite witnesses do not have loopStart".into());
                }
                super::validation::verify_witness(model, &question, &witness)?;
            }
            report["status"] = json!("verified");
            report["depth"] = json!(depth);
        }
        (Claim::Reachable | Claim::Violated, None) => return Err("reply requires a witness".into()),
        (_, Some(_)) => {
            return Err("only reachable or violated replies can provide a witness".into());
        }
        _ => {}
    }
    Ok(report)
}
