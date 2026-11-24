//! emit <model> <output-dir>: writes <name>.als and, per check,
//! <name>-<check>.tla/.cfg (plan 0091).
fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [path, output] = arguments.as_slice() else {
        panic!("usage: emit <model> <output-dir>");
    };
    let model = rmodel::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    std::fs::create_dir_all(output).unwrap();
    std::fs::write(
        format!("{output}/{}.als", model.name),
        rmodel::to_alloy(&model),
    )
    .unwrap();
    for check in &model.checks {
        let (source, config) = rmodel::to_tla(&model, &check.name).unwrap();
        // TLC wants the module name to match the file name.
        let module = format!("{}_{}", model.name, check.name);
        let source = source.replacen(
            &format!("MODULE {} ", model.name),
            &format!("MODULE {module} "),
            1,
        );
        std::fs::write(format!("{output}/{module}.tla"), source).unwrap();
        std::fs::write(format!("{output}/{module}.cfg"), config).unwrap();
    }
}
