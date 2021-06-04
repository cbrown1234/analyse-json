use std::error::Error;
use std::fs::File;
use structopt::StructOpt;

pub mod json;

#[derive(StructOpt)]
pub struct Cli {
    #[structopt(parse(from_os_str))]
    file_path: std::path::PathBuf,
}

pub fn run(args: Cli) -> Result<(), Box<dyn Error>> {
    let file = File::open(&args.file_path)?;

    let file_stats = json::ndjson::parse_ndjson_file(file);

    println!("{}", file_stats);

    Ok(())
}
