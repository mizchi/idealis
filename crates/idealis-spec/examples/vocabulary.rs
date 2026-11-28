use idealis_spec::{compile_vocabulary, from_json, vocabulary_uses};

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [path, options @ ..] = args.as_slice() else {
        return Err("usage: vocabulary SOURCE [--uses TERM]".into());
    };
    let read = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let module = compile_vocabulary(&read).map_err(|error| format!("{path}:{error}"))?;
    let result = match options {
        [] => module,
        [flag, id] if flag == "--uses" => vocabulary_uses(&from_json(&module.to_string())?, id)?,
        _ => return Err("usage: vocabulary SOURCE [--uses TERM]".into()),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&result).expect("JSON value")
    );
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
