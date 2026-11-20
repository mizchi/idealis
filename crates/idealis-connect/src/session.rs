use crate::generator::Random;
use crate::shrink::Shrinker;
use crate::{Case, Error, Generator, Work, text};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TestStatus {
    Passed,
    Failed,
    Exhausted,
    Inconclusive,
    Error,
    NotFailing,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Generation,
    Baseline,
    Shrink,
    Confirmation,
    Replay,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShrinkStatus {
    Minimal,
    Bounded,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Verdict {
    Matched {},
    Mismatch { signature: String },
    Invalid {},
    Unknown { reason: String },
    Error { message: String },
}
impl Verdict {
    fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Mismatch { signature } => text(signature),
            Self::Unknown { reason } => text(reason),
            Self::Error { message } => text(message),
            _ => Ok(()),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub generator: Generator,
    pub seed: u32,
    pub cases: u32,
    pub max_attempts: u32,
    pub max_shrink_checks: u32,
    pub max_work: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Trial {
    pub id: u32,
    pub phase: Phase,
    pub case: Case,
    pub verdict: Verdict,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShrinkSummary {
    pub status: ShrinkStatus,
    pub checks: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TestResult {
    pub status: TestStatus,
    pub reason: Option<String>,
    pub seed: u32,
    pub attempts: u32,
    pub cases: u32,
    pub skips: u32,
    pub work: u32,
    pub shrink: Option<ShrinkSummary>,
    pub counterexample: Option<Case>,
    pub signature: Option<String>,
    pub trials: Vec<Trial>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Effect {
    Evaluate { id: u32, phase: Phase, case: Case },
    Done { result: TestResult },
}
pub struct Session {
    plan: Option<Plan>,
    generator: Option<Generator>,
    random: Random,
    work: Work,
    result: TestResult,
    shrinker: Option<Shrinker>,
    effect: Effect,
    next_id: u32,
    replay_signature: Option<String>,
    record_bytes: usize,
}
impl Session {
    fn new(seed: u32, maximum: u32) -> Self {
        let result = TestResult {
            status: TestStatus::Inconclusive,
            reason: None,
            seed,
            attempts: 0,
            cases: 0,
            skips: 0,
            work: 0,
            shrink: None,
            counterexample: None,
            signature: None,
            trials: Vec::new(),
        };
        Self {
            plan: None,
            generator: None,
            random: Random(seed),
            work: Work { used: 0, maximum },
            effect: Effect::Done {
                result: result.clone(),
            },
            result,
            shrinker: None,
            next_id: 1,
            replay_signature: None,
            record_bytes: 0,
        }
    }
    pub fn property(mut plan: Plan) -> Result<Self, Error> {
        if plan.seed == 0
            || plan.cases == 0
            || plan.cases > 256
            || plan.max_attempts < plan.cases
            || plan.max_attempts > 4096
            || !(2..=512).contains(&plan.max_shrink_checks)
        {
            return Err(Error::protocol("invalid property budgets or seed"));
        }
        plan.generator = plan.generator.normalize()?;
        let mut session = Self::new(plan.seed, plan.max_work);
        session.generator = Some(plan.generator.clone());
        session.plan = Some(plan);
        session.generate();
        Ok(session)
    }
    pub fn shrink(
        generator: Generator,
        case: Case,
        max_checks: u32,
        max_work: u32,
    ) -> Result<Self, Error> {
        if !(2..=512).contains(&max_checks) {
            return Err(Error::protocol("shrinking needs 2-512 checks"));
        }
        let generator = generator.normalize()?;
        let case = case.normalize()?;
        if case.generator_id != generator.id || case.generator_version != generator.version {
            return Err(Error::protocol("case generator identity differs"));
        }
        let mut session = Self::new(case.seed, max_work);
        session.generator = Some(generator);
        session.shrinker = Some(Shrinker::new(case.clone(), String::new(), max_checks));
        match session
            .generator
            .as_ref()
            .expect("generator")
            .accepts(&case.actions, &mut session.work)
        {
            Ok(true) => session.emit(Phase::Baseline, case),
            Ok(false) => return Err(Error::protocol("initial counterexample is not enabled")),
            Err(()) => session.finish(TestStatus::Inconclusive, Some("work".into())),
        }
        Ok(session)
    }
    pub fn replay(case: Case, expected_signature: Option<String>) -> Result<Self, Error> {
        let case = case.normalize()?;
        if let Some(signature) = &expected_signature {
            text(signature)?;
        }
        let mut session = Self::new(case.seed, 0);
        session.result.attempts = 1;
        session.replay_signature = expected_signature;
        session.emit(Phase::Replay, case);
        Ok(session)
    }
    pub fn effect(&self) -> Effect {
        self.effect.clone()
    }
    pub fn is_done(&self) -> bool {
        matches!(self.effect, Effect::Done { .. })
    }
    fn emit(&mut self, phase: Phase, case: Case) {
        self.effect = Effect::Evaluate {
            id: self.next_id,
            phase,
            case,
        };
        self.next_id += 1;
    }
    fn finish(&mut self, status: TestStatus, reason: Option<String>) {
        self.result.status = status;
        self.result.reason = reason;
        self.result.work = self.work.used;
        self.effect = Effect::Done {
            result: self.result.clone(),
        };
    }
    fn generate(&mut self) {
        let plan = self.plan.as_ref().expect("property plan");
        loop {
            if self.result.cases >= plan.cases {
                self.finish(TestStatus::Passed, None);
                return;
            }
            if self.result.attempts >= plan.max_attempts {
                self.finish(TestStatus::Exhausted, Some("attempts".into()));
                return;
            }
            let index = self.result.attempts;
            self.result.attempts += 1;
            match self.generator.as_ref().expect("generator").generate(
                &mut self.random,
                plan.seed,
                index,
                &mut self.work,
            ) {
                Ok(Some(case)) => {
                    self.emit(Phase::Generation, case);
                    return;
                }
                Ok(None) => self.result.skips += 1,
                Err(()) => {
                    self.finish(TestStatus::Inconclusive, Some("work".into()));
                    return;
                }
            }
        }
    }
    fn shrink_next(&mut self) {
        let shrinker = self.shrinker.as_mut().expect("shrinker");
        let candidate = shrinker.next(self.generator.as_ref().expect("generator"), &mut self.work);
        match candidate {
            Some(case) => self.emit(Phase::Shrink, case),
            None => {
                let case = shrinker.current.clone();
                self.emit(Phase::Confirmation, case);
            }
        }
    }
    pub fn resume(&mut self, id: u32, verdict: Verdict) -> Result<(), Error> {
        let Effect::Evaluate {
            id: expected,
            phase,
            case,
        } = &self.effect
        else {
            return Err(Error::protocol("session has completed"));
        };
        if id != *expected {
            return Err(Error::protocol("feedback id differs from pending request"));
        }
        verdict.validate()?;
        let trial = Trial {
            id,
            phase: *phase,
            case: case.clone(),
            verdict: verdict.clone(),
        };
        let bytes = serde_json::to_vec(&trial)
            .expect("serializable trial")
            .len();
        if self.record_bytes + bytes > 16 * 1024 * 1024 {
            return Err(Error::resource("trial records exceed 16 MiB"));
        }
        self.record_bytes += bytes;
        self.result.trials.push(trial.clone());
        match *phase {
            Phase::Generation | Phase::Baseline => self.baseline(trial, verdict),
            Phase::Shrink => {
                let shrinker = self.shrinker.as_mut().expect("shrinker");
                shrinker.checks += 1;
                match verdict {
                    Verdict::Mismatch { signature } if signature == shrinker.signature => {
                        shrinker.reduced(trial.case)
                    }
                    Verdict::Unknown { .. } => shrinker.unknown = true,
                    Verdict::Error { message } => {
                        self.finish(TestStatus::Error, Some(message));
                        return Ok(());
                    }
                    _ => (),
                }
                self.shrink_next();
            }
            Phase::Confirmation => {
                let shrinker = self.shrinker.as_mut().expect("shrinker");
                shrinker.checks += 1;
                self.result.counterexample = Some(shrinker.current.clone());
                self.result.signature = Some(shrinker.signature.clone());
                self.result.shrink = Some(ShrinkSummary {
                    status: if shrinker.minimal {
                        ShrinkStatus::Minimal
                    } else {
                        ShrinkStatus::Bounded
                    },
                    checks: shrinker.checks,
                });
                let reason = shrinker.reason.clone();
                match verdict {
                    Verdict::Mismatch { signature } if signature == shrinker.signature => {
                        self.finish(TestStatus::Failed, reason)
                    }
                    Verdict::Unknown { reason } => {
                        self.finish(TestStatus::Inconclusive, Some(reason))
                    }
                    Verdict::Error { message } => self.finish(TestStatus::Error, Some(message)),
                    _ => self.finish(TestStatus::Error, Some("not-reproduced".into())),
                }
            }
            Phase::Replay => self.replayed(trial.case, verdict),
        }
        Ok(())
    }
    fn baseline(&mut self, trial: Trial, verdict: Verdict) {
        match verdict {
            Verdict::Matched {} if trial.phase == Phase::Generation => {
                self.result.cases += 1;
                self.generate();
            }
            Verdict::Invalid {} if trial.phase == Phase::Generation => {
                self.result.skips += 1;
                self.generate();
            }
            Verdict::Mismatch { signature } => {
                let maximum = self.plan.as_ref().map_or_else(
                    || self.shrinker.as_ref().expect("shrinker").maximum,
                    |plan| plan.max_shrink_checks,
                );
                self.shrinker = Some(Shrinker::new(trial.case, signature, maximum));
                self.shrink_next();
            }
            Verdict::Unknown { reason } => self.finish(TestStatus::Inconclusive, Some(reason)),
            Verdict::Error { message } => self.finish(TestStatus::Error, Some(message)),
            _ => self.finish(TestStatus::NotFailing, None),
        }
    }
    fn replayed(&mut self, case: Case, verdict: Verdict) {
        if self.replay_signature.is_some()
            && !matches!(&verdict, Verdict::Mismatch { signature } if Some(signature) == self.replay_signature.as_ref())
            && !matches!(verdict, Verdict::Unknown { .. } | Verdict::Error { .. })
        {
            self.finish(TestStatus::Error, Some("not-reproduced".into()));
            return;
        }
        match verdict {
            Verdict::Matched {} => {
                self.result.cases = 1;
                self.finish(TestStatus::Passed, None);
            }
            Verdict::Mismatch { signature } => {
                self.result.counterexample = Some(case);
                self.result.signature = Some(signature);
                self.finish(TestStatus::Failed, None);
            }
            Verdict::Invalid {} => {
                self.result.skips = 1;
                self.finish(TestStatus::Exhausted, Some("invalid".into()));
            }
            Verdict::Unknown { reason } => self.finish(TestStatus::Inconclusive, Some(reason)),
            Verdict::Error { message } => self.finish(TestStatus::Error, Some(message)),
        }
    }
}
