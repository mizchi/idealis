use crate::tape::{Tape, View};
use roracle::{Case, CaseReport, CheckReport, Error, Observation, OracleGroup};
use serde::Serialize;

pub enum InputObservation<'a> {
    Value(u32),
    Allowed(Vec<u32>),
    Unknown(&'a str),
    Invalid(&'a str),
    Error(&'a str),
}
pub struct Input<'a> {
    pub id: &'a str,
    pub input: u32,
    pub reference: InputObservation<'a>,
    pub actual: Vec<InputObservation<'a>>,
}
pub struct Batch<'a> {
    pub version: u32,
    pub group: &'a str,
    pub reference: &'a str,
    pub implementations: Vec<&'a str>,
    pub cases: Vec<Input<'a>>,
    pub payload: &'a [u8],
}
#[derive(Serialize)]
struct WireInput<'a> {
    id: &'a str,
    input: View<'a>,
    reference: Observation<View<'a>>,
    actual: Vec<Observation<View<'a>>>,
}
#[derive(Serialize)]
struct WireRequest<'a> {
    version: u32,
    group: &'a str,
    reference: &'a str,
    implementations: &'a [&'a str],
    cases: Vec<WireInput<'a>>,
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum Reply<T> {
    Checked { report: T },
    Error { message: String },
}
struct SizeLimit(usize);
impl std::io::Write for SizeLimit {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > (16 * 1024 * 1024usize).saturating_sub(self.0) {
            return Err(std::io::Error::other("JSON batch exceeds 16 MiB"));
        }
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn root<'a>(tape: &Tape<'a>, offset: u32, base: u32, nodes: &mut u32) -> Result<View<'a>, Error> {
    let info = tape.info(offset)?;
    if info.height + base >= 128 {
        return Err(Error("JSON batch exceeds parser depth".into()));
    }
    *nodes = nodes
        .checked_add(info.nodes)
        .filter(|nodes| *nodes <= 1_000_000)
        .ok_or_else(|| Error("typed batch node budget exceeded".into()))?;
    tape.view(offset)
}
fn observation<'a>(
    input: &InputObservation<'a>,
    tape: &Tape<'a>,
    nodes: &mut u32,
    base: u32,
) -> Result<Observation<View<'a>>, Error> {
    *nodes += match input {
        InputObservation::Value(_) => 4,
        _ => 5,
    };
    Ok(match input {
        InputObservation::Value(offset) => Observation::value(root(tape, *offset, base, nodes)?),
        InputObservation::Allowed(offsets) => Observation::Allowed {
            values: offsets
                .iter()
                .map(|offset| root(tape, *offset, base + 1, nodes))
                .collect::<Result<_, _>>()?,
        },
        InputObservation::Unknown(reason) => Observation::Unknown {
            reason: (*reason).into(),
        },
        InputObservation::Invalid(reason) => Observation::Invalid {
            reason: (*reason).into(),
        },
        InputObservation::Error(message) => Observation::Error {
            message: (*message).into(),
        },
    })
}

pub fn check(batch: Batch<'_>) -> Result<String, Error> {
    if batch.version != 1
        || batch.cases.is_empty()
        || batch.cases.len() > 4096
        || batch.implementations.is_empty()
        || batch.implementations.len() > 256
        || batch
            .cases
            .iter()
            .any(|case| case.actual.len() != batch.implementations.len())
    {
        return Err(Error(
            "invalid version, batch dimensions or observation count".into(),
        ));
    }
    let tape = Tape::new(batch.payload)?;
    let mut nodes = 11 + batch.implementations.len() as u32;
    let mut cases = Vec::with_capacity(batch.cases.len());
    for input in &batch.cases {
        nodes += 7;
        cases.push(WireInput {
            id: input.id,
            input: root(&tape, input.input, 3, &mut nodes)?,
            reference: observation(&input.reference, &tape, &mut nodes, 4)?,
            actual: input
                .actual
                .iter()
                .map(|input| observation(input, &tape, &mut nodes, 5))
                .collect::<Result<_, _>>()?,
        });
    }
    if nodes > 1_000_000 {
        return Err(Error("typed batch node budget exceeded".into()));
    }
    let request = WireRequest {
        version: batch.version,
        group: batch.group,
        reference: batch.reference,
        implementations: &batch.implementations,
        cases,
    };
    serde_json::to_writer(SizeLimit(0), &request).map_err(|error| Error(error.to_string()))?;
    let mut group = OracleGroup::new(batch.group);
    for (index, input) in request.cases.iter().enumerate() {
        group.case(input.id, index);
    }
    group.reference(batch.reference, |index| {
        request.cases[*index].reference.clone()
    });
    for (implementation, id) in batch.implementations.iter().enumerate() {
        let cases = &request.cases;
        group.test_function(*id, move |index| {
            cases[*index].actual[implementation].clone()
        });
    }
    let report = group.check()?;
    let report = CheckReport {
        version: report.version,
        group: report.group,
        reference: report.reference,
        implementations: report.implementations,
        status: report.status,
        cases: report
            .cases
            .into_iter()
            .map(|case| CaseReport {
                case: Case {
                    id: case.case.id,
                    input: request.cases[case.case.input].input,
                },
                reference: case.reference,
                comparisons: case.comparisons,
            })
            .collect(),
    };
    Ok(serde_json::to_string(&Reply::Checked { report }).expect("serializable borrowed report"))
}

pub fn reply(result: Result<String, Error>) -> String {
    result.unwrap_or_else(|error| {
        serde_json::to_string(&Reply::<()>::Error {
            message: error.to_string(),
        })
        .expect("serializable typed error")
    })
}
