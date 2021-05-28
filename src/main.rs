use std::fs::File;
use structopt::StructOpt;

mod json;

fn main() {
    let args = Cli::from_args();

    let file = File::open(&args.file_path).expect("could not read file");

    let file_stats = json::ndjson::parse_ndjson_file(file);

    println!("{}", file_stats);
}

#[derive(StructOpt)]
struct Cli {
    #[structopt(parse(from_os_str))]
    file_path: std::path::PathBuf,
}
