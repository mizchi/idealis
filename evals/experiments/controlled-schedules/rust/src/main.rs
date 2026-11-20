use std::io::{self, BufRead, Write};
fn main() {
    let mut output = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        serde_json::to_writer(
            &mut output,
            &idealis_schedule_eval::dispatch(line.expect("read schedule").as_bytes()),
        )
        .expect("write schedule result");
        output.write_all(b"\n").expect("write newline");
        output.flush().expect("flush result");
    }
}
