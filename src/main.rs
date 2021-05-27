use itertools::sorted;
use serde_json::Value;
use std::fs::File;
use std::io::{self, prelude::*};
use structopt::StructOpt;

mod json;

fn main() {
    let args = Cli::from_args();

    let file = File::open(&args.file_path).expect("could not read file");
    let reader = io::BufReader::new(file);

    let mut keys_count = std::collections::HashMap::new();
    let mut line_count: i64 = 0;
    let mut bad_lines = Vec::new();

    for line in reader.lines() {
        line_count += 1;
        let line = line.expect(&format!("Failed to read line {}", line_count));

        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                bad_lines.push(line_count);
                continue;
            }
        };

        for key in v.paths().iter() {
            let counter = keys_count.entry(key.to_owned()).or_insert(0);
            *counter += 1;
        }
    }

    let keys = sorted(keys_count.keys());
    println!("Keys:\n{:#?}", keys);
    println!("Key value counts:\n{:#?}", keys_count);
    println!("Key occurance:");
    for (k, v) in keys_count {
        println!("{}: {}%", k, 100f64 * v as f64 / line_count as f64)
    }
    println!("Corrupted lines:");
    println!("{:?}", bad_lines);
}

trait Paths {
    fn paths(&self) -> Vec<String>;
}

impl Paths for Value {
    fn paths(&self) -> Vec<String> {
        json::paths::parse_json_paths(&self)
    }
}

#[derive(StructOpt)]
struct Cli {
    #[structopt(parse(from_os_str))]
    file_path: std::path::PathBuf,
}
