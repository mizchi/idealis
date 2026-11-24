use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
fn main() {
    let sets = [
        ("unsupported-string-direct", "{\"only\"}"),
        ("unsupported-string-tuple", "{<<\"only\">>}"),
        ("unsupported-string-set", "{{\"only\"}}"),
        ("unsupported-record-direct", "{[field |-> 0]}"),
        ("unsupported-record-tuple", "{<<[field |-> 0]>>}"),
        ("unsupported-record-set", "{{[field |-> 0]}}"),
        ("set-strings", "{\"b\", \"a\", \"A\", \"aa\", \"B\", \"\"}"),
        ("set-strings-reversed", "{\"\", \"B\", \"aa\", \"A\", \"a\", \"b\"}"),
        ("set-records", "{[a |-> 2], [a |-> 1, b |-> 0], [a |-> 1]}"),
        ("set-record-fields", "{[zeta |-> 0], [alpha |-> 0]}"),
        ("set-nested-strings", "{<<\"b\">>, <<\"a\">>}"),
        ("set-sets-of-strings", "{{\"b\"}, {\"a\"}}"),
        ("set-functions-with-strings", "{[i \\in {0} |-> \"b\"], [i \\in {0} |-> \"a\"]}"),
        ("set-functions-with-records", "{[i \\in {0} |-> [a |-> 0]]}"),
    ];
    let bodies = [
        ("append", "<<>>", "Append(a, b)"),
        ("plus-one", "0", "a + 1"),
        ("union", "{}", "a \\union {b}"),
        ("union-left", "{}", "{<<b>>} \\cup a"),
        ("union-nonset-init", "<<>>", "a \\union {b}"),
        ("reads-acc", "{}", "a \\union {<<b, Cardinality(a)>>}"),
    ];
    let config = rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Prop\nCHECK_DEADLOCK FALSE").unwrap();
    let mut accepted = 0;
    let mut refused = 0;
    let mut other = 0;
    let mut differing = 0;
    for (set_name, set) in sets {
        for (body_name, init, body) in bodies {
            let source = format!("---- MODULE Differential ----\nEXTENDS Integers, Sequences, FiniteSets, Apalache\nVARIABLE x\nOp(a, b) == {body}\nInit == x = (ApaFoldSet(Op, {init}, {set}))\nNext == UNCHANGED x\nProp == x # x\n====");
            let module = rtla::parse_module(&source).unwrap();
            let mut outcomes = vec![];
            for version in [IrVersion::V1, IrVersion::V2] {
                let program = compile_with_options(&module, &config, CompileOptions { irVersion: version }).unwrap();
                let result = program.check(rtlc::SafetyBounds { max_depth: 10, max_states: 100, max_work: 1_000_000, max_temporal_nodes: None });
                outcomes.push(match result {
                    Ok(r) => format!("ok:{}:{}", r.status, r.work),
                    Err(e) => format!("err:{}", e.message),
                });
            }
            let first = &outcomes[0];
            let kind = if first.starts_with("ok:") && !first.starts_with("ok:bounded:") { accepted += 1; "accepted" } else if first.contains("set fold order") { refused += 1; "refused" } else { other += 1; "other" };
            let same = outcomes[0] == outcomes[1];
            if !same { differing += 1; }
            println!("{set_name}\t{body_name}\t{kind}\tv1v2same={same}\tv1={}\tv2={}", outcomes[0], outcomes[1]);
        }
    }
    println!("TOTAL accepted={accepted} refused={refused} other={other} v1v2differ={differing}");
}
