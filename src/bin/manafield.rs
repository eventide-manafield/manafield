use manafield::cli;

fn main() {
    let command = match cli::parse_args(std::env::args().skip(1)) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("manafield: {error}");
            eprintln!();
            cli::print_help();
            std::process::exit(2);
        }
    };

    if let Err(error) = cli::run(command) {
        eprintln!("manafield: {error}");
        std::process::exit(1);
    }
}
