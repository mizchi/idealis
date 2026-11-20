pub mod ast;
mod config;
mod error;
mod lexer;
mod parser;
mod protocol;
mod version;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use config::parse_config;
pub use error::{ErrorKind, ParseError};
pub use parser::{
    parse_expression, parse_expression_with_version, parse_module, parse_module_with_version,
};
pub use protocol::{Request, Response, dispatch};
pub use version::SyntaxVersion;

const MAX_NESTING: usize = 64;
