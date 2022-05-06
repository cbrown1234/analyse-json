use clap::Parser;
use flate2::read::GzDecoder;
use glob::glob;
use json::ndjson::{parse_json_iterable, parse_ndjson_bufreader, FileStats};
use jsonpath::Selector;
use std::error;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::PathBuf;

pub mod json;

type Result<T> = ::std::result::Result<T, Box<dyn error::Error>>;

#[derive(Parser, Default)]
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

impl Cli {
    fn jsonpath_selector(&self) -> Result<Option<Selector>> {
        let jsonpath_selector = if let Some(jsonpath) = &self.jsonpath {
            let selector = Selector::new(&jsonpath)?;
            Some(selector)
        } else {
            None
        };
        Ok(jsonpath_selector)
    }
}

// TODO: Does this need to be Box<dyn BufRead>? Could it be impl BufRead?
fn get_bufreader(_args: &Cli, file_path: &std::path::PathBuf) -> Result<Box<dyn BufRead + Send>> {
    let extension = file_path.extension().and_then(OsStr::to_str);
    let file = File::open(file_path)?;
    if extension == Some("gz") {
        let file = GzDecoder::new(file);
        Ok(Box::new(io::BufReader::new(file)))
    } else {
        Ok(Box::new(io::BufReader::new(file)))
    }
}

fn parse_ndjson_file_path(args: &Cli, file_path: &PathBuf) -> Result<FileStats> {
    let buf_reader = get_bufreader(args, file_path)?;
    Ok(parse_ndjson_bufreader(args, buf_reader)?)
}

// fn parse_stdin<'a>(args: &Cli, stdin: &'a mut StdinLock<'a>) -> impl Iterator<Item = io::Result<String>> + 'a {
//     let stdin_iter = stdin.lines();
//     let stdin_iter = if let Some(n) = args.lines {
//         stdin_iter.take(n)
//     } else {
//         stdin_iter.take(usize::MAX)
//     };
//     stdin_iter
// }

pub fn run(args: Cli) -> Result<()> {
    let stdin = io::stdin();
    let stdin_iter = stdin.lock().lines();

    // TODO: Impl line limit option
    if let Some(file_path) = &args.file_path {
        let file_stats = parse_ndjson_file_path(&args, file_path)?;
        println!("{}", file_stats);
        return Ok(());
    }
    if let Some(pattern) = &args.glob {
        println!("Glob '{}':", pattern);
        for entry in glob(&pattern)? {
            let path = entry?;
            println!("File '{}':", path.display());
            let file_stats = parse_ndjson_file_path(&args, &path)?;
            println!("{}", file_stats);
        }
        return Ok(());
    }

    // TODO: Refactor: parse borrow of args around to functions
    // TODO: Fix hang on empty stdin
    let stdin_file_stats = parse_json_iterable(&args, stdin_iter)?;
    if stdin_file_stats != FileStats::default() {
        // TODO: change output format depending on if writing to tty or stdout pipe (e.g. ripgrep)
        println!("{}", stdin_file_stats);
        return Ok(());
    }

    Ok(())
}
