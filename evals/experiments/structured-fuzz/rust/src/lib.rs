use arbitrary::Unstructured;
use idealis_connect::{Case, Effect, Engine, TestStatus, Verdict};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Raw,
    Structured,
}
#[derive(Debug, Serialize)]
pub struct Exchange {
    pub request: Vec<u8>,
    pub response: Value,
}
#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub commands: u32,
    pub engine_calls: u32,
    pub errors: u32,
    pub bounded: bool,
    pub phases: BTreeMap<String, u32>,
    pub results: BTreeMap<String, u32>,
    pub trace: Vec<Exchange>,
}
struct Audit {
    subject: Engine,
    reference: Engine,
    accepted: Vec<Vec<u8>>,
    pending: Option<(u32, Case)>,
    last_case: Option<Case>,
    report: Report,
    capture: bool,
}
impl Audit {
    fn new(capture: bool) -> Self {
        Self {
            subject: Engine::default(),
            reference: Engine::default(),
            accepted: vec![],
            pending: None,
            last_case: None,
            report: Report::default(),
            capture,
        }
    }
    fn apply(&mut self, bytes: Vec<u8>) {
        if self.report.bounded {
            return;
        }
        if self.report.engine_calls + 2 > 256 {
            self.report.bounded = true;
            return;
        }
        self.report.engine_calls += 2;
        let response = self.subject.dispatch(&bytes);
        let expected = self.reference.dispatch(&bytes);
        assert_eq!(response, expected, "protocol state differs");
        if response["kind"] == "error" {
            serde_json::from_value::<idealis_connect::Error>(response["error"].clone())
                .expect("named protocol/resource error");
            self.report.errors += 1;
            self.reference = Engine::default();
            for input in &self.accepted {
                if self.report.engine_calls == 256 {
                    self.report.bounded = true;
                    break;
                }
                self.report.engine_calls += 1;
                assert_ne!(
                    self.reference.dispatch(input)["kind"],
                    "error",
                    "accepted prefix is replayable"
                );
            }
        } else if response["kind"] == "effect" {
            let request: Value = serde_json::from_slice(&bytes).expect("accepted JSON");
            if request["request"]["kind"] != "resume" {
                self.accepted.clear();
            }
            self.accepted.push(bytes.clone());
            match serde_json::from_value::<Effect>(response["effect"].clone())
                .expect("typed effect")
            {
                Effect::Evaluate { id, phase, case } => {
                    let label = serde_json::to_value(phase)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned();
                    *self.report.phases.entry(label).or_default() += 1;
                    self.last_case = Some(case.clone());
                    self.pending = Some((id, case));
                }
                Effect::Done { result } => {
                    if result.status == TestStatus::Passed {
                        assert!(result.cases > 0, "vacuous success");
                    }
                    if result.status == TestStatus::Failed {
                        let case = result.counterexample.as_ref().expect("failed needs case");
                        let signature = result.signature.as_ref().expect("failed needs signature");
                        let last = result.trials.last().expect("failed needs final trial");
                        assert_eq!(&last.case, case);
                        assert!(
                            matches!(&last.verdict, Verdict::Mismatch { signature: final_signature } if final_signature == signature)
                        );
                    }
                    assert!(
                        result
                            .trials
                            .windows(2)
                            .all(|trials| trials[0].id < trials[1].id)
                    );
                    let label = serde_json::to_value(result.status)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned();
                    *self.report.results.entry(label).or_default() += 1;
                    self.pending = None;
                }
            }
        } else if response["kind"] == "cancelled" {
            self.accepted.clear();
            self.pending = None;
        } else if response["kind"] == "comparison" {
            for comparison in response["result"]["comparisons"]
                .as_array()
                .expect("comparisons")
            {
                if comparison["status"] == "matched" {
                    assert!(
                        comparison["expected"]
                            .as_array()
                            .expect("allowed values")
                            .contains(&comparison["actual"])
                    );
                }
            }
        } else {
            assert_eq!(response["kind"], "generator");
        }
        if self.capture {
            self.report.trace.push(Exchange {
                request: bytes,
                response,
            });
        }
    }
    fn request(&mut self, request: Value) {
        self.apply(serde_json::to_vec(&json!({"version":1,"request":request})).unwrap());
    }
    fn feedback(&mut self, choice: u8) {
        let id = self.pending.as_ref().map_or(0, |pending| pending.0);
        let verdict = match choice % 7 {
            0 => json!({"status":"matched"}),
            1 => json!({"status":"mismatch","signature":"contains-bad"}),
            2 => json!({"status":"invalid"}),
            3 => json!({"status":"unknown","reason":"fuzz"}),
            4 => json!({"status":"error","message":"fuzz"}),
            5 => json!({"status":"mismatch","signature":"other"}),
            _ => json!({"status":"matched","extra":true}),
        };
        self.request(json!({"kind":"resume","id":id,"verdict":verdict}));
    }
    fn drain(&mut self) {
        for _ in 0..32 {
            if self.report.bounded {
                break;
            }
            let Some((_, case)) = self.pending.as_ref() else {
                break;
            };
            let broken = case.actions.iter().any(|action| action == "bad");
            self.feedback(u8::from(broken));
        }
        if self.pending.is_some() {
            self.report.bounded = true;
        }
    }
}
fn generator(selector: u8) -> Value {
    let actions = if selector & 1 == 1 {
        vec![json!("bad")]
    } else {
        vec![json!("good"), json!("bad"), json!({"nested":[true,null,1]})]
    };
    json!({"id":"g","version":"1","initial":["s"],"states":{"s":actions.into_iter().map(|action| json!({"action":action,"target":"s"})).collect::<Vec<_>>()},"minLength":0,"maxLength":8})
}
fn fallback() -> Value {
    json!({"format":"idealis.test-case","version":1,"rng":"xorshift32-v1","generatorId":"g","generatorVersion":"1","seed":1,"index":0,"actions":["bad"]})
}
pub fn exercise(data: &[u8], mode: Mode, capture: bool) -> Report {
    let mut audit = Audit::new(capture);
    if data.len() > 8192 {
        audit.report.bounded = true;
        return audit.report;
    }
    match mode {
        Mode::Raw => {
            for (index, line) in data.split(|byte| *byte == b'\n').enumerate() {
                if index == 64 {
                    audit.report.bounded = true;
                    break;
                }
                if audit.report.bounded {
                    break;
                }
                audit.report.commands += 1;
                audit.apply(line.to_vec());
            }
        }
        Mode::Structured => {
            let mut input = Unstructured::new(data);
            let selector = input.arbitrary::<u8>().unwrap_or(0);
            let base = generator(selector);
            for _ in 0..64 {
                if input.is_empty() || audit.report.bounded {
                    break;
                }
                audit.report.commands += 1;
                let opcode = input.arbitrary::<u8>().unwrap_or(0) % 17;
                match opcode {
                    0 => {
                        let cases = u32::from(input.arbitrary::<u8>().unwrap_or(0) % 4) + 1;
                        let length = input.arbitrary::<u8>().unwrap_or(4) % 9;
                        let work = if input.arbitrary::<u8>().unwrap_or(1) == 0 {
                            0
                        } else {
                            4096
                        };
                        let mut generator = base.clone();
                        generator["maxLength"] = json!(length);
                        if selector & 1 == 1 {
                            generator["minLength"] = json!(length.min(1));
                        }
                        audit.request(json!({"kind":"property","plan":{"generator":generator,"seed":u32::from(selector)+1,"cases":cases,"maxAttempts":cases,"maxShrinkChecks":16,"maxWork":work}}));
                    }
                    1..=7 => audit.feedback(opcode - 1),
                    8 => audit.request(json!({"kind":"cancel"})),
                    9 | 10 => {
                        let case = audit
                            .last_case
                            .as_ref()
                            .map_or_else(fallback, |case| serde_json::to_value(case).unwrap());
                        if opcode == 9 {
                            audit.request(
                                json!({"kind":"replay","case":case,"expectedSignature":null}),
                            );
                        } else {
                            audit.request(json!({"kind":"shrink","case":case,"generator":base,"maxChecks":16,"maxWork":4096}));
                        }
                    }
                    11 => {
                        let expected = input.arbitrary::<i8>().unwrap_or(0);
                        let actual = input.arbitrary::<i8>().unwrap_or(0);
                        audit.request(json!({"kind":"compare","oracle":{"kind":"allowed","values":[expected]},"targets":[{"id":"sut","outcome":{"kind":"value","value":actual}}]}));
                    }
                    12 => {
                        let value = input.arbitrary::<i8>().unwrap_or(0);
                        audit.request(json!({"kind":"schedule","schedule":{"id":"schedule","version":"1","actors":[{"id":"left","operations":[value,{"nested":[true,null]}]},{"id":"right","operations":["op"]}]}}));
                    }
                    13 => {
                        let id = audit.pending.as_ref().map_or(0, |pending| pending.0);
                        audit.request(json!({"kind":"resume","id":id,"verdict":{"status":"matched","extra":true}}));
                    }
                    14 => {
                        let length =
                            usize::from(input.arbitrary::<u8>().unwrap_or(0)).min(input.len());
                        if let Ok(bytes) = input.bytes(length) {
                            audit.apply(bytes.to_vec());
                        }
                    }
                    15 => audit.drain(),
                    _ => {
                        let low = u16::from(input.arbitrary::<u8>().unwrap_or(0));
                        let high = u16::from(input.arbitrary::<u8>().unwrap_or(0));
                        let length = usize::from(low | (high << 8)).min(input.len()).min(4096);
                        if let Ok(bytes) = input.bytes(length) {
                            match serde_json::from_slice::<Value>(bytes) {
                                Ok(case) => audit.request(
                                    json!({"kind":"replay","case":case,"expectedSignature":null}),
                                ),
                                Err(_) => audit.apply(bytes.to_vec()),
                            }
                        }
                    }
                }
            }
            if !input.is_empty() {
                audit.report.bounded = true;
            }
        }
    }
    audit.drain();
    audit.report
}
pub fn known_fault() {
    let mut audit = Audit::new(false);
    audit.request(json!({"kind":"replay","case":fallback(),"expectedSignature":null}));
    audit.request(json!({"kind":"resume","id":1,"verdict":{"status":"matched","extra":true}}));
    audit
        .subject
        .dispatch(br#"{"version":1,"request":{"kind":"cancel"}}"#);
    audit.feedback(1);
}
