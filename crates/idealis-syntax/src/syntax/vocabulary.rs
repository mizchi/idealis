use super::{Parser, tokens, valid_definition};
use crate::{Error, ParseDiagnostic};
use std::collections::{BTreeMap, BTreeSet};

pub struct Vocabulary {
    pub id: String,
    pub name: LocalizedText,
    pub terms: Vec<VocabularyTerm>,
}

pub struct LocalizedText {
    pub ja: String,
    pub en: String,
}

pub struct VocabularyTerm {
    pub id: String,
    pub name: LocalizedText,
    pub description: LocalizedText,
    pub role: VocabularyRole,
    pub input: Vec<String>,
    pub output: Vec<String>,
    pub compared_with: Vec<String>,
    pub code: Option<(String, String)>,
    offset: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VocabularyRole {
    Entity,
    State,
    Relation,
    Action,
    Check,
    Concept,
    Logic,
    Language,
    Artifact,
    Parser,
    Compiler,
    Checker,
}

impl VocabularyRole {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "entity" => Self::Entity,
            "state" => Self::State,
            "relation" => Self::Relation,
            "action" => Self::Action,
            "check" => Self::Check,
            "concept" => Self::Concept,
            "logic" => Self::Logic,
            "language" => Self::Language,
            "artifact" => Self::Artifact,
            "parser" => Self::Parser,
            "compiler" => Self::Compiler,
            "checker" => Self::Checker,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::State => "state",
            Self::Relation => "relation",
            Self::Action => "action",
            Self::Check => "check",
            Self::Concept => "concept",
            Self::Logic => "logic",
            Self::Language => "language",
            Self::Artifact => "artifact",
            Self::Parser => "parser",
            Self::Compiler => "compiler",
            Self::Checker => "checker",
        }
    }

    pub fn domain_kind(self) -> Option<&'static str> {
        matches!(
            self,
            Self::Entity
                | Self::State
                | Self::Relation
                | Self::Action
                | Self::Check
                | Self::Concept
        )
        .then(|| self.as_str())
    }

    fn is_tool(self) -> bool {
        matches!(self, Self::Parser | Self::Compiler | Self::Checker)
    }
}

pub fn parse_vocabulary(source: &str) -> Result<Vocabulary, ParseDiagnostic> {
    if source.len() > 1 << 20 {
        return Err(ParseDiagnostic {
            error: Error::syntax("source exceeds 1 MiB"),
            offset: None,
        });
    }
    let (tokens, offsets) = tokens(source)?;
    let mut parser = Parser {
        tokens,
        offsets,
        ..Parser::default()
    };
    let vocabulary = parser.vocabulary().map_err(|error| ParseDiagnostic {
        error,
        offset: Some(
            parser
                .offsets
                .get(parser.position)
                .copied()
                .unwrap_or_else(|| source.chars().count()),
        ),
    })?;
    vocabulary.check_links()?;
    Ok(vocabulary)
}

impl Parser {
    fn vocabulary(&mut self) -> Result<Vocabulary, Error> {
        self.expect("vocabulary")?;
        let id = self.term_id()?;
        self.expect("{")?;
        self.expect("name")?;
        self.expect("=")?;
        let name = self.localized_text()?;
        let mut terms = Vec::new();
        while !self.take("}") {
            let offset = self.offsets.get(self.position).copied();
            self.expect("term")?;
            terms.push(self.vocabulary_term(offset.expect("term has an offset"))?);
        }
        if !self.peek().is_empty() {
            return Err(Error::syntax("expected end of vocabulary"));
        }
        if terms.is_empty() {
            return Err(Error::syntax("vocabulary needs at least one term"));
        }
        Ok(Vocabulary { id, name, terms })
    }

