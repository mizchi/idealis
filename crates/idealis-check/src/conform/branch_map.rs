//! Optional provenance for diagnostics over an IR file.
use crate::{ir, project};
use idealis_model::{Model, UpdateBranch};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const VERSION: u32 = 4;

fn is_false(value: &bool) -> bool {
    !value
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BranchMap {
    version: u32,
    ir_id: String,
    map_id: String,
    groups: BTreeMap<String, Vec<Group>>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Group {
    source: String,
    assignment: String,
    ancestors: Vec<bool>,
    #[serde(default, skip_serializing_if = "is_false")]
    value_conditional: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    then_unreachable: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    else_unreachable: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    expanded_sites: Vec<Site>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    nested_sites: Vec<Site>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Site {
    assignment: String,
    path: Vec<usize>,
}

pub(super) struct Locations {
    pub groups: BTreeMap<String, Vec<UpdateBranch>>,
    pub positions: BTreeMap<usize, String>,
    pub includes_assigned_values: bool,
}

fn digest(value: impl Serialize) -> String {
    let bytes = serde_json::to_vec(&value).expect("serializable branch map data");
    format!("{:x}", Sha256::digest(bytes))
}

impl BranchMap {
    fn identity(&self) -> String {
        digest((self.version, &self.ir_id, &self.groups))
    }

    fn restore(self, model: &Model) -> Result<Locations, String> {
        if !(1..=VERSION).contains(&self.version) {
            return Err(format!(
                "branch map version {} (want 1 through {VERSION})",
                self.version
            ));
        }
        if self.ir_id != digest(ir::to_ir(model)?) {
            return Err("branch map does not match the IR".into());
        }
        if self.map_id != self.identity() {
            return Err("branch map content does not match its mapId".into());
        }
        let mut positions = BTreeMap::new();
        let mut groups = BTreeMap::new();
        for (action, rows) in self.groups {
            if self.version == 1 && rows.iter().any(|row| row.value_conditional) {
                return Err("branch map version 1 cannot contain assigned value branches".into());
            }
            if rows.iter().any(|row| {
                !row.expanded_sites.is_empty() && (self.version < 3 || !row.value_conditional)
            }) {
                return Err("expanded binding sites require a version 3 value branch".into());
            }
            if rows.iter().any(|row| {
                !row.nested_sites.is_empty()
                    && (self.version < 4
                        || !row.value_conditional
                        || !row.expanded_sites.is_empty())
            }) {
                return Err("nested expression sites require a version 4 value branch".into());
            }
            if !model
                .actions
                .iter()
                .any(|candidate| candidate.name == action)
            {
                return Err(format!("branch map names an unknown action {action}"));
            }
            let branches = rows
                .into_iter()
                .map(|row| {
                    let id = positions.len();
                    positions.insert(id, row.source);
                    let binding = !row.expanded_sites.is_empty();
                    let sites = if binding {
                        row.expanded_sites
                    } else {
                        row.nested_sites
                    };
                    UpdateBranch::from_selector(id, row.assignment, row.ancestors)
                        .with_value_conditional(row.value_conditional)
                        .with_unreachable_reasons([row.then_unreachable, row.else_unreachable])
                        .with_value_sites(
                            sites
                                .into_iter()
                                .map(|site| (site.assignment, site.path))
                                .collect(),
                            binding,
                        )
                })
                .collect();
            groups.insert(action, branches);
        }
        Ok(Locations {
            groups,
            positions,
            includes_assigned_values: self.version >= 2,
        })
    }
}

/// Produce the optional diagnostic artifact for a source file or linked project.
pub fn generate(path: &str) -> Result<BranchMap, String> {
    let (located, positions) = project::load_update_branches(path)?;
    let groups: BTreeMap<String, Vec<Group>> = located
        .update_branches
        .into_iter()
        .map(|(action, branches)| {
            let rows = branches
                .iter()
                .map(|branch| {
                    let (assignment, ancestors) = branch.selector();
                    let [then_unreachable, else_unreachable] = branch.unreachable_reasons().clone();
                    let sites = || {
                        branch
                            .value_sites()
                            .iter()
                            .map(|(assignment, path)| Site {
                                assignment: assignment.clone(),
                                path: path.clone(),
                            })
                            .collect()
                    };
                    let (expanded_sites, nested_sites) = if branch.binding_name().is_some() {
                        (sites(), Vec::new())
                    } else {
                        (Vec::new(), sites())
                    };
                    Group {
                        source: positions[&branch.offset].clone(),
                        assignment: assignment.to_owned(),
                        ancestors: ancestors.to_vec(),
                        value_conditional: branch.is_value_conditional(),
                        then_unreachable,
                        else_unreachable,
                        expanded_sites,
                        nested_sites,
                    }
                })
                .collect();
            (action, rows)
        })
        .collect();
    let version = if groups
        .values()
        .flatten()
        .any(|row| !row.nested_sites.is_empty())
    {
        VERSION
    } else if groups
        .values()
        .flatten()
        .any(|row| !row.expanded_sites.is_empty())
    {
        3
    } else {
        2
    };
    let mut map = BranchMap {
        version,
        ir_id: digest(ir::to_ir(&located.model)?),
        map_id: String::new(),
        groups,
    };
    map.map_id = map.identity();
    Ok(map)
}

pub(super) fn load(
    path: &str,
    branch_map: Option<&str>,
) -> Result<(idealis_model::CheckedModel, Option<Locations>), String> {
    if path.ends_with(".json") && !project::is_manifest(path) {
        let (model, omitted) = ir::parse_checked(&project::load_source(path)?)?;
        if !omitted.is_empty() {
            return Err(
                "branch diagnostics require a complete model; the IR has adapter omissions".into(),
            );
        }
        let locations = branch_map
            .map(|path| {
                let file = std::fs::File::open(path).map_err(|error| format!("{path}: {error}"))?;
                let map: BranchMap = serde_json::from_reader(std::io::BufReader::new(file))
                    .map_err(|error| format!("{path}: {error}"))?;
                map.restore(&model)
            })
            .transpose()?;
        return Ok((model, locations));
    }
    if branch_map.is_some() {
        return Err("--branch-map requires a JSON IR input".into());
    }
    let (located, positions) = project::load_update_branches(path)?;
    Ok((
        located.model,
        Some(Locations {
            groups: located.update_branches,
            positions,
            includes_assigned_values: true,
        }),
    ))
}
