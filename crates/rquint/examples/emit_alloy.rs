//! emit_alloy <quint-ir.json> <invariant> <output.als>: rquint's Alloy
//! lowering with the default bounds (plan 0095).
fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [ir, invariant, output] = arguments.as_slice() else {
        panic!("usage: emit_alloy <quint-ir.json> <invariant> <output.als>");
    };
    let source = rquint::to_alloy(
        &std::fs::read_to_string(ir).unwrap(),
        &rquint::Options {
            invariants: vec![invariant.clone()],
            ..rquint::Options::default()
        },
        rquint::Bounds::default(),
    )
    .unwrap();
    std::fs::write(output, source).unwrap();
}
