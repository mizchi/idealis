//! Compose ordered source fragments before passing them to the existing parser.
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Project {
    fragments: Vec<PathBuf>,
}

pub fn is_manifest(path: &str) -> bool {
    path.ends_with(".rmodel.json")
}

pub fn load_source(path: &str) -> Result<String, String> {
    let entry = Path::new(path);
    let read = |file: &Path| {
        std::fs::read_to_string(file).map_err(|error| format!("{}: {error}", file.display()))
    };
    if !is_manifest(path) {
        return read(entry);
    }
    let project: Project =
        serde_json::from_str(&read(entry)?).map_err(|error| format!("{path}: {error}"))?;
    if project.fragments.is_empty() {
        return Err(format!("{path}: expected at least one fragment"));
    }

    let directory = entry.parent().unwrap_or_else(|| Path::new("."));
    let mut files = BTreeSet::new();
    let mut source = String::new();
    for fragment in project.fragments {
        if fragment
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("model")
        {
            return Err(format!(
                "{path}: expected a .model fragment: {}",
                fragment.display()
            ));
        }
        let file = directory.join(fragment);
        let canonical = file
            .canonicalize()
            .map_err(|error| format!("{}: {error}", file.display()))?;
        if !files.insert(canonical.clone()) {
            return Err(format!("{path}: duplicate fragment {}", file.display()));
        }
        source.push_str(&read(&canonical)?);
        source.push('\n');
    }
    Ok(source)
}
