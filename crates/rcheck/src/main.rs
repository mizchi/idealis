//! rcheck <model> [--checker auto|reference|explicit|sat|all] [--question NAME]
//! Answers every question of an rmodel model, or one, and prints JSON. Exits
//! 1 when two definite answers disagree, 2 on a usage or model error.
use rcheck::Selection;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mut path = None;
    let mut selection = Selection::Auto;
    let mut only = None;
    let mut rest = arguments.iter();
    while let Some(argument) = rest.next() {
        match argument.as_str() {
            "--checker" => {
                selection = match rest.next().map(String::as_str) {
                    Some("auto") => Selection::Auto,
                    Some("all") => Selection::All,
                    Some(name) => Selection::Named(name.to_owned()),
                    None => usage(),
                }
            }
            "--question" => only = Some(rest.next().cloned().unwrap_or_else(|| usage())),
            _ if path.is_none() => path = Some(argument.clone()),
            _ => usage(),
        }
    }
    let path = path.unwrap_or_else(|| usage());
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| fail(&error.to_string()));
    let model = rmodel::parse(&source).unwrap_or_else(|error| fail(&error.to_string()));
    let names: Vec<String> = match only {
        Some(name) => vec![name],
        None => rcheck::questions(&model)
            .into_iter()
            .map(|question| question.name)
            .collect(),
    };
    let mut reports = Vec::new();
    for name in names {
        match rcheck::check(&model, &name, &selection, rcheck::Budget::default()) {
            Ok(report) => reports.push(report),
            Err(error) => fail(&error.to_string()),
        }
    }
    println!("{}", serde_json::to_string_pretty(&reports).expect("JSON"));
    if reports.iter().any(|report| report.disagreement) {
        std::process::exit(1);
    }
}

fn usage() -> ! {
    fail("usage: rcheck <model> [--checker auto|reference|explicit|sat|all] [--question NAME]")
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}
