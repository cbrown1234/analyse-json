use analyse_json::Cli;
use std::process;
use structopt::StructOpt;

fn main() {
    let args = Cli::from_args();

    if let Err(e) = analyse_json::run(args) {
        eprintln!("Application error: {}", e);

        process::exit(1);
    }
}
