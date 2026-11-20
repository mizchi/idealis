use idealis_fuzz_eval::{Mode, exercise, known_fault};
use serde::Deserialize;
use std::io::{self, BufRead, Write};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    mode: Mode,
    bytes: Vec<u8>,
}
fn main() {
    if std::env::args().nth(1).as_deref() == Some("control") {
        known_fault();
        return;
    }
    let mut output = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let input: Input = serde_json::from_str(&line.expect("read input")).expect("decode input");
        serde_json::to_writer(&mut output, &exercise(&input.bytes, input.mode, true))
            .expect("write audit report");
        output.write_all(b"\n").expect("newline");
        output.flush().expect("flush audit");
    }
}
