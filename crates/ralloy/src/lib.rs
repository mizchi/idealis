mod check;
mod engine;
mod eval;
mod instance;
mod parser;
mod sat;
mod search;
mod syntax;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use engine::Engine;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorKind {
    Syntax,
    Unsupported,
    Type,
    Scope,
    Resource,
    Protocol,
}

#[derive(Debug, Serialize)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}
impl Error {
    fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Run,
    Check,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Instance,
    Counterexample,
    NoInstance,
    NoCounterexample,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckStatus {
    Matched,
    Mismatch,
    Invalid,
    Unknown,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckReason {
    Scope,
    Declaration,
    Facts,
    Property,
    Work,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstanceCheck {
    pub command: Command,
    pub status: CheckStatus,
    pub reason: Option<CheckReason>,
    pub stats: Stats,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignatureScope {
    pub name: String,
    pub maximum: usize,
    pub exact: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scope {
    pub signatures: Vec<SignatureScope>,
    pub integer_bitwidth: usize,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Command {
    pub label: String,
    pub kind: Kind,
    pub scope: Scope,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Instance {
    pub signatures: BTreeMap<String, Vec<String>>,
    /// Tuples of each field, owner atom first; binary fields have pairs.
    pub fields: BTreeMap<String, Vec<Vec<String>>>,
}
#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stats {
    pub work: u64,
    pub candidates: u64,
    pub fact_instances: u64,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Analysis {
    pub command: Command,
    pub status: Status,
    pub instance: Option<Instance>,
    pub stats: Stats,
}

pub struct Model {
    syntax: syntax::Module,
    commands: Vec<Command>,
}
impl Model {
    pub fn parse(source: &str) -> Result<Self, Error> {
        let mut syntax = parser::parse(source)?;
        let commands = check::validate(&syntax)?;
        syntax.hierarchy = check::hierarchy(&syntax)?;
        Ok(Self { syntax, commands })
    }
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }
    pub fn check_instance(
        &self,
        command: usize,
        instance: &Instance,
        max_work: u64,
    ) -> Result<InstanceCheck, Error> {
        let (metadata, body) = self.get_command(command)?;
        instance::check(&self.syntax, metadata, body, instance, max_work)
    }
    pub fn analyze(&self, command: usize, max_work: u64) -> Result<Analysis, Error> {
        let (metadata, body) = self.get_command(command)?;
        search::analyze(&self.syntax, metadata, body, max_work)
    }
    fn get_command(&self, command: usize) -> Result<(&Command, &syntax::Expr), Error> {
        let metadata = self
            .commands
            .get(command)
            .ok_or_else(|| Error::new(ErrorKind::Protocol, "command index out of range"))?;
        let body = &self.syntax.commands[command].body;
        let body = if let (Kind::Check, syntax::Expr::Name(name)) = (metadata.kind, body) {
            &self.syntax.assertions[name]
        } else {
            body
        };
        Ok((metadata, body))
    }
}
