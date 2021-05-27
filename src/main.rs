use itertools::sorted;
use std::fs::File;
use structopt::StructOpt;

mod json;

fn main() {
    let args = Cli::from_args();

    let file = File::open(&args.file_path).expect("could not read file");

    let file_stats = json::ndjson::parse_ndjson_file(file);

    let keys = sorted(file_stats.keys_count.keys());
    println!("Keys:\n{:#?}", keys);
    println!("Key value counts:\n{:#?}", file_stats.keys_count);
    println!("Key occurance:");
    for (k, v) in file_stats.key_occurance() {
        println!("{}: {}%", k, v)
    }
    println!("Corrupted lines:");
    println!("{:?}", file_stats.bad_lines);
}

#[derive(StructOpt)]
struct Cli {
    #[structopt(parse(from_os_str))]
    file_path: std::path::PathBuf,
}