    fn vocabulary_term(&mut self, offset: usize) -> Result<VocabularyTerm, Error> {
        let id = self.term_id()?;
        self.expect("{")?;
        let mut fields = BTreeSet::new();
        let (mut name, mut description, mut role, mut code) = (None, None, None, None);
        let (mut input, mut output, mut compared_with) = (Vec::new(), Vec::new(), Vec::new());
        while !self.take("}") {
            let field = self.name()?;
            if !fields.insert(field.clone()) {
                return Err(Error::syntax(format!("{id}: duplicate field {field}")));
            }
            self.expect("=")?;
            match field.as_str() {
                "name" => name = Some(self.localized_text()?),
                "description" => description = Some(self.localized_text()?),
                "kind" => {
                    let kind = self.peek();
                    role =
                        Some(VocabularyRole::from_name(kind).ok_or_else(|| {
                            Error::syntax(format!("{id}: unknown term kind {kind}"))
                        })?);
                    self.position += 1;
                }
                "input" => input = self.term_id_list()?,
                "output" => output = self.term_id_list()?,
                "compared_with" => compared_with = self.term_id_list()?,
                "code" => {
                    self.expect("{")?;
                    self.expect("path")?;
                    self.expect(":")?;
                    let path = self.vocabulary_text()?;
                    self.expect(",")?;
                    self.expect("symbol")?;
                    self.expect(":")?;
                    let symbol = self.vocabulary_text()?;
                    self.expect("}")?;
                    code = Some((path, symbol));
                }
                _ => return Err(Error::syntax(format!("{id}: unknown field {field}"))),
            }
        }
        let missing = |field| Error::syntax(format!("{id}: missing {field}"));
        let name = name.ok_or_else(|| missing("name"))?;
        let description = description.ok_or_else(|| missing("description"))?;
        let role = role.ok_or_else(|| missing("kind"))?;
        Ok(VocabularyTerm {
            id,
            name,
            description,
            role,
            input,
            output,
            compared_with,
            code,
            offset,
        })
    }

    fn vocabulary_text(&mut self) -> Result<String, Error> {
        let text = self.description_text()?;
        if !valid_definition(&text) {
            return Err(Error::syntax("expected nonempty printable text"));
        }
        Ok(text)
    }

    fn localized_text(&mut self) -> Result<LocalizedText, Error> {
        if !self.take("{") {
            let text = self.vocabulary_text()?;
            return Ok(LocalizedText {
                ja: text.clone(),
                en: text,
            });
        }
        let (mut ja, mut en) = (None, None);
        loop {
            let slot = match self.name()?.as_str() {
                "ja" => &mut ja,
                "en" => &mut en,
                other => {
                    return Err(Error::syntax(format!(
                        "unsupported vocabulary language {other}"
                    )));
                }
            };
            self.expect(":")?;
            if slot.replace(self.vocabulary_text()?).is_some() {
                return Err(Error::syntax("duplicate vocabulary language"));
            }
            if self.take("}") {
                break;
            }
            self.expect(",")?;
            if self.take("}") {
                break;
            }
        }
        let (Some(ja), Some(en)) = (ja, en) else {
            return Err(Error::syntax("vocabulary text requires ja and en"));
        };
        Ok(LocalizedText { ja, en })
    }

    fn term_id_list(&mut self) -> Result<Vec<String>, Error> {
        self.expect("[")?;
        let mut ids = Vec::new();
        while !self.take("]") {
            ids.push(self.term_id()?);
            if self.take("]") {
                break;
            }
            self.expect(",")?;
        }
        Ok(ids)
    }
}

impl Vocabulary {
    fn check_links(&self) -> Result<(), ParseDiagnostic> {
        let mut roles = BTreeMap::new();
        for term in &self.terms {
            if roles.insert(term.id.as_str(), term.role).is_some() {
                return Err(term.error("duplicate term ID"));
            }
        }
        for term in &self.terms {
            for (field, refs) in [
                ("input", &term.input),
                ("output", &term.output),
                ("compared_with", &term.compared_with),
            ] {
                if !refs.is_empty() && !term.role.is_tool() {
                    return Err(term.error(format!("{field} applies only to tools")));
                }
                let mut seen = BTreeSet::new();
                for id in refs {
                    if !seen.insert(id) {
                        return Err(term.error(format!("duplicate {field} reference {id}")));
                    }
                    let role = *roles
                        .get(id.as_str())
                        .ok_or_else(|| term.error(format!("unknown term {id}")))?;
                    let expected = match field {
                        "input"
                            if !matches!(
                                role,
                                VocabularyRole::Language | VocabularyRole::Artifact
                            ) =>
                        {
                            Some("a language or artifact")
                        }
                        "output" if role != VocabularyRole::Artifact => Some("an artifact"),
                        "compared_with" if role != term.role || id == &term.id => {
                            Some(match term.role {
                                VocabularyRole::Parser => "a parser other than this term",
                                VocabularyRole::Compiler => "a compiler other than this term",
                                _ => "a checker other than this term",
                            })
                        }
                        _ => None,
                    };
                    if let Some(expected) = expected {
                        return Err(term.error(format!("{field} requires {expected}, got {id}")));
                    }
                }
            }
        }
        Ok(())
    }
}

impl VocabularyTerm {
    fn error(&self, message: impl std::fmt::Display) -> ParseDiagnostic {
        ParseDiagnostic {
            error: Error::kind(format!("{}: {message}", self.id)),
            offset: Some(self.offset),
        }
    }
}
