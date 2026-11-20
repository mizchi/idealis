use super::{
    Result,
    modules::{Bundle, from_cwd, with_suffix},
};
pub fn run(args: impl Iterator<Item = String>) -> Result<u8> {
    let mut args = args.peekable();
    let mut semantics = true;
    let mut levels = true;
    while args.peek().is_some_and(|s| s.starts_with('-')) {
        match args.next().unwrap().as_str() {
            "-s" | "-S" => semantics = !semantics,
            // The v1.7.4 frontEndStatistics hook has an empty implementation.
            "-stat" | "-STAT" => {}
            "-l" | "-L" => levels = !levels,
            flag => return Err((255, format!("Unsupported SANY option: {flag}"))),
        }
    }
    if args.peek().is_none() {
        return Err((255, "Usage: idealis tla [-s] [-l] [-stat] spec...".into()));
    }
    for file in args {
        let bundle =
            Bundle::load(&from_cwd(&with_suffix(&file, ".tla")), true).map_err(|msg| (255, msg))?;
        if semantics {
            println!("Semantic processing of module {}", bundle.root.name);
            if let Err(error) =
                rtla_compiler::analyze_modules(&bundle.root, &bundle.dependencies, levels)
            {
                if let Some(name) = error
                    .message
                    .strip_prefix("unresolved module identifier ")
                    .or_else(|| error.message.strip_prefix("unresolved module operator "))
                {
                    // SANY v1.7.4 reports semantic diagnostics on stdout and exits 0.
                    println!("Semantic errors:\nUnknown operator: `{name}'.");
                } else if error.message.starts_with("Multiply-defined symbol")
                    || error.message.starts_with("level error at line ")
                    || error
                        .message
                        .starts_with("level error: recursive operator parameters")
                {
                    println!("Semantic errors:\n{}", error.message);
                } else {
                    return Err((255, error.to_string()));
                }
            }
        }
    }
    Ok(0)
}
