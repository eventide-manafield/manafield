use std::env;
use std::path::Path;

fn main() {
    if let Err(error) = run() {
        eprintln!("manafield-build-plan: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let input = args
        .next()
        .ok_or("usage: manafield-build-plan <instance.yaml> [output.json] [ci-output-dir]")?;
    let output = args.next();
    let ci_output_dir = args.next();

    if args.next().is_some() {
        return Err(
            "usage: manafield-build-plan <instance.yaml> [output.json] [ci-output-dir]".into(),
        );
    }

    manafield::build_plan::resolve(
        Path::new(&input),
        output.as_deref().map(Path::new),
        ci_output_dir.as_deref().map(Path::new),
    )
}
