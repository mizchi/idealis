use crate::report::compare;
use crate::{CheckReport, Error, Observation, OracleGroup, Status};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BenchConfig {
    pub baseline: String,
    pub scope: String,
    pub samples: u32,
    pub warmup: u32,
    pub iterations: u32,
    pub max_evaluations: u64,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            baseline: String::new(),
            scope: "function call and output construction".into(),
            samples: 20,
            warmup: 3,
            iterations: 1,
            max_evaluations: 1_000_000,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Statistics {
    pub mean_ns: f64,
    pub median_ns: f64,
    pub min_ns: f64,
    pub max_ns: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    pub id: String,
    /// Total elapsed nanoseconds for each sample's fixed iterations.
    pub samples_ns: Vec<u64>,
    /// Per-invocation statistics, including only fully completed samples.
    pub statistics: Option<Statistics>,
    /// Baseline median divided by this implementation's median. Only set on a complete matched run.
    pub speedup: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BenchCase {
    pub id: String,
    pub order: Vec<Vec<String>>,
    pub measurements: Vec<Measurement>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BenchFailure<O> {
    pub phase: String,
    pub case: String,
    pub implementation: String,
    pub sample: Option<u32>,
    pub iteration: u32,
    pub status: Status,
    pub actual: Observation<O>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BenchReport<I, O> {
    pub preflight: CheckReport<I, O>,
    pub config: BenchConfig,
    pub status: Status,
    pub evaluations: u64,
    pub planned_evaluations: u64,
    pub cases: Vec<BenchCase>,
    pub failure: Option<BenchFailure<O>>,
}

impl<I: Debug, O: Debug> BenchReport<I, O> {
    #[track_caller]
    pub fn assert_matched(&self) {
        assert_eq!(self.status, Status::Matched, "{self:#?}");
    }
}

impl<I: Clone, O: Clone + PartialEq> OracleGroup<'_, I, O> {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn bench(&mut self, config: BenchConfig) -> Result<BenchReport<I, O>, Error> {
        let start = std::time::Instant::now();
        self.bench_with_clock(config, || {
            u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
        })
    }

    /// The caller supplies a monotonic nanosecond clock. Comparisons and output destruction are outside it.
    pub fn bench_with_clock(
        &mut self,
        config: BenchConfig,
        mut clock: impl FnMut() -> u64,
    ) -> Result<BenchReport<I, O>, Error> {
        self.validate(true)?;
        if config.samples == 0
            || config.iterations == 0
            || config.scope.trim().is_empty()
            || !self
                .targets
                .iter()
                .any(|target| target.id == config.baseline)
        {
            return Err(Error(
                "bench needs positive samples/iterations, a scope and a registered target baseline"
                    .into(),
            ));
        }
        let count = self.cases.len() as u64;
        let targets = self.targets.len() as u64;
        let planned_evaluations = u64::from(config.samples)
            .checked_mul(u64::from(config.iterations))
            .and_then(|calls| calls.checked_add(u64::from(config.warmup)))
            .and_then(|calls| calls.checked_add(1))
            .and_then(|calls| calls.checked_mul(targets))
            .and_then(|calls| calls.checked_add(1))
            .and_then(|calls| calls.checked_mul(count))
            .ok_or_else(|| Error("evaluation count overflow".into()))?;
        if planned_evaluations > config.max_evaluations {
            return Err(Error(format!(
                "planned {planned_evaluations} evaluations exceed max_evaluations"
            )));
        }
        let preflight = self.check()?;
        let mut report = BenchReport {
            status: preflight.status,
            evaluations: count * (targets + 1),
            planned_evaluations,
            cases: self
                .cases
                .iter()
                .map(|case| BenchCase {
                    id: case.id.clone(),
                    order: vec![],
                    measurements: self
                        .targets
                        .iter()
                        .map(|target| Measurement {
                            id: target.id.clone(),
                            samples_ns: vec![],
                            statistics: None,
                            speedup: None,
                        })
                        .collect(),
                })
                .collect(),
            config,
            preflight,
            failure: None,
        };
        if report.status != Status::Matched {
            return Ok(report);
        }
        let mut previous_tick = None;
        'cases: for (case_index, case) in self.cases.iter().enumerate() {
            let reference = &report.preflight.cases[case_index].reference;
            for iteration in 0..report.config.warmup {
                for target in &mut self.targets {
                    let actual = (target.run)(&case.input);
                    report.evaluations += 1;
                    let status = compare(reference, &actual);
                    if status != Status::Matched {
                        report.failure = Some(BenchFailure {
                            phase: "warmup".into(),
                            case: case.id.clone(),
                            implementation: target.id.clone(),
                            sample: None,
                            iteration,
                            status,
                            actual,
                        });
                        report.status = status;
                        break 'cases;
                    }
                }
            }
            for sample in 0..report.config.samples {
                let offset = (sample as usize / 2) % self.targets.len();
                let mut order = (0..self.targets.len())
                    .map(|index| (offset + index) % self.targets.len())
                    .collect::<Vec<_>>();
                if sample % 2 == 1 {
                    order.reverse();
                }
                report.cases[case_index].order.push(
                    order
                        .iter()
                        .map(|index| self.targets[*index].id.clone())
                        .collect(),
                );
                for target_index in order {
                    let target = &mut self.targets[target_index];
                    let mut elapsed = 0_u64;
                    for iteration in 0..report.config.iterations {
                        let start = clock();
                        let actual =
                            std::hint::black_box((target.run)(std::hint::black_box(&case.input)));
                        let end = clock();
                        report.evaluations += 1;
                        let status = compare(reference, &actual);
                        if status != Status::Matched {
                            report.failure = Some(BenchFailure {
                                phase: "sample".into(),
                                case: case.id.clone(),
                                implementation: target.id.clone(),
                                sample: Some(sample),
                                iteration,
                                status,
                                actual,
                            });
                            report.status = status;
                            break 'cases;
                        }
                        let duration = end
                            .checked_sub(start)
                            .filter(|duration| *duration > 0)
                            .filter(|_| previous_tick.is_none_or(|previous| start >= previous));
                        previous_tick = Some(end);
                        let total = duration.and_then(|duration| elapsed.checked_add(duration));
                        let Some(total) = total else {
                            report.status = Status::Unknown;
                            report.failure = Some(BenchFailure { phase: "clock".into(), case: case.id.clone(), implementation: target.id.clone(), sample: Some(sample), iteration, status: Status::Unknown, actual: Observation::Unknown { reason: "clock must advance monotonically without zero intervals or overflow".into() } });
                            break 'cases;
                        };
                        elapsed = total;
                    }
                    report.cases[case_index].measurements[target_index]
                        .samples_ns
                        .push(elapsed);
                }
            }
        }
        for case in &mut report.cases {
            for measurement in &mut case.measurements {
                if measurement.samples_ns.is_empty() {
                    continue;
                }
                let mut sorted = measurement
                    .samples_ns
                    .iter()
                    .map(|sample| *sample as f64 / f64::from(report.config.iterations))
                    .collect::<Vec<_>>();
                sorted.sort_by(f64::total_cmp);
                let middle = sorted.len() / 2;
                measurement.statistics = Some(Statistics {
                    mean_ns: sorted.iter().sum::<f64>() / sorted.len() as f64,
                    median_ns: if sorted.len() % 2 == 0 {
                        (sorted[middle - 1] + sorted[middle]) / 2.0
                    } else {
                        sorted[middle]
                    },
                    min_ns: sorted[0],
                    max_ns: sorted[sorted.len() - 1],
                });
            }
            if report.status == Status::Matched {
                let baseline = case
                    .measurements
                    .iter()
                    .find(|measurement| measurement.id == report.config.baseline)
                    .and_then(|measurement| measurement.statistics.as_ref())
                    .expect("completed baseline")
                    .median_ns;
                for measurement in &mut case.measurements {
                    measurement.speedup = measurement
                        .statistics
                        .as_ref()
                        .map(|statistics| baseline / statistics.median_ns);
                }
            }
        }
        Ok(report)
    }
}
