use crate::{
    Case, Error, Generator, Observation, Plan, Schedule, Session, Target, Verdict, compare,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u32,
    request: Request,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Request {
    Schedule {
        schedule: Schedule,
    },
    Property {
        plan: Plan,
    },
    Shrink {
        generator: Generator,
        case: Case,
        #[serde(rename = "maxChecks")]
        max_checks: u32,
        #[serde(rename = "maxWork")]
        max_work: u32,
    },
    Replay {
        case: Case,
        #[serde(rename = "expectedSignature")]
        expected_signature: Option<String>,
    },
    Resume {
        id: u32,
        verdict: Verdict,
    },
    Cancel {},
    Compare {
        oracle: Observation,
        targets: Vec<Target>,
    },
}
#[derive(Default)]
pub struct Engine {
    session: Option<Session>,
}
impl Engine {
    pub fn dispatch(&mut self, input: &[u8]) -> Value {
        match self.execute(input) {
            Ok(value) => value,
            Err(error) => json!({"kind":"error","error":error}),
        }
    }
    fn execute(&mut self, input: &[u8]) -> Result<Value, Error> {
        if input.len() > 1_048_576 {
            return Err(Error::resource("request exceeds 1 MiB"));
        }
        let envelope: Envelope =
            serde_json::from_slice(input).map_err(|error| Error::protocol(error.to_string()))?;
        if envelope.version != 1 {
            return Err(Error::protocol("unsupported protocol version"));
        }
        match envelope.request {
            Request::Schedule { schedule } => {
                Ok(json!({"kind":"generator","generator":schedule.generator()?}))
            }
            Request::Compare { oracle, targets } => {
                Ok(json!({"kind":"comparison","result":compare(oracle, targets)?}))
            }
            Request::Cancel {} => {
                self.session = None;
                Ok(json!({"kind":"cancelled"}))
            }
            Request::Resume { id, verdict } => {
                let session = self
                    .session
                    .as_mut()
                    .ok_or_else(|| Error::protocol("no active session"))?;
                session.resume(id, verdict)?;
                Ok(json!({"kind":"effect","effect":session.effect()}))
            }
            request => {
                if self
                    .session
                    .as_ref()
                    .is_some_and(|session| !session.is_done())
                {
                    return Err(Error::protocol("a session is already active"));
                }
                let session = match request {
                    Request::Property { plan } => Session::property(plan)?,
                    Request::Shrink {
                        generator,
                        case,
                        max_checks,
                        max_work,
                    } => Session::shrink(generator, case, max_checks, max_work)?,
                    Request::Replay {
                        case,
                        expected_signature,
                    } => Session::replay(case, expected_signature)?,
                    _ => unreachable!("handled request"),
                };
                let result = json!({"kind":"effect","effect":session.effect()});
                self.session = Some(session);
                Ok(result)
            }
        }
    }
}
