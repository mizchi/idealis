//! Incrementally maintained assertion context.
//!
//! A `Session` answers exactly like compiling its accumulated source from
//! scratch. It keeps the parsed assertions so that an `assert` tokenizes only
//! the new expression. Constant substitution is global: an assertion that binds
//! a new integer constant rewrites the earlier assertions, so that case
//! recompiles every stored assertion from its parsed node instead of the text.
use crate::{
    Edge, Error, Formula, Problem, Result,
    parser::{self, Asserted, Constants, Declarations},
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const SOURCE_LIMIT: usize = 1_048_576;
const TOKEN_LIMIT: usize = 200_000;
const NESTING_LIMIT: usize = 64;

#[derive(Clone)]
pub(crate) struct Session {
    declarations: Arc<Declarations>,
    assertions: Vec<Asserted>,
    formulas: Vec<Arc<Formula>>,
    constants: Arc<Constants>,
    anchors: Vec<Edge>,
    /// Length and tokens of the normalized source the context stands for.
    length: usize,
    tokens: usize,
    /// Running SHA-256 of the normalized commands, each followed by a newline.
    source: Sha256,
    problem: Arc<Problem>,
}
impl Session {
    pub(crate) fn parse(source: &str) -> Result<Self> {
        let mut nodes = parser::parse(source)?;
        let (declarations, assertions) = parser::commands(&nodes)?;
        let mut constants = Constants::new();
        for assertion in &assertions {
            let found = parser::new_constants(&declarations, assertion, &constants);
            constants.extend(found);
        }
        let formulas = compile_all(&declarations, &constants, &assertions)?;
        if nodes.last().is_some_and(parser::is_check) {
            nodes.pop();
        }
        let length = nodes
            .iter()
            .map(|node| parser::render(node).len())
            .sum::<usize>()
            + nodes.len().saturating_sub(1);
        if length > SOURCE_LIMIT {
            return Err(Error::new("resource", "normalized source exceeds 1 MiB"));
        }
        let tokens = nodes.iter().map(parser::token_count).sum();
        let mut source = Sha256::new();
        for node in &nodes {
            source.update(parser::render(node).as_bytes());
            source.update(b"\n");
        }
        let anchors = parser::anchors(&declarations, &constants);
        let problem = Arc::new(parser::problem(
            &declarations,
            anchors.clone(),
            formulas.clone(),
        ));
        Ok(Self {
            declarations: Arc::new(declarations),
            assertions,
            formulas,
            constants: Arc::new(constants),
            anchors,
            length,
            tokens,
            source,
            problem,
        })
    }
    /// Adds one assertion. On error the session is left unchanged.
    pub(crate) fn assert(&mut self, expression: &str) -> Result<()> {
        let mut nodes = parser::parse(expression)?;
        if nodes.len() != 1 {
            return Err(Error::new("syntax", "assert needs a single expression"));
        }
        let node = nodes.pop().expect("one expression");
        // Checked in the order a reparse of the accumulated source reports them.
        let rendered = format!("(assert {})", parser::render(&node));
        let length = self.length + 1 + rendered.len();
        if length > SOURCE_LIMIT {
            return Err(Error::new("resource", "source exceeds 1 MiB"));
        }
        let tokens = self.tokens + 3 + parser::token_count(&node);
        if tokens > TOKEN_LIMIT {
            return Err(Error::new("resource", "too many tokens"));
        }
        if parser::nesting(&node) >= NESTING_LIMIT {
            return Err(Error::new("resource", "nesting exceeds 64"));
        }
        let assertion = Asserted {
            node: Arc::new(node),
            visible: self.declarations.visible(),
        };
        let found = parser::new_constants(&self.declarations, &assertion, &self.constants);
        if !found.is_empty() {
            let mut constants = (*self.constants).clone();
            constants.extend(found);
            let mut assertions = self.assertions.clone();
            assertions.push(assertion);
            let formulas = compile_all(&self.declarations, &constants, &assertions)?;
            self.anchors = parser::anchors(&self.declarations, &constants);
            self.constants = Arc::new(constants);
            self.assertions = assertions;
            self.formulas = formulas;
        } else {
            let formula = parser::formula(&self.declarations, &self.constants, &assertion)?;
            self.assertions.push(assertion);
            self.formulas.push(formula);
        }
        self.length = length;
        self.tokens = tokens;
        self.source.update(rendered.as_bytes());
        self.source.update(b"\n");
        self.problem = Arc::new(parser::problem(
            &self.declarations,
            self.anchors.clone(),
            self.formulas.clone(),
        ));
        Ok(())
    }
    pub(crate) fn problem(&self) -> &Arc<Problem> {
        &self.problem
    }
    /// SHA-256 of the normalized source this session stands for.
    pub(crate) fn source_digest(&self) -> [u8; 32] {
        self.source.clone().finalize().into()
    }
    /// The running hash state, for callers that may never need the digest.
    pub(crate) fn source_state(&self) -> Sha256 {
        self.source.clone()
    }
}
fn compile_all(
    declarations: &Declarations,
    constants: &Constants,
    assertions: &[Asserted],
) -> Result<Vec<Arc<Formula>>> {
    assertions
        .iter()
        .map(|assertion| parser::formula(declarations, constants, assertion))
        .collect()
}
