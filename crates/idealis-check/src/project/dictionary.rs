use super::{byte_offset, position};
use idealis_model::{CheckedModel, LocatedModel, VocabularyRole};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    id: String,
    terms: Vec<Term>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Term {
    id: String,
    kind: String,
    name: BTreeMap<String, String>,
    definition: BTreeMap<String, String>,
}

pub(super) struct Dictionary {
    id: String,
    terms: BTreeMap<String, Term>,
}

impl Dictionary {
    pub(super) fn read(path: &Path) -> Result<Self, String> {
        let error = |message: String| format!("{}: {message}", path.display());
        let text = std::fs::read_to_string(path).map_err(|e| error(e.to_string()))?;
        let document: Document = if path
            .extension()
            .is_some_and(|extension| extension == "ideal")
        {
            let vocabulary = idealis_model::parse_vocabulary(&text).map_err(|diagnostic| {
                let location = diagnostic.offset.map_or_else(
                    || path.display().to_string(),
                    |offset| position(path, &text[..byte_offset(&text, offset)]),
                );
                format!("{location}: {}", diagnostic.error)
            })?;
            Document {
                id: vocabulary.id,
                terms: vocabulary
                    .terms
                    .into_iter()
                    .map(|term| Term {
                        id: term.id,
                        kind: term.role.as_str().into(),
                        name: [("ja".into(), term.name.ja), ("en".into(), term.name.en)].into(),
                        definition: [
                            ("ja".into(), term.description.ja),
                            ("en".into(), term.description.en),
                        ]
                        .into(),
                    })
                    .collect(),
            }
        } else {
            serde_json::from_str(&text).map_err(|e| error(e.to_string()))?
        };
        if !idealis_model::valid_term_id(&document.id) {
            return Err(error(format!("invalid dictionary ID {:?}", document.id)));
        }
        let mut terms = BTreeMap::new();
        for term in document.terms {
            let id = term.id.clone();
            if !idealis_model::valid_term_id(&id) {
                return Err(error(format!("invalid term ID {id:?}")));
            }
            if VocabularyRole::from_name(&term.kind).is_none() {
                return Err(error(format!("{id}: unknown term kind {}", term.kind)));
            }
            if term.name.is_empty()
                || term.name.keys().ne(term.definition.keys())
                || term
                    .name
                    .values()
                    .chain(term.definition.values())
                    .any(|s| s.trim().is_empty())
            {
                return Err(error(format!(
                    "{id}: name and definition must have the same nonempty translations"
                )));
            }
            if terms.insert(id.clone(), term).is_some() {
                return Err(error(format!("duplicate dictionary term {id}")));
            }
        }
        Ok(Self {
            id: document.id,
            terms,
        })
    }

    pub(super) fn check(
        dictionary: Option<&Self>,
        located: &LocatedModel<CheckedModel>,
        position: impl Fn(usize) -> String,
    ) -> Result<(), String> {
        for (target, id) in &located.model.term_bindings {
            let location = || position(located.term_locations[target]);
            let dictionary = dictionary.ok_or_else(|| {
                format!("{}: term {id} requires a project dictionary", location())
            })?;
            let term = dictionary
                .terms
                .get(id)
                .ok_or_else(|| format!("{}: unknown dictionary term {id}", location()))?;
            let kind = located
                .model
                .term_target_kind(target)
                .expect("checked term target");
            let expected = &term.kind;
            if expected != kind {
                return Err(format!(
                    "{}: term {id} expects {expected}, but {target} is {kind}",
                    location()
                ));
            }
        }
        Ok(())
    }

    pub(super) fn report(&self, model: &CheckedModel) -> Value {
        let mut targets: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (target, id) in &model.term_bindings {
            targets.entry(id).or_default().push(target);
        }
        let terms: Vec<_> = self
            .terms
            .values()
            .map(|term| {
                let mut entry = json!(term);
                entry["targets"] =
                    json!(targets.get(term.id.as_str()).map_or(&[][..], Vec::as_slice));
                entry
            })
            .collect();
        json!({"id": self.id, "terms": terms})
    }
}
