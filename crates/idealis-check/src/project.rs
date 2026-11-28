//! Compose ordered source fragments before passing them to the existing parser.
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
mod component;
mod contracts;
mod dictionary;
pub mod impact;
pub use contracts::check_contracts;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Project {
    fragments: Vec<FragmentInput>,
    exports: Option<Vec<String>>,
    dictionary: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum FragmentInput {
    Plain(PathBuf),
    Instance(Instance),
    Component(ComponentInstance),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentInstance {
    component: PathBuf,
    namespace: String,
    #[serde(default)]
    ports: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Instance {
    path: PathBuf,
    namespace: String,
    #[serde(default)]
    ports: BTreeMap<String, Port>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Port {
    state: String,
    access: Access,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Access {
    Read,
    Write,
}

/// Type-check a component in its declared interface context.
pub fn check_component(path: &str) -> Result<serde_json::Value, String> {
    let checked = component::load(Path::new(path))?;
    let ports: serde_json::Map<String, serde_json::Value> = checked.ports.into_iter().map(|(name, contract)| {
        let access = match contract.access {
            Access::Read => "read",
            Access::Write => "write",
        };
        (name, serde_json::json!({"shape": component::shape_text(&contract.shape), "access": access}))
    }).collect();
    let mut output =
        serde_json::json!({"status":"typed", "ports": ports, "operations": checked.operations});
    if let Some(contract) = checked.contract {
        output["contract"] = serde_json::json!(contract);
    }
    Ok(output)
}

pub fn is_manifest(path: &str) -> bool {
    path.ends_with(".project.json") || path.ends_with(".rmodel.json")
}

fn is_model_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("ideal" | "model")
    )
}

pub fn load_source(path: &str) -> Result<String, String> {
    let source = read_source(path)?;
    if source
        .fragments
        .iter()
        .any(|fragment| fragment.instance.is_some())
    {
        source.model(path)?;
    }
    Ok(source.text)
}

/// Load source, preserving the original file and position when parsing fails.
pub fn load_model_source(path: &str) -> Result<(idealis_model::Model, String), String> {
    load_checked_model_source(path).map(|(model, source)| (model.into_model(), source))
}

/// Load source, including project links, into the validated model boundary.
pub fn load_checked_model_source(
    path: &str,
) -> Result<(idealis_model::CheckedModel, String), String> {
    let source = read_source(path)?;
    let model = source.located_model(path)?.model;
    Ok((model, source.text))
}

/// Check data-value domains and locate each declaration in the original files.
pub fn check_refinements(
    path: &str,
    selected: Option<&str>,
    max_work: u32,
) -> Result<crate::refinements::Report, String> {
    let source = read_source(path)?;
    let located = source.located_model(path)?;
    let mut report = crate::refinements::check(&located.model, selected, max_work)?;
    for entry in &mut report.types {
        entry.location = located
            .declarations
            .get(&entry.name)
            .map(|&offset| source.position(offset));
    }
    Ok(report)
}

/// Export the dictionary with its bindings in this linked model.
pub fn dictionary_report(path: &str) -> Result<serde_json::Value, String> {
    let source = read_source(path)?;
    let dictionary = source
        .dictionary
        .as_ref()
        .ok_or_else(|| format!("{path}: no project dictionary"))?;
    let located = source.located_model(path)?;
    Ok(dictionary.report(&located.model))
}

/// Load the complete model with its checked list of exported actions.
pub fn load_public_binding_source(
    path: &str,
) -> Result<(idealis_model::Model, BTreeSet<String>), String> {
    let source = read_source(path)?;
    let model = source.model(path)?;
    let exports = source
        .exports
        .ok_or_else(|| format!("{path}: public bindings require a manifest with exports"))?;
    Ok((model, exports.into_iter().collect()))
}

pub(crate) fn load_update_branches(
    path: &str,
) -> Result<
    (
        idealis_model::LocatedModel<idealis_model::CheckedModel>,
        BTreeMap<usize, String>,
    ),
    String,
> {
    if path.ends_with(".json") && !is_manifest(path) {
        return Err("branches requires model source or a project manifest".into());
    }
    let source = read_source(path)?;
    let located = source.located_model(path)?;
    let positions = located
        .update_branches
        .values()
        .flatten()
        .map(|branch| (branch.offset, source.position(branch.offset)))
        .collect();
    Ok((located, positions))
}

#[derive(Default)]
struct Source {
    text: String,
    fragments: Vec<Fragment>,
    characters: usize,
    exports: Option<Vec<String>>,
    dictionary: Option<dictionary::Dictionary>,
    port_shapes: BTreeMap<String, BTreeMap<String, component::Contract>>,
    contracts: BTreeMap<String, component::BehaviorContract>,
}

struct Fragment {
    path: PathBuf,
    start: usize,
    start_char: usize,
    original: String,
    edits: Vec<NameEdit>,
    instance: Option<Instance>,
}

struct NameEdit {
    lowered: Range<usize>,
    original: Range<usize>,
}

impl Fragment {
    fn original_byte(&self, lowered: usize) -> usize {
        let i = self
            .edits
            .partition_point(|edit| edit.lowered.start <= lowered);
        let Some(last) = i.checked_sub(1).map(|i| &self.edits[i]) else {
            return lowered.min(self.original.len());
        };
        if lowered < last.lowered.end {
            last.original.start
        } else {
            (last.original.end + lowered - last.lowered.end).min(self.original.len())
        }
    }
}

impl Source {
    fn append(
        &mut self,
        path: PathBuf,
        original: String,
        instance: Option<Instance>,
    ) -> Result<(), String> {
        self.push(path, original, instance)?;
        self.text.push('\n');
        self.characters += 1;
        Ok(())
    }

    fn push(
        &mut self,
        path: PathBuf,
        original: String,
        instance: Option<Instance>,
    ) -> Result<(), String> {
        let start = self.text.len();
        let mut edits = Vec::new();
        if let Some(instance) = &instance {
            let slots = idealis_model::component_slots(&original).map_err(|diagnostic| {
                let byte = byte_offset(&original, diagnostic.offset.unwrap_or_default());
                format!(
                    "{}: {}",
                    position(&path, &original[..byte]),
                    diagnostic.error
                )
            })?;
            let mut cursor = 0;
            for (span, name) in slots {
                self.text.push_str(&original[cursor..span.start]);
                let lowered = self.text.len() - start;
                if let Some(port) = instance.ports.get(&name) {
                    self.text.push_str(&port.state);
                } else {
                    self.text.push_str(&instance.namespace);
                    self.text.push('_');
                    self.text.push_str(&name);
                }
                cursor = span.end;
                edits.push(NameEdit {
                    lowered: lowered..self.text.len() - start,
                    original: span,
                });
            }
            self.text.push_str(&original[cursor..]);
        } else {
            self.text.push_str(&original);
        }
        self.fragments.push(Fragment {
            path,
            start,
            start_char: self.characters,
            original,
            edits,
            instance,
        });
        self.characters += self.text[start..].chars().count();
        Ok(())
    }

    fn fragment(&self, offset: usize) -> &Fragment {
        let index = self
            .fragments
            .partition_point(|fragment| fragment.start_char <= offset);
        &self.fragments[index - 1]
    }

    fn position(&self, offset: usize) -> String {
        let fragment = self.fragment(offset);
        let byte = byte_offset(&self.text[fragment.start..], offset - fragment.start_char);
        let original = fragment.original_byte(byte);
        position(&fragment.path, &fragment.original[..original])
    }

    fn model(&self, path: &str) -> Result<idealis_model::Model, String> {
        Ok(self.located_model(path)?.model.into_model())
    }

    fn located_model(
        &self,
        path: &str,
    ) -> Result<idealis_model::LocatedModel<idealis_model::CheckedModel>, String> {
        let located = self.parse_model(path)?;
        dictionary::Dictionary::check(self.dictionary.as_ref(), &located, |offset| {
            self.position(offset)
        })?;
        Ok(located)
    }

    fn parse_model(
        &self,
        path: &str,
    ) -> Result<idealis_model::LocatedModel<idealis_model::CheckedModel>, String> {
        let located = idealis_model::parse_located_checked(&self.text).map_err(|diagnostic| {
            let location = diagnostic
                .offset
                .map_or_else(|| path.to_owned(), |offset| self.position(offset));
            format!("{location}: {}", diagnostic.error)
        })?;
        self.check_ownership(&located)?;
        if let Some(exports) = &self.exports {
            let mut seen = BTreeSet::new();
            for name in exports {
                if !seen.insert(name) {
                    return Err(format!("{path}: duplicate export {name}"));
                }
                if !located
                    .model
                    .actions
                    .iter()
                    .any(|action| action.name == *name)
                {
                    return Err(format!("{path}: export names unknown action {name}"));
                }
            }
        }
        Ok(located)
    }

    fn check_ownership(
        &self,
        located: &idealis_model::LocatedModel<idealis_model::CheckedModel>,
    ) -> Result<(), String> {
        let instances: BTreeMap<_, _> = self
            .fragments
            .iter()
            .filter_map(|f| {
                f.instance
                    .as_ref()
                    .map(|i| (i.namespace.as_str(), (i, &f.path)))
            })
            .collect();
        if instances.is_empty() {
            return Ok(());
        }
        let owner = |offset| {
            self.fragment(offset)
                .instance
                .as_ref()
                .map(|i| i.namespace.as_str())
        };
        for (name, &offset) in &located.declarations {
            let reserved = name
                .split_once('_')
                .map(|(prefix, _)| prefix)
                .filter(|prefix| instances.contains_key(prefix));
            if owner(offset) != reserved {
                return Err(format!(
                    "{}: {name} must be declared in its own namespace",
                    self.position(offset)
                ));
            }
        }

        let mut writers = BTreeMap::new();
        for (&namespace, &(instance, file)) in &instances {
            for (alias, port) in &instance.ports {
                let shared = located.model.relation(&port.state).filter(|r| {
                    r.var
                        && located
                            .declarations
                            .get(&port.state)
                            .is_some_and(|&offset| owner(offset).is_none())
                });
                let Some(shared) = shared else {
                    return Err(format!(
                        "{}: {namespace}.{alias} must bind a project state variable, got {}",
                        file.display(),
                        port.state
                    ));
                };
                if let Some(contract) = self
                    .port_shapes
                    .get(namespace)
                    .and_then(|ports| ports.get(alias))
                    && contract.shape != shared.shape
                {
                    return Err(format!(
                        "{}: {namespace}.{alias} requires {}, but {} has {}",
                        file.display(),
                        component::shape_text(&contract.shape),
                        port.state,
                        component::shape_text(&shared.shape)
                    ));
                }
                if port.access == Access::Write
                    && let Some(previous) = writers.insert(port.state.as_str(), namespace)
                {
                    return Err(format!(
                        "{}: {} has two write ports, {previous} and {namespace}",
                        file.display(),
                        port.state
                    ));
                }
            }
        }

        for action in &located.model.actions {
            let offset = located.declarations[&action.name];
            let source = owner(offset);
            for (target, _) in &action.assignments {
                let target_owner = owner(located.declarations[target])
                    .or_else(|| writers.get(target.as_str()).copied());
                if source != target_owner {
                    return Err(format!(
                        "{}: {} cannot write {target}",
                        self.position(offset),
                        action.name
                    ));
                }
            }
        }
        for (&offset, states) in &located.state_reads {
            let Some(instance) = &self.fragment(offset).instance else {
                continue;
            };
            for state in states {
                let state_owner = owner(located.declarations[state]);
                if state_owner != Some(instance.namespace.as_str())
                    && !instance.ports.values().any(|port| port.state == *state)
                {
                    return Err(format!(
                        "{}: {} cannot read {state}",
                        self.position(offset),
                        instance.namespace
                    ));
                }
            }
        }
        Ok(())
    }
}

fn byte_offset(text: &str, offset: usize) -> usize {
    text.char_indices()
        .nth(offset)
        .map_or(text.len(), |(byte, _)| byte)
}

fn position(path: &Path, prefix: &str) -> String {
    let line = prefix.bytes().filter(|&byte| byte == b'\n').count() + 1;
    let column = prefix
        .rsplit('\n')
        .next()
        .unwrap_or_default()
        .chars()
        .count()
        + 1;
    format!("{}:{line}:{column}", path.display())
}

fn valid_identifier(name: &str) -> bool {
    name.as_bytes()
        .first()
        .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
        && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

fn read_source(path: &str) -> Result<Source, String> {
    let entry = Path::new(path);
    let read = |file: &Path| {
        std::fs::read_to_string(file).map_err(|error| format!("{}: {error}", file.display()))
    };
    let mut source = Source::default();
    if !is_manifest(path) {
        source.push(entry.to_owned(), read(entry)?, None)?;
        return Ok(source);
    }
    let project: Project =
        serde_json::from_str(&read(entry)?).map_err(|error| format!("{path}: {error}"))?;
    if project.fragments.is_empty() {
        return Err(format!("{path}: expected at least one fragment"));
    }
    source.exports = project.exports;

    let directory = entry.parent().unwrap_or_else(|| Path::new("."));
    source.dictionary = project
        .dictionary
        .as_ref()
        .map(|file| dictionary::Dictionary::read(&directory.join(file)))
        .transpose()?;
    let mut files = BTreeMap::new();
    let mut namespaces = BTreeSet::new();
    let mut components = BTreeMap::<PathBuf, component::Checked>::new();
    let mut type_dependencies = Vec::new();
    for input in project.fragments {
        let (file, instance, text) = match input {
            FragmentInput::Plain(file) => (directory.join(file), None, None),
            FragmentInput::Instance(instance) => {
                (directory.join(&instance.path), Some(instance), None)
            }
            FragmentInput::Component(input) => {
                let descriptor = directory.join(&input.component);
                let checked = match components.entry(descriptor.clone()) {
                    std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        let checked = component::load(entry.key())?;
                        entry.insert(checked)
                    }
                };
                let instance = checked
                    .bind(input.namespace, input.ports)
                    .map_err(|error| format!("{path}: {error}"))?;
                type_dependencies.push((instance.namespace.clone(), descriptor));
                source
                    .port_shapes
                    .insert(instance.namespace.clone(), checked.ports.clone());
                if let Some(contract) = &checked.contract {
                    source
                        .contracts
                        .insert(instance.namespace.clone(), contract.clone());
                }
                (
                    checked.body.clone(),
                    Some(instance),
                    Some(checked.text.clone()),
                )
            }
        };
        if let Some(instance) = &instance {
            let namespace = &instance.namespace;
            if !valid_identifier(namespace) || namespace.contains('_') {
                return Err(format!("{path}: invalid namespace {namespace:?}"));
            }
            if !namespaces.insert(namespace.clone()) {
                return Err(format!("{path}: duplicate namespace {namespace}"));
            }
            for (alias, port) in &instance.ports {
                if !valid_identifier(alias) || !valid_identifier(&port.state) {
                    return Err(format!(
                        "{path}: invalid port {namespace}.{alias} -> {:?}",
                        port.state
                    ));
                }
            }
        }
        if !is_model_source(&file) {
            return Err(format!(
                "{path}: expected an .ideal or .model fragment: {}",
                file.display()
            ));
        }
        let canonical = file
            .canonicalize()
            .map_err(|error| format!("{}: {error}", file.display()))?;
        if instance.is_none()
            && files
                .insert(canonical.clone(), source.fragments.len())
                .is_some()
        {
            return Err(format!("{path}: duplicate fragment {}", file.display()));
        }
        let text = match text {
            Some(text) => text,
            None => read(&canonical)?,
        };
        source.append(file, text, instance)?;
    }
    for (namespace, descriptor) in type_dependencies {
        for (dependency, expected) in &components[&descriptor].types {
            let Some(&index) = files.get(dependency) else {
                return Err(format!(
                    "{path}: {namespace} requires a shared type fragment {}",
                    dependency.display()
                ));
            };
            if &source.fragments[index].original != expected {
                return Err(format!(
                    "{path}: {} changed while loading {namespace}",
                    dependency.display()
                ));
            }
        }
    }
    Ok(source)
}
