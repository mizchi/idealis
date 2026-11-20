use crate::report::{compare, identities};
use crate::{Case, CaseReport, CheckReport, Comparison, Error, Observation, Status};

type Callback<'a, I, O> = Box<dyn FnMut(&I) -> Observation<O> + 'a>;

pub(crate) struct Implementation<'a, I, O> {
    pub id: String,
    pub run: Callback<'a, I, O>,
}

pub struct OracleGroup<'a, I, O> {
    pub(crate) id: String,
    pub(crate) cases: Vec<Case<I>>,
    references: Vec<Implementation<'a, I, O>>,
    pub(crate) targets: Vec<Implementation<'a, I, O>>,
}

impl<'a, I: Clone, O: Clone + PartialEq> OracleGroup<'a, I, O> {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            cases: vec![],
            references: vec![],
            targets: vec![],
        }
    }

    pub fn case(&mut self, id: impl Into<String>, input: I) -> &mut Self {
        self.cases.push(Case {
            id: id.into(),
            input,
        });
        self
    }

    pub fn reference(
        &mut self,
        id: impl Into<String>,
        run: impl FnMut(&I) -> Observation<O> + 'a,
    ) -> &mut Self {
        self.references.push(Implementation {
            id: id.into(),
            run: Box::new(run),
        });
        self
    }

    pub fn test_function(
        &mut self,
        id: impl Into<String>,
        run: impl FnMut(&I) -> Observation<O> + 'a,
    ) -> &mut Self {
        self.targets.push(Implementation {
            id: id.into(),
            run: Box::new(run),
        });
        self
    }

    pub(crate) fn validate(&self, require_cases: bool) -> Result<(), Error> {
        identities(std::iter::once(self.id.as_str()))?;
        if self.references.len() != 1
            || self.targets.is_empty()
            || (require_cases && self.cases.is_empty())
        {
            return Err(Error(
                "register exactly one reference, at least one case and one target".into(),
            ));
        }
        identities(
            self.references
                .iter()
                .chain(&self.targets)
                .map(|implementation| implementation.id.as_str()),
        )?;
        identities(self.cases.iter().map(|case| case.id.as_str()))
    }

    pub fn check(&mut self) -> Result<CheckReport<I, O>, Error> {
        self.validate(true)?;
        let mut cases = Vec::new();
        for case in &self.cases {
            let reference = (self.references[0].run)(&case.input);
            let comparisons = self
                .targets
                .iter_mut()
                .map(|target| {
                    let actual = (target.run)(&case.input);
                    Comparison {
                        id: target.id.clone(),
                        status: compare(&reference, &actual),
                        actual,
                    }
                })
                .collect();
            cases.push(CaseReport {
                case: case.clone(),
                reference,
                comparisons,
            });
        }
        Ok(self.report(cases))
    }

    fn report(&self, cases: Vec<CaseReport<I, O>>) -> CheckReport<I, O> {
        CheckReport {
            version: 1,
            group: self.id.clone(),
            reference: self.references[0].id.clone(),
            implementations: self
                .targets
                .iter()
                .map(|target| target.id.clone())
                .collect(),
            status: Status::aggregate(
                cases
                    .iter()
                    .flat_map(|case| case.comparisons.iter().map(|comparison| comparison.status)),
            ),
            cases,
        }
    }

    /// Re-run targets against recorded observations. The reference callback is not called.
    pub fn replay(&mut self, saved: &CheckReport<I, O>) -> Result<CheckReport<I, O>, Error>
    where
        I: PartialEq,
    {
        self.validate(false)?;
        if saved.version != 1
            || saved.group != self.id
            || saved.reference != self.references[0].id
            || saved.implementations
                != self
                    .targets
                    .iter()
                    .map(|target| target.id.clone())
                    .collect::<Vec<_>>()
            || saved.cases.is_empty()
        {
            return Err(Error(
                "saved report does not match group identities or version".into(),
            ));
        }
        identities(saved.cases.iter().map(|case| case.case.id.as_str()))?;
        if !self.cases.is_empty()
            && self.cases
                != saved
                    .cases
                    .iter()
                    .map(|case| case.case.clone())
                    .collect::<Vec<_>>()
        {
            return Err(Error("registered cases differ from recorded inputs".into()));
        }
        for case in &saved.cases {
            if case.comparisons.len() != self.targets.len()
                || case
                    .comparisons
                    .iter()
                    .zip(&self.targets)
                    .any(|(comparison, target)| {
                        comparison.id != target.id
                            || comparison.status != compare(&case.reference, &comparison.actual)
                    })
            {
                return Err(Error(
                    "saved comparison status or identity is inconsistent".into(),
                ));
            }
        }
        if self.report(saved.cases.clone()).status != saved.status {
            return Err(Error("saved aggregate status is inconsistent".into()));
        }
        let cases = saved
            .cases
            .iter()
            .map(|case| CaseReport {
                case: case.case.clone(),
                reference: case.reference.clone(),
                comparisons: self
                    .targets
                    .iter_mut()
                    .map(|target| {
                        let actual = (target.run)(&case.case.input);
                        Comparison {
                            id: target.id.clone(),
                            status: compare(&case.reference, &actual),
                            actual,
                        }
                    })
                    .collect(),
            })
            .collect();
        Ok(self.report(cases))
    }
}
