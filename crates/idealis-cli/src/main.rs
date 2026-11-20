use std::io::{self, BufRead, Write};
fn main() {
    let mut engine = idealis_connect::Engine::default();
    let mut output = io::BufWriter::new(io::stdout().lock());
    for input in io::stdin().lock().lines() {
        let input = input.expect("read request");
        let reply = engine.dispatch(input.as_bytes());
        serde_json::to_writer(&mut output, &reply).expect("serialize response");
        output.write_all(b"\n").expect("write newline");
        output.flush().expect("flush response");
    }
}
