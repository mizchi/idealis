use crate::Observation;
use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::fmt;

#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub(crate) struct WireObservation(pub Observation<Value>);

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "kebab-case")]
enum Field {
    Kind,
    Value,
    Values,
    Reason,
    Message,
}

fn field<'de, T: Deserialize<'de>, M: MapAccess<'de>>(
    map: &mut M,
    slot: &mut Option<T>,
    name: &'static str,
) -> Result<(), M::Error> {
    if slot.is_some() {
        return Err(de::Error::duplicate_field(name));
    }
    *slot = Some(map.next_value()?);
    Ok(())
}

impl<'de> Deserialize<'de> for WireObservation {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObservationVisitor;
        impl<'de> Visitor<'de> for ObservationVisitor {
            type Value = WireObservation;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a tagged observation")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut kind: Option<String> = None;
                let mut value = None;
                let mut values = None;
                let mut reason = None;
                let mut message = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Kind => field(&mut map, &mut kind, "kind")?,
                        Field::Value => field(&mut map, &mut value, "value")?,
                        Field::Values => field(&mut map, &mut values, "values")?,
                        Field::Reason => field(&mut map, &mut reason, "reason")?,
                        Field::Message => field(&mut map, &mut message, "message")?,
                    }
                }
                let observation = match (kind.as_deref(), value, values, reason, message) {
                    (Some("value"), Some(value), None, None, None) => Observation::Value { value },
                    (Some("allowed"), None, Some(values), None, None) => {
                        Observation::Allowed { values }
                    }
                    (Some("unknown"), None, None, Some(reason), None) => {
                        Observation::Unknown { reason }
                    }
                    (Some("invalid"), None, None, Some(reason), None) => {
                        Observation::Invalid { reason }
                    }
                    (Some("error"), None, None, None, Some(message)) => {
                        Observation::Error { message }
                    }
                    _ => return Err(de::Error::custom("invalid observation kind or fields")),
                };
                Ok(WireObservation(observation))
            }

            fn visit_seq<S: SeqAccess<'de>>(
                self,
                mut sequence: S,
            ) -> Result<Self::Value, S::Error> {
                let kind: String = sequence
                    .next_element()?
                    .ok_or_else(|| de::Error::missing_field("kind"))?;
                let observation = match kind.as_str() {
                    "value" => Observation::Value {
                        value: sequence
                            .next_element()?
                            .ok_or_else(|| de::Error::missing_field("value"))?,
                    },
                    "allowed" => Observation::Allowed {
                        values: sequence
                            .next_element()?
                            .ok_or_else(|| de::Error::missing_field("values"))?,
                    },
                    "unknown" => Observation::Unknown {
                        reason: sequence
                            .next_element()?
                            .ok_or_else(|| de::Error::missing_field("reason"))?,
                    },
                    "invalid" => Observation::Invalid {
                        reason: sequence
                            .next_element()?
                            .ok_or_else(|| de::Error::missing_field("reason"))?,
                    },
                    "error" => Observation::Error {
                        message: sequence
                            .next_element()?
                            .ok_or_else(|| de::Error::missing_field("message"))?,
                    },
                    _ => return Err(de::Error::custom("invalid observation kind")),
                };
                if sequence.next_element::<IgnoredAny>()?.is_some() {
                    return Err(de::Error::custom(
                        "observation sequence requires one payload",
                    ));
                }
                Ok(WireObservation(observation))
            }
        }
        deserializer.deserialize_any(ObservationVisitor)
    }
}
