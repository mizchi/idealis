use super::{Access, Instance, Port, Source};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    interface: PathBuf,
    #[serde(default)]
    types: Vec<PathBuf>,
    body: PathBuf,
    ports: BTreeMap<String, Access>,
    contract: Option<BehaviorContract>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BehaviorContract {
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub guarantees: Vec<String>,
    #[serde(default)]
    pub exercises: Vec<String>,
}

pub(super) struct Checked {
    pub body: PathBuf,
    pub text: String,
    pub types: Vec<(PathBuf, String)>,
    pub ports: BTreeMap<String, Contract>,
    pub operations: Vec<String>,
    pub contract: Option<BehaviorContract>,
}

#[derive(Clone)]
pub(super) struct Contract {
    pub shape: rmodel::Shape,
    pub access: Access,
}

pub(super) fn load(path: &Path) -> Result<Checked, String> {
    if !path.to_string_lossy().ends_with(".component.json") {
        return Err(format!(
            "{}: expected a .component.json file",
            path.display()
        ));
    }
    let manifest: Manifest = serde_json::from_str(&read(path)?)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let interface = model_path(directory, &manifest.interface)?;
    let body = model_path(directory, &manifest.body)?;
    let text = read(&body)?;
    let types = manifest
        .types
        .iter()
        .map(|file| {
            let path = model_path(directory, file)?;
            let canonical = path
                .canonicalize()
                .map_err(|error| format!("{}: {error}", path.display()))?;
            Ok((canonical, read(&path)?))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut source = Source::default();
    source.append(interface.clone(), read(&interface)?, None)?;
    for (file, text) in &types {
        source.append(file.clone(), text.clone(), None)?;
    }
    let interface_model = source.located_model(&interface.to_string_lossy())?;
    let base = &interface_model.model;
    if !base.facts.is_empty()
        || !base.init.is_empty()
        || !base.fairness.is_empty()
        || !base.checks.is_empty()
        || !base.temporal.is_empty()
        || !base.transitions.is_empty()
        || !base.actions.is_empty()
        || !base.derivations.is_empty()
        || interface_model.declarations.iter().any(|(name, &offset)| {
            let is_port = base.relation(name).is_some_and(|relation| relation.var)
                && manifest.ports.contains_key(name);
            (source.fragment(offset).path == interface) != is_port
        })
    {
        return Err(format!(
            "{}: the interface declares only its port variables, model and scope; shared fragments declare types",
            interface.display()
        ));
    }
    let mut ports = BTreeMap::new();
    for (name, access) in manifest.ports {
        let relation = base
            .relation(&name)
            .filter(|relation| relation.var)
            .ok_or_else(|| {
                format!(
                    "{}: port {name} must name a state variable in {}",
                    path.display(),
                    interface.display()
                )
            })?;
        ports.insert(
            name,
            Contract {
                shape: relation.shape.clone(),
                access,
            },
        );
    }
    if base.relations.len() != ports.len() {
        return Err(format!(
            "{}: every state variable in the interface must be declared as a port",
            interface.display()
        ));
    }

    let namespace = std::iter::once("part".to_owned())
        .chain((0..).map(|index| format!("part{index}")))
        .find(|prefix| {
            !interface_model
                .declarations
                .keys()
                .any(|name| name.split_once('_').is_some_and(|(head, _)| head == prefix))
        })
        .expect("a fresh component namespace");
    let local = Instance {
        path: body.clone(),
        namespace: namespace.clone(),
        ports: ports
            .iter()
            .map(|(name, contract)| {
                (
                    name.clone(),
                    Port {
                        state: name.clone(),
                        access: contract.access,
                    },
                )
            })
            .collect(),
    };
    source.append(body.clone(), text.clone(), Some(local))?;
    let model = source.model(&path.to_string_lossy())?;
    let prefix = format!("{namespace}_");
    if let Some(contract) = &manifest.contract {
        contract
            .check_names(&model, &prefix)
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    let operations = model
        .actions
        .into_iter()
        .map(|action| {
            action
                .name
                .strip_prefix(&prefix)
                .expect("owned component action")
                .to_owned()
        })
        .collect();
    Ok(Checked {
        body,
        text,
        types,
        ports,
        operations,
        contract: manifest.contract,
    })
}

impl BehaviorContract {
    fn check_names(&self, model: &rmodel::Model, prefix: &str) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        for (names, exercise) in [
            (&self.assumptions, false),
            (&self.guarantees, false),
            (&self.exercises, true),
        ] {
            for name in names {
                if !seen.insert(name) {
                    return Err(format!("contract check {name} is listed more than once"));
                }
                let full_name = format!("{prefix}{name}");
                let kind = if exercise {
                    rmodel::CheckKind::Reachable
                } else {
                    rmodel::CheckKind::Invariant
                };
                let valid = model
                    .checks
                    .iter()
                    .any(|check| check.name == full_name && check.kind == kind)
                    || !exercise
                        && model
                            .transitions
                            .iter()
                            .any(|check| check.name == full_name);
                if !valid {
                    let expected = if exercise {
                        "reachable"
                    } else {
                        "invariant or transition"
                    };
                    return Err(format!(
                        "contract check {name} must name a local {expected} declaration"
                    ));
                }
            }
        }
        Ok(())
    }
}

impl Checked {
    pub fn bind(
        &self,
        namespace: String,
        mut bindings: BTreeMap<String, String>,
    ) -> Result<Instance, String> {
        let mut ports = BTreeMap::new();
        for (name, contract) in &self.ports {
            let state = bindings
                .remove(name)
                .ok_or_else(|| format!("{namespace} is missing port {name}"))?;
            ports.insert(
                name.clone(),
                Port {
                    state,
                    access: contract.access,
                },
            );
        }
        if let Some(name) = bindings.keys().next() {
            return Err(format!("{namespace} has no port {name}"));
        }
        Ok(Instance {
            path: self.body.clone(),
            namespace,
            ports,
        })
    }
}

fn model_path(directory: &Path, file: &Path) -> Result<PathBuf, String> {
    if file.extension().and_then(|extension| extension.to_str()) != Some("model") {
        return Err(format!(
            "expected a .model file: {}",
            directory.join(file).display()
        ));
    }
    Ok(directory.join(file))
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}

pub(super) fn shape_text(shape: &rmodel::Shape) -> String {
    fn mult(mult: rmodel::Mult) -> &'static str {
        match mult {
            rmodel::Mult::Set => "set",
            rmodel::Mult::Lone => "lone",
            rmodel::Mult::One => "one",
            rmodel::Mult::Some => "some",
        }
    }
    match shape {
        rmodel::Shape::Unary {
            sig,
            mult: cardinality,
        } => format!("{} {sig}", mult(*cardinality)),
        rmodel::Shape::Binary {
            from,
            to,
            mult: cardinality,
        } => format!("{from} -> {} {to}", mult(*cardinality)),
    }
}
