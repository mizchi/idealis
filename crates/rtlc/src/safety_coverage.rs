use crate::ir::{
    SafetyActionCoverage, SafetyCoverage, SafetyCoverageReport, SafetyProgram, SafetyResult,
};

pub(super) trait Recording: Sync {
    type Output;
    const ENABLED: bool;
    fn new(program: &SafetyProgram) -> Self;
    fn record(&mut self, initial: bool, action: usize, work: u32, generated: u32, distinct: usize);
    fn finish(self, result: SafetyResult) -> Self::Output;
}

pub(super) struct NoCoverage;
impl Recording for NoCoverage {
    type Output = SafetyResult;
    const ENABLED: bool = false;
    fn new(_: &SafetyProgram) -> Self {
        Self
    }
    fn record(&mut self, _: bool, _: usize, _: u32, _: u32, _: usize) {}
    fn finish(self, result: SafetyResult) -> Self::Output {
        result
    }
}

pub(super) struct WithCoverage(SafetyCoverage);
impl Recording for WithCoverage {
    type Output = SafetyCoverageReport;
    const ENABLED: bool = true;
    fn new(program: &SafetyProgram) -> Self {
        Self(SafetyCoverage {
            initial: program
                .initial
                .iter()
                .map(|branch| SafetyActionCoverage {
                    name: branch.name.clone(),
                    ..SafetyActionCoverage::default()
                })
                .collect(),
            next: program
                .next
                .iter()
                .map(|branch| SafetyActionCoverage {
                    name: branch.name.clone(),
                    ..SafetyActionCoverage::default()
                })
                .collect(),
            unattributed_work: 0,
        })
    }
    fn record(&mut self, initial: bool, action: usize, work: u32, generated: u32, distinct: usize) {
        let row = if initial {
            &mut self.0.initial[action]
        } else {
            &mut self.0.next[action]
        };
        row.attempts += 1;
        row.enabled += u32::from(generated > 0);
        row.generated += generated;
        row.distinct += distinct;
        row.work += work;
    }
    fn finish(mut self, result: SafetyResult) -> Self::Output {
        self.0.unattributed_work = result.work
            - self
                .0
                .initial
                .iter()
                .chain(&self.0.next)
                .map(|row| row.work)
                .sum::<u32>();
        SafetyCoverageReport {
            result,
            coverage: self.0,
        }
    }
}
