use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyntaxVersion {
    #[default]
    #[serde(rename = "1.7.4")]
    Tlc174,
    #[serde(rename = "1.8.0")]
    Tlc180,
}
