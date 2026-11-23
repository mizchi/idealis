//! Incrementally maintained assertion context.
//!
//! A `Session` answers exactly like compiling its accumulated source from
//! scratch. It keeps the parsed assertions so that an `assert` tokenizes only
//! the new expression. Constant substitution is global: an assertion that binds
//! a new integer constant rewrites the earlier assertions, so that case
//! recompiles the stored assertions that mention a newly bound name from their
//! parsed nodes instead of the text. The source hash and the `Problem` are
//! built only when a check or a cache key needs them (plan 0077).
use crate::{
    Edge, Error, Formula, Problem, Result,
    parser::{self, Asserted, Constants, Declarations},
    source::SourceHash,
};
use std::sync::{Arc, OnceLock};

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
    /// The normalized commands, each followed by a newline.
    source: SourceHash,
    problem: OnceLock<Arc<Problem>>,
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
        let length =
            nodes.iter().map(parser::rendered_len).sum::<usize>() + nodes.len().saturating_sub(1);
        if length > SOURCE_LIMIT {
            return Err(Error::new("resource", "normalized source exceeds 1 MiB"));
        }
        let tokens = nodes.iter().map(parser::token_count).sum();
        let anchors = parser::anchors(&declarations, &constants);
        Ok(Self {
            declarations: Arc::new(declarations),
            assertions,
            formulas,
            constants: Arc::new(constants),
            anchors,
            length,
            tokens,
            source: SourceHash::commands(Arc::new(nodes)),
            problem: OnceLock::new(),
        })
    }
    /// Adds one assertion. On error the session is left unchanged.
    pub(crate) fn assert(&mut self, expression: &str) -> Result<()> {
        let mut nodes = parser::parse(expression)?;
        if nodes.len() != 1 {
            return Err(Error::new("syntax", "assert needs a single expression"));
        }
        let node = nodes.pop().expect("one expression");
        // Checked in the order a reparse of the accumulated source reports
        // them; "(assert " and ")" add 9 bytes and 3 tokens.
        let length = self.length + 1 + 9 + parser::rendered_len(&node);
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
        let node = Arc::new(node);
        let assertion = Asserted {
            node: node.clone(),
            visible: self.declarations.visible(),
        };
        let found = parser::new_constants(&self.declarations, &assertion, &self.constants);
        if !found.is_empty() {
            let mut constants = (*self.constants).clone();
            constants.extend(
                found
                    .iter()
                    .map(|(name, value)| (name.clone(), value.clone())),
            );
            // Only an assertion naming a newly bound symbol can compile
            // differently; the others keep their formulas.
            let mut formulas = Vec::with_capacity(self.formulas.len() + 1);
            for (assertion, formula) in self.assertions.iter().zip(&self.formulas) {
                formulas.push(if parser::mentions(&assertion.node, &found) {
                    parser::formula(&self.declarations, &constants, assertion)?
                } else {
                    formula.clone()
                });
            }
            formulas.push(parser::formula(&self.declarations, &constants, &assertion)?);
            self.anchors = parser::anchors(&self.declarations, &constants);
            self.constants = Arc::new(constants);
            self.formulas = formulas;
        } else {
            let formula = parser::formula(&self.declarations, &self.constants, &assertion)?;
            self.formulas.push(formula);
        }
        self.assertions.push(assertion);
        self.length = length;
        self.tokens = tokens;
        self.source = self.source.assert(node);
        self.problem = OnceLock::new();
        Ok(())
    }
    pub(crate) fn problem(&self) -> &Arc<Problem> {
        self.problem.get_or_init(|| {
            Arc::new(parser::problem(
                &self.declarations,
                self.anchors.clone(),
                self.formulas.clone(),
            ))
        })
    }
    /// The normalized source this session stands for, hashed on demand.
    pub(crate) fn source(&self) -> &SourceHash {
        &self.source
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
