use crate::Spec;
use serde_json::{Value, json};

/// Lower a checked vocabulary to the same JSON module read by `from_json`.
pub fn compile_vocabulary(source: &str) -> Result<Value, String> {
    let vocabulary = idealis_syntax::parse_vocabulary(source).map_err(|diagnostic| {
        diagnostic.offset.map_or_else(
            || diagnostic.error.to_string(),
            |offset| {
                let (mut line, mut column) = (1, 1);
                for c in source.chars().take(offset) {
                    if c == '\n' {
                        line += 1;
                        column = 1;
                    } else {
                        column += 1;
                    }
                }
                format!("{line}:{column}: {}", diagnostic.error)
            },
        )
    })?;
    let group = &vocabulary.name.ja;
    let terms: Vec<_> = vocabulary
        .terms
        .iter()
        .map(|term| {
            let code: Vec<_> = term
                .code
                .iter()
                .map(|(path, symbol)| json!({"path": path, "symbol": symbol}))
                .collect();
            let kind = term.role.domain_kind();
            let mut entry = json!({
                "id": term.id,
                "kind": kind.unwrap_or("concept"),
                "group": group,
                "name": {"ja": term.name.ja, "en": term.name.en},
                "definition": {"ja": term.description.ja, "en": term.description.en},
                "code": code,
            });
            if kind.is_none() {
                entry["flow"] = json!({
                    "role": term.role.as_str(),
                    "input": term.input,
                    "output": term.output,
                    "comparedWith": term.compared_with,
                });
            }
            entry
        })
        .collect();
    Ok(json!({"spec": {
        "id": vocabulary.id,
        "name": {"ja": group, "en": vocabulary.name.en},
        "groups": [group],
        "terms": terms,
    }}))
}

/// Invert the typed tool references for a vocabulary ID.
pub fn vocabulary_uses(spec: &Spec, id: &str) -> Result<Value, String> {
    if !spec.terms.iter().any(|term| term.id == id) {
        return Err(format!("unknown term {id}"));
    }
    let mut producers = Vec::new();
    let mut consumers = Vec::new();
    let mut compared_by = Vec::new();
    for term in &spec.terms {
        if let Some(flow) = &term.flow {
            if flow.input.iter().any(|input| input == id) {
                consumers.push(&term.id);
            }
            if flow.output.iter().any(|output| output == id) {
                producers.push(&term.id);
            }
            if flow.compared_with.iter().any(|other| other == id) {
                compared_by.push(&term.id);
            }
        }
    }
    Ok(json!({"id": id, "producers": producers, "consumers": consumers, "comparedBy": compared_by}))
}
