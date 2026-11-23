//! Times each command of the temporal corpus (plan 0090): the median of
//! seven runs and the work of one, as JSON lines. Not an adoption gate.
use std::time::Instant;

fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/alloy/temporal");
    let mut paths: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "als"))
        .collect();
    paths.sort();
    for path in paths {
        let model = ralloy::Model::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for (index, command) in model.commands().iter().enumerate() {
            let mut times = Vec::new();
            let mut analysis = None;
            for _ in 0..7 {
                let start = Instant::now();
                analysis = Some(model.analyze(index, 50_000_000).unwrap());
                times.push(start.elapsed().as_secs_f64() * 1e3);
            }
            times.sort_by(f64::total_cmp);
            let analysis = analysis.unwrap();
            println!(
                "{{\"model\":{:?},\"command\":{:?},\"steps\":{},\"status\":\"{:?}\",\"medianMs\":{:.3},\"work\":{}}}",
                path.file_stem().unwrap().to_string_lossy(),
                command.label,
                command.scope.steps.unwrap_or(0),
                analysis.status,
                times[3],
                analysis.stats.work
            );
        }
    }
}
