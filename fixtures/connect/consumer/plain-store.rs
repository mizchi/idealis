use std::io::{self, BufRead, Write};

fn main() -> io::Result<()> {
    let drop_writes = std::env::args().nth(1).as_deref() == Some("drop-writes");
    let mut cell = String::new();
    let mut output = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line == "clear" {
            cell = "0".into();
        } else if let Some(decimal) = line.strip_prefix("set ") {
            if !drop_writes {
                cell = decimal.into();
            }
        } else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, line));
        }
        writeln!(output, "{cell}")?;
        output.flush()?;
    }
    Ok(())
}
