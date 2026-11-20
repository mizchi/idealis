use crate::batch::{Batch, Input, InputObservation};
use crate::exports::idealis::oracle::protocol::{Batch as WitBatch, Guest, Observation};

fn observation(input: &Observation) -> InputObservation<'_> {
    match input {
        Observation::Value(offset) => InputObservation::Value(*offset),
        Observation::Allowed(offsets) => InputObservation::Allowed(offsets.clone()),
        Observation::Unknown(reason) => InputObservation::Unknown(reason),
        Observation::Invalid(reason) => InputObservation::Invalid(reason),
        Observation::Error(message) => InputObservation::Error(message),
    }
}
pub struct Component;
impl Guest for Component {
    fn check(request: WitBatch) -> String {
        let batch = Batch {
            version: request.version,
            group: &request.group,
            reference: &request.reference,
            implementations: request.implementations.iter().map(String::as_str).collect(),
            cases: request
                .cases
                .iter()
                .map(|input| Input {
                    id: &input.id,
                    input: input.input,
                    reference: observation(&input.reference),
                    actual: input.actual.iter().map(observation).collect(),
                })
                .collect(),
            payload: &request.payload,
        };
        crate::batch::reply(crate::batch::check(batch))
    }
    fn check_buffer(data: Vec<u8>) -> String {
        crate::buffer::dispatch(&data)
    }
}
