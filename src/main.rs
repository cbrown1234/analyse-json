use itertools::sorted;
use serde_json::Value;
use structopt::StructOpt;

fn main() {
    let args = Cli::from_args();

    let content = std::fs::read_to_string(&args.file_path).expect("could not read file");

    let mut keys_count = std::collections::HashMap::new();
    let mut line_count: i64 = 0;

    for line in content.lines() {
        let v: Value = serde_json::from_str(line).unwrap();

        for (key, _) in v.as_object().unwrap() {
            let counter = keys_count.entry(key.to_owned()).or_insert(0);
            *counter += 1;
        }
        line_count += 1;
    }

    let keys = sorted(keys_count.keys());
    println!("Keys:\n{:#?}", keys);
    println!("Key value counts:\n{:#?}", keys_count);
    for (k, v) in keys_count {
        println!("{}: {}%", k, 100f64 * v as f64 / line_count as f64)
    }
}

#[derive(StructOpt)]
struct Cli {
    #[structopt(parse(from_os_str))]
    file_path: std::path::PathBuf,
}
