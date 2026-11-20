use crate::ast::{Config, Expression, Location, Module};
use crate::{
    ErrorKind, ParseError, SyntaxVersion, parse_config, parse_expression_with_version,
    parse_module_with_version,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Request {
    Module {
        source: String,
        #[serde(default, rename = "syntaxVersion")]
        syntax_version: SyntaxVersion,
    },
    Expression {
        source: String,
        #[serde(default, rename = "syntaxVersion")]
        syntax_version: SyntaxVersion,
    },
    Config {
        source: String,
    },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Response {
    Module { module: Module },
    Expression { expression: Expression },
    Config { config: Box<Config> },
    Error { error: ParseError },
}
pub fn dispatch(input: &[u8]) -> Vec<u8> {
    let response = match serde_json::from_slice::<Request>(input) {
        Ok(Request::Module {
            source,
            syntax_version,
        }) => parse_module_with_version(&source, syntax_version)
            .map(|module| Response::Module { module }),
        Ok(Request::Expression {
            source,
            syntax_version,
        }) => parse_expression_with_version(&source, syntax_version)
            .map(|expression| Response::Expression { expression }),
        Ok(Request::Config { source }) => parse_config(&source).map(|config| Response::Config {
            config: Box::new(config),
        }),
        Err(error) => Err(ParseError {
            kind: ErrorKind::Syntax,
            message: format!("Invalid parser request: {error}"),
            location: Location {
                start: 0,
                end: 0,
                line: 1,
                column: 1,
            },
        }),
    }
    .unwrap_or_else(|error| Response::Error { error });
    serde_json::to_vec(&response).expect("serializable parser result")
}
