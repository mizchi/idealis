//! emit <quint-ir.json> <invariant> <output-dir>: writes <Module>.tla and
//! <Module>.cfg translated by rquint.
fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [ir, invariant, output] = arguments.as_slice() else {
        panic!("usage: emit <quint-ir.json> <invariant> <output-dir>");
    };
    let translation = rquint::translate(
        &std::fs::read_to_string(ir).unwrap(),
        &rquint::Options {
            invariants: vec![invariant.clone()],
            ..rquint::Options::default()
        },
    )
    .unwrap();
    let name = translation
        .source
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(2))
        .unwrap()
        .to_owned();
    std::fs::create_dir_all(output).unwrap();
    std::fs::write(format!("{output}/{name}.tla"), &translation.source).unwrap();
    std::fs::write(format!("{output}/{name}.cfg"), &translation.config).unwrap();
}
