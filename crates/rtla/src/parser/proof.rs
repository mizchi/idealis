use super::{Parser, symbolic};
use crate::ParseError;
use crate::ast::{
    DeclarationLevel, Expression, ExpressionKind, Proof, ProofAssumption, ProofBindings,
    ProofReference, ProofStep, ProofStepKind, UseBody,
};
use crate::lexer::Kind;

impl Parser {
    pub(super) fn statement(&mut self) -> Result<Expression, ParseError> {
        self.nested_statement(false)
    }

    fn nested_statement(&mut self, allow_label: bool) -> Result<Expression, ParseError> {
        let labeled = allow_label
            && self.raw().kind == Kind::Name
            && self.peek(1).text == "::"
            && (self.peek(2).text == "ASSUME"
                || self.peek(2).text == "[]" && self.peek(3).text == "ASSUME");
        let boxed = self.at("[]") && self.peek(1).text == "ASSUME";
        if !labeled && !self.at("ASSUME") && !boxed {
            return self.expression(0);
        }
        if self.depth >= crate::MAX_NESTING {
            return Err(self.unsupported("Sequent nesting exceeds 64"));
        }
        self.depth += 1;
        let result = if labeled {
            self.labeled_statement()
        } else {
            self.assume_prove()
        };
        self.depth -= 1;
        result
    }

    fn labeled_statement(&mut self) -> Result<Expression, ParseError> {
        let token = self.advance()?;
        self.expect("::")?;
        Ok(Expression {
            location: token.location,
            node: ExpressionKind::Label {
                name: token.text,
                params: vec![],
                body: Box::new(self.nested_statement(true)?),
            },
        })
    }

    fn assume_prove(&mut self) -> Result<Expression, ParseError> {
        let location = self.raw().location;
        let is_boxed_assume = self.accept("[]");
        self.expect("ASSUME")?;
        let mut assumptions = vec![];
        loop {
            let is_new = self.accept("NEW");
            let level = match self.raw().text.as_str() {
                "CONSTANT" => Some(DeclarationLevel::Constant),
                "VARIABLE" => Some(DeclarationLevel::Variable),
                "STATE" => Some(DeclarationLevel::State),
                "ACTION" => Some(DeclarationLevel::Action),
                "TEMPORAL" => Some(DeclarationLevel::Temporal),
                _ => None,
            };
            if level.is_some() {
                self.advance()?;
            }
            if is_new || level.is_some() {
                let level = level.unwrap_or(DeclarationLevel::Constant);
                let declaration = self.declaration()?;
                if level == DeclarationLevel::Variable && declaration.arity != 0 {
                    return Err(self.error("NEW VARIABLE requires an identifier"));
                }
                let set = if self.accept("\\in") {
                    if level != DeclarationLevel::Constant || declaration.arity != 0 {
                        return Err(self.error("Only a constant identifier may have a NEW domain"));
                    }
                    Some(Box::new(self.expression(0)?))
                } else {
                    None
                };
                assumptions.push(ProofAssumption::New {
                    declaration,
                    level,
                    is_new,
                    set,
                });
            } else {
                assumptions.push(ProofAssumption::Expression {
                    expression: self.nested_statement(true)?,
                });
            }
            if !self.accept(",") {
                break;
            }
        }
        let is_boxed_prove = self.accept("[]");
        self.expect("PROVE")?;
        Ok(Expression {
            location,
            node: ExpressionKind::AssumeProve {
                assumptions,
                prove: Box::new(self.expression(0)?),
                is_boxed_assume,
                is_boxed_prove,
            },
        })
    }

    fn step_level(&self) -> Option<&str> {
        if self.raw().kind != Kind::Step {
            return None;
        }
        let end = self.raw().text.find('>')?;
        Some(&self.raw().text[1..end])
    }

    pub(super) fn begins_proof(&self, parent: Option<usize>) -> bool {
        if ["PROOF", "BY", "OBVIOUS", "OMITTED"]
            .iter()
            .any(|keyword| self.at(keyword))
        {
            return true;
        }
        match self.step_level() {
            Some("+") => true,
            Some("*") => parent.is_none(),
            Some(level) => level
                .parse::<usize>()
                .ok()
                .is_some_and(|level| parent.is_none_or(|parent| level > parent)),
            None => false,
        }
    }

    pub(super) fn proof(&mut self, parent: Option<usize>) -> Result<Proof, ParseError> {
        if self.depth >= crate::MAX_NESTING {
            return Err(self.unsupported("Proof nesting exceeds 64"));
        }
        self.depth += 1;
        let result = self.proof_inner(parent);
        self.depth -= 1;
        result
    }

