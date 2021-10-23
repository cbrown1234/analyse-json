use glob::glob;
use std::error::Error;
use std::fs::File;
use structopt::StructOpt;

pub mod json;

#[derive(StructOpt)]
pub struct Cli {
    #[structopt(parse(from_os_str))]
    file_path: Option<std::path::PathBuf>,

    #[structopt(short, long)]
    glob: Option<String>,
}

pub fn run(args: Cli) -> Result<(), Box<dyn Error>> {
    if let Some(file_path) = args.file_path {
        let file = File::open(file_path)?;
        let file_stats = json::ndjson::parse_ndjson_file(file);
        println!("{}", file_stats);
        return Ok(());
    }
    if let Some(pattern) = args.glob {
        for entry in glob(&pattern)? {
            match entry {
                Ok(path) => {
                    println!("File '{}':", path.display());
                    let file = File::open(path)?;
                    let file_stats = json::ndjson::parse_ndjson_file(file);
                    println!("{}", file_stats);
                }
                Err(e) => eprintln!("Error reading glob entry: {:?}", e),
            }
        }
    }

    Ok(())
}
