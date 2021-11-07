use analyse_json::Cli;
use humantime::format_duration;
use std::{process, time::Instant};
use structopt::StructOpt;

fn main() {
    let args = Cli::from_args();

    let now = Instant::now();
    if let Err(e) = analyse_json::run(args) {
        eprintln!("Application error: {}", e);

        process::exit(1);
    }
    println!("Completed in {}", format_duration(now.elapsed()));
}