    fn proof_inner(&mut self, parent: Option<usize>) -> Result<Proof, ParseError> {
        let location = self.raw().location;
        self.accept("PROOF");
        if self.accept("BY") {
            return Ok(Proof::By {
                usage: self.use_body(true)?,
                location,
            });
        }
        if self.accept("OBVIOUS") {
            return Ok(Proof::Obvious { location });
        }
        if self.accept("OMITTED") {
            return Ok(Proof::Omitted { location });
        }
        let level = match self.step_level() {
            Some("*" | "+") => parent
                .map_or(Some(0), |level| level.checked_add(1))
                .ok_or_else(|| self.error("Proof level exceeds integer range"))?,
            Some(level) => level
                .parse::<usize>()
                .map_err(|_| self.error("Invalid proof level"))?,
            None => return Err(self.error("Expected proof step or terminal proof")),
        };
        if parent.is_some_and(|parent| level <= parent) {
            return Err(self.error("Nested proof must have a higher level"));
        }
        let mut steps = vec![];
        loop {
            let valid = match self.step_level() {
                Some("*") => true,
                Some("+") => steps.is_empty(),
                Some(actual) => actual.parse::<usize>().ok() == Some(level),
                None => false,
            };
            if !valid {
                return Err(
                    self.error("Expected a step at the current proof level, ending with QED")
                );
            }
            let token = self.advance()?;
            let body = self.proof_step()?;
            let is_qed = matches!(body, ProofStepKind::Qed);
            let may_have_proof = matches!(
                body,
                ProofStepKind::Assert { .. }
                    | ProofStepKind::Case { .. }
                    | ProofStepKind::Pick { .. }
                    | ProofStepKind::Qed
            );
            let proof = if self.begins_proof(Some(level)) {
                if !may_have_proof {
                    return Err(self.error("This proof step does not take a proof"));
                }
                Some(Box::new(self.proof(Some(level))?))
            } else {
                None
            };
            steps.push(ProofStep {
                number: token.text,
                level,
                body,
                proof,
                location: token.location,
            });
            if is_qed {
                break;
            }
        }
        Ok(Proof::Steps { steps, location })
    }

    fn proof_bindings(&mut self) -> Result<ProofBindings, ParseError> {
        if self.bound_ahead() {
            return Ok(ProofBindings::Bounded {
                bounds: self.bounds()?,
            });
        }
        let mut names = vec![self.name()?.text];
        while self.accept(",") {
            names.push(self.name()?.text);
        }
        Ok(ProofBindings::Unbounded { names })
    }

    fn definition_ahead(&self) -> bool {
        let line = self.raw().location.line;
        self.tokens[self.cursor..]
            .iter()
            .take_while(|token| token.location.line == line && token.kind != Kind::Step)
            .any(|token| token.text == "==")
    }

    fn proof_step(&mut self) -> Result<ProofStepKind, ParseError> {
        if self.accept("QED") {
            return Ok(ProofStepKind::Qed);
        }
        if self.accept("USE") {
            return Ok(ProofStepKind::Use {
                usage: self.use_body(true)?,
            });
        }
        if self.accept("HIDE") {
            return Ok(ProofStepKind::Hide {
                usage: self.use_body(false)?,
            });
        }
        if self.at("INSTANCE") {
            return Ok(ProofStepKind::Instance {
                instance: self.instance()?,
            });
        }
        if self.accept("DEFINE") || self.definition_ahead() {
            let mut definitions = vec![];
            let mut instances = vec![];
            loop {
                if self.instance_ahead() {
                    instances.push(self.instance()?);
                } else {
                    definitions.push(self.definition()?);
                }
                if !self.definition_ahead() {
                    break;
                }
            }
            return Ok(ProofStepKind::Define {
                definitions,
                instances,
            });
        }
        if self.accept("HAVE") {
            return Ok(ProofStepKind::Have {
                expression: self.expression(0)?,
            });
        }
        if self.accept("TAKE") {
            return Ok(ProofStepKind::Take {
                bindings: self.proof_bindings()?,
            });
        }
        if self.accept("WITNESS") {
            let mut expressions = vec![self.expression(0)?];
            while self.accept(",") {
                expressions.push(self.expression(0)?);
            }
            return Ok(ProofStepKind::Witness { expressions });
        }
        if self.accept("PICK") {
            let bindings = self.proof_bindings()?;
            self.expect(":")?;
            return Ok(ProofStepKind::Pick {
                bindings,
                expression: self.expression(0)?,
            });
        }
        if self.accept("CASE") {
            return Ok(ProofStepKind::Case {
                expression: self.expression(0)?,
            });
        }
        let suffices = self.accept("SUFFICES");
        Ok(ProofStepKind::Assert {
            suffices,
            statement: self.statement()?,
        })
    }

    fn reference(&mut self) -> Result<ProofReference, ParseError> {
        if self.accept("MODULE") {
            return Ok(ProofReference::Module {
                name: self.name()?.text,
            });
        }
        let expression = if symbolic(self.raw())
            && matches!(self.peek(1).kind, Kind::Step | Kind::End | Kind::Keyword)
            || symbolic(self.raw()) && matches!(self.peek(1).text.as_str(), "," | "====" | "----")
        {
            let token = self.advance()?;
            Expression {
                node: ExpressionKind::Identifier { name: token.text },
                location: token.location,
            }
        } else {
            self.expression(0)?
        };
        Ok(ProofReference::Expression { expression })
    }

    pub(super) fn use_body(&mut self, allow_only: bool) -> Result<UseBody, ParseError> {
        let only = allow_only && self.accept("ONLY");
        let mut facts = vec![];
        if self.reference_ahead() {
            facts.push(self.reference()?);
            while self.accept(",") {
                facts.push(self.reference()?);
            }
        }
        let mut definitions = vec![];
        if self.accept("DEF") || self.accept("DEFS") {
            definitions.push(self.reference()?);
            while self.accept(",") {
                definitions.push(self.reference()?);
            }
        }
        Ok(UseBody {
            only,
            facts,
            definitions,
        })
    }

    fn reference_ahead(&self) -> bool {
        matches!(
            self.raw().kind,
            Kind::Name | Kind::Number | Kind::Decimal | Kind::String | Kind::Step
        ) || self.at("MODULE")
            || symbolic(self.raw())
            || matches!(
                self.raw().text.as_str(),
                "(" | "{" | "[" | "<<" | "@" | "IF" | "CASE" | "LET" | "CHOOSE" | "\\A" | "\\E"
            )
    }
}
