use clap::CommandFactory;
use clap::Parser;
use flate2::read::GzDecoder;
use glob::glob;
use grep_cli::is_readable_stdin;
use humantime::format_duration;
use json::ndjson::{parse_json_iterable, parse_ndjson_bufreader, FileStats};
use jsonpath_lib::Compiled;
use std::error;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::time::Instant;

pub mod json;

type Result<T> = ::std::result::Result<T, Box<dyn error::Error>>;

#[derive(Parser, Default, PartialEq)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    /// File to process, expected to contain a single JSON object or Newline Delimited (ND) JSON objects
    #[clap(parse(from_os_str))]
    file_path: Option<std::path::PathBuf>,

    /// Process all files identified by this glob pattern
    #[clap(short, long)]
    glob: Option<String>,

    /// Limit inspection to the first n lines
    #[clap(short = 'n', long)]
    lines: Option<usize>,

    /// JSONpath query to filter/limit the inspection to
    #[clap(long)]
    jsonpath: Option<String>,

    /// Walk the elements of arrays?
    #[clap(long)]
    explode_arrays: bool,

    /// Include combined results for all files when using glob
    #[clap(long)]
    merge: bool,
}

impl Cli {
    fn jsonpath_selector(&self) -> Result<Option<Compiled>> {
        let jsonpath_selector = if let Some(jsonpath) = &self.jsonpath {
            let selector = Compiled::compile(jsonpath)?;
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
    parse_ndjson_bufreader(args, buf_reader)
}

fn run_stdin(args: Cli) -> Result<()> {
    let stdin = io::stdin().lock();
    let stdin_file_stats = parse_json_iterable(&args, stdin.lines())?;

    // TODO: change output format depending on if writing to tty or stdout pipe (e.g. ripgrep)
    println!("{}", stdin_file_stats);

    Ok(())
}

fn run_no_stdin(args: Cli) -> Result<()> {
    if let Some(file_path) = &args.file_path {
        let file_stats = parse_ndjson_file_path(&args, file_path)?;
        file_stats.print()?;
        return Ok(());
    }

    if let Some(pattern) = &args.glob {
        let mut file_stats_list = Vec::new();

        println!("Glob '{}':", pattern);
        for entry in glob(pattern)? {
            let path = entry?;
            println!("File '{}':", path.display());
            let file_stats = parse_ndjson_file_path(&args, &path)?;
            println!("{}", file_stats);
            if args.merge {
                file_stats_list.push(file_stats)
            }
        }
        if args.merge {
            println!("Overall Stats");
            let overall_file_stats: FileStats = file_stats_list.iter().sum();
            // TODO: Fix handling of corrupt & empty lines
            println!("{}", overall_file_stats);
        }
        return Ok(());
    }
    Ok(())
}

pub fn run(args: Cli) -> Result<()> {
    let now = Instant::now();
    if is_readable_stdin() {
        run_stdin(args)?;
    } else if args == Cli::default() {
        let mut cmd = Cli::command();
        cmd.print_help()?;
        return Ok(());
    } else {
        run_no_stdin(args)?;
    }
    eprintln!("Completed in {}", format_duration(now.elapsed()));
    Ok(())
}
