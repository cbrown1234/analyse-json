use flate2::read::GzDecoder;
use glob::glob;
use json::ndjson::{parse_json_iterable, parse_ndjson_bufreader, FileStats};
use std::error::Error;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::PathBuf;
use structopt::StructOpt;

pub mod json;

#[derive(StructOpt)]
pub struct Cli {
    #[structopt(parse(from_os_str))]
    file_path: Option<std::path::PathBuf>,

    #[structopt(short, long)]
    glob: Option<String>,
}

fn get_bufreader(file_path: std::path::PathBuf) -> Result<Box<dyn BufRead + Send>, Box<dyn Error>> {
    let path = file_path.clone();
    let extension = path.extension().and_then(OsStr::to_str);
    let file = File::open(file_path)?;
    if extension == Some("gz") {
        let file = GzDecoder::new(file);
        Ok(Box::new(io::BufReader::new(file)))
    } else {
        Ok(Box::new(io::BufReader::new(file)))
    }
}

fn parse_ndjson_file_path(file_path: PathBuf) -> Result<FileStats, Box<dyn Error>> {
    let buf_reader = get_bufreader(file_path)?;
    Ok(parse_ndjson_bufreader(buf_reader)?)
}

pub fn run(args: Cli) -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let stdin_file_stats = parse_json_iterable(stdin.lock().lines())?;
    if stdin_file_stats != FileStats::default() {
        println!("{}", stdin_file_stats);
        return Ok(());
    }

    if let Some(file_path) = args.file_path {
        let file_stats = parse_ndjson_file_path(file_path)?;
        println!("{}", file_stats);
        return Ok(());
    }
    if let Some(pattern) = args.glob {
        println!("Glob '{}':", pattern);
        for entry in glob(&pattern)? {
            let path = entry?;
            println!("File '{}':", path.display());
            let file_stats = parse_ndjson_file_path(path)?;
            println!("{}", file_stats);
        }
        return Ok(());
    }

    Ok(())
}
