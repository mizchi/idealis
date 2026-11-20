mod modules;
mod tla;
mod tlc;

use std::process::ExitCode;

type Failure = (u8, String);
type Result<T> = std::result::Result<T, Failure>;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let result = match args.next().as_deref() {
        Some("tlc") => tlc::run(args),
        Some("tla") => tla::run(args),
        _ => Err((
            1,
            "Usage: idealis tlc [options] spec | idealis tla [-s] [-l] [-stat] spec...".into(),
        )),
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err((code, message)) => {
            eprintln!("{message}");
            ExitCode::from(code)
        }
    }
}
