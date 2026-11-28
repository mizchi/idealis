// Line input and output for the drivers on the JS target (plan 0105):
// MoonBit's core has no stdin, so Node's synchronous fs calls serve.

///|
extern "js" fn read_line_js() -> String =
  #|() => {
  #|  const fs = process.getBuiltinModule("fs");
  #|  const g = (globalThis.__idealis_check_input ??= { buffer: "", done: false });
  #|  while (!g.buffer.includes("\n") && !g.done) {
  #|    const chunk = Buffer.alloc(65536);
  #|    let n = 0;
  #|    try {
  #|      n = fs.readSync(0, chunk, 0, chunk.length, null);
  #|    } catch (error) {
  #|      if (error.code === "EAGAIN") continue;
  #|      throw error;
  #|    }
  #|    if (n === 0) g.done = true;
  #|    else g.buffer += chunk.toString("utf8", 0, n);
  #|  }
  #|  if (g.buffer.length === 0) return "\u0000";
  #|  const end = g.buffer.indexOf("\n");
  #|  const line = end < 0 ? g.buffer : g.buffer.slice(0, end);
  #|  g.buffer = end < 0 ? "" : g.buffer.slice(end + 1);
  #|  return line;
  #|}

///|
/// The next line of stdin, or `None` at its end.
pub fn read_line() -> String? {
  let line = read_line_js()
  if line == "\u0000" {
    None
  } else {
    Some(line)
  }
}

///|
/// Writes a line to stdout.
pub extern "js" fn write_line(line : String) -> Unit =
  #|(line) => { process.getBuiltinModule("fs").writeSync(1, line + "\n"); }

///|
/// The first command-line argument, or "".
pub extern "js" fn argument() -> String =
  #|() => process.argv[2] ?? ""
