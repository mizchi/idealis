use rtla::ast::Module;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub fn with_suffix(file: &str, suffix: &str) -> PathBuf {
    if file.ends_with(suffix) {
        file.into()
    } else {
        format!("{file}{suffix}").into()
    }
}

// WASI starts without a host working directory. The npm launcher supplies its guest path.
pub fn from_cwd(path: &Path) -> PathBuf {
    #[cfg(target_os = "wasi")]
    if let Some(cwd) = std::env::var_os("IDEALIS_CWD") {
        return PathBuf::from(cwd).join(path);
    }
    path.to_owned()
}

fn read_module(path: &Path, verbose: bool) -> Result<Module, String> {
    if verbose {
        println!("Parsing file {}", path.display());
    }
    let source = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let module = rtla::parse_module(&source).map_err(|e| format!("{}: {e}", path.display()))?;
    if path.file_stem() != Some(module.name.as_ref()) {
        return Err(format!(
            "{}: module name {} differs from file name",
            path.display(),
            module.name
        ));
    }
    Ok(module)
}

pub struct Bundle {
    pub root: Module,
    pub dependencies: Vec<Module>,
}

impl Bundle {
    pub fn load(path: &Path, verbose: bool) -> Result<Self, String> {
        let root = read_module(path, verbose)?;
        let dir = path.parent().unwrap_or(Path::new("."));
        let mut dependencies = Vec::new();
        let mut loaded = HashSet::new();
        let mut visiting = HashSet::new();
        visit(
            &root,
            dir,
            verbose,
            &mut visiting,
            &mut loaded,
            &mut dependencies,
        )?;
        Ok(Self { root, dependencies })
    }
}

fn visit(
    module: &Module,
    dir: &Path,
    verbose: bool,
    visiting: &mut HashSet<String>,
    loaded: &mut HashSet<String>,
    dependencies: &mut Vec<Module>,
) -> Result<(), String> {
    if !visiting.insert(module.name.clone()) {
        return Err(format!("Cyclic module import: {}", module.name));
    }
    if visiting.len() > 40 || loaded.len() >= 256 {
        return Err("Module bundle exceeds the frontend limit".into());
    }
    for name in module
        .extends
        .iter()
        .chain(module.instances.iter().map(|instance| &instance.module))
    {
        if loaded.contains(name) {
            continue;
        }
        let path = dir.join(format!("{name}.tla"));
        if matches!(
            name.as_str(),
            "Naturals" | "Integers" | "Sequences" | "FiniteSets" | "TLC"
        ) {
            if path.exists() {
                return Err(format!(
                    "Unsupported local override of standard module {name}"
                ));
            }
            continue;
        }
        let child = read_module(&path, verbose)?;
        visit(&child, dir, verbose, visiting, loaded, dependencies)?;
        dependencies.push(child);
    }
    visiting.remove(&module.name);
    loaded.insert(module.name.clone());
    Ok(())
}
