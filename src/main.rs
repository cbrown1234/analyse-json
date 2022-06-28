use analyse_json::Cli;
use clap::Parser;
use std::process;

fn main() {
    let args = Cli::parse();

    if let Err(e) = analyse_json::run(args) {
        eprintln!("Application error: {}", e);

        process::exit(1);
    }
}
