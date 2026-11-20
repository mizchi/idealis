use std::io::{self, BufRead, Write};
fn main() -> io::Result<()> {
    let mut engine = rusmt::Engine::default();
    let mut output = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let line = line?;
        serde_json::to_writer(&mut output, &engine.dispatch(line.as_bytes()))?;
        writeln!(output)?;
        output.flush()?;
    }
    Ok(())
}
