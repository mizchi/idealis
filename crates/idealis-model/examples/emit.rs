//! emit <model> <output-dir>: writes <name>.als and, per check,
//! <name>-<check>.tla/.cfg (plan 0091).
fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (path, output, builtins) = match arguments.as_slice() {
        [path, output] => (path, output, false),
        [path, output, flag] if flag == "--builtins" => (path, output, true),
        _ => panic!("usage: emit <model> <output-dir> [--builtins]"),
    };
    let model = idealis_model::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    std::fs::create_dir_all(output).unwrap();
    if idealis_model::alloy_supports(&model).is_ok() {
        std::fs::write(
            format!("{output}/{}.als", model.name),
            idealis_model::to_alloy(&model),
        )
        .unwrap();
    }
    let mut checks: Vec<&str> = model
        .checks
        .iter()
        .map(|check| check.name.as_str())
        .collect();
    checks.extend(model.temporal.iter().map(|check| check.name.as_str()));
    checks.extend(model.transitions.iter().map(|check| check.name.as_str()));
    if builtins {
        checks.push(idealis_model::DECLARATIONS);
        if model.has_values() {
            checks.push(idealis_model::SCOPE);
        }
    }
    for check in checks {
        let (source, config) = idealis_model::to_tla(&model, check).unwrap();
        // TLC wants the module name to match the file name.
        let module = format!("{}_{check}", model.name);
        let source = source.replacen(
            &format!("MODULE {} ", model.name),
            &format!("MODULE {module} "),
            1,
        );
        std::fs::write(format!("{output}/{module}.tla"), source).unwrap();
        std::fs::write(format!("{output}/{module}.cfg"), config).unwrap();
    }
}
