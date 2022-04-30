use flate2::read::GzDecoder;
use glob::glob;
use json::ndjson::{parse_json_iterable, parse_ndjson_bufreader, FileStats};
use jsonpath::Selector;
use std::error::Error;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::PathBuf;
use clap::Parser;


pub mod json;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(parse(from_os_str))]
    file_path: Option<std::path::PathBuf>,

    #[clap(short, long)]
    glob: Option<String>,

    #[clap(short = 'n', long)]
    lines: Option<usize>,

    #[clap(long)]
    jsonpath: Option<String>,
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
    let stdin_iter = stdin.lock().lines();
    let stdin_iter = if let Some(n) = args.lines {
        stdin_iter.take(n)
    } else {
        stdin_iter.take(usize::MAX)
    }
    ;
    let selector;
    let jsonpath= if let Some(jsonpath) = args.jsonpath {
        selector = Selector::new(&jsonpath)?;
        Some(&selector)
    } else {
        None
    }
    ;
    let stdin_file_stats = parse_json_iterable(stdin_iter, jsonpath)?;
    if stdin_file_stats != FileStats::default() {
        // TODO: change output format depending on if writing to tty or stdout pipe (e.g. ripgrep)
        println!("{}", stdin_file_stats);
        return Ok(());
    }

    // TODO: Impl line limit option
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
