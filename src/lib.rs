use clap::CommandFactory;
use clap::Parser;
use clap_complete::Shell;
use flate2::read::GzDecoder;
use glob::glob;
use grep_cli::is_readable_stdin;
use humantime::format_duration;
use json::ndjson::parse_ndjson_bufreader_par;
use json::ndjson::parse_ndjson_receiver_par;
use json::ndjson::process_json_iterable_par;
use json::ndjson::Errors;
use json::ndjson::ErrorsPar;
use jsonpath_lib::Compiled;
use std::error;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::time::Instant;

use crate::json::ndjson::{
    parse_ndjson_bufreader, parse_ndjson_file_path, process_json_iterable, FileStats,
};

mod io_helpers;
pub mod json;

type Result<T> = ::std::result::Result<T, Box<dyn error::Error>>;

#[derive(Parser, Default, PartialEq, Eq)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    /// File to process, expected to contain a single JSON object or Newline Delimited (ND) JSON objects
    #[clap(value_parser)]
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

    /// Walk the elements of arrays grouping elements paths together under `$.path.to.array[*]`?
    /// Overrides `--explode-arrays`
    #[clap(long)]
    inspect_arrays: bool,

    /// Walk the elements of arrays treating arrays like a map of their enumerated elements?
    /// (E.g. $.path.to.array[0], $.path.to.array[1], ...)
    /// Ignored if using `--inspect-arrays`
    #[clap(long)]
    explode_arrays: bool,

    /// Include combined results for all files when using glob
    #[clap(long)]
    merge: bool,

    /// Use parallel version of the processing
    #[clap(long)]
    parallel: bool,

    /// Silence error logging
    #[clap(short, long)]
    quiet: bool,

    #[clap(value_enum, long, id = "SHELL")]
    generate_completions: Option<Shell>,
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

pub struct Settings {
    args: Cli,
    jsonpath_selector: Option<Compiled>,
}

impl Settings {
    fn init(args: Cli) -> Result<Self> {
        let jsonpath_selector = args.jsonpath_selector()?;
        Ok(Self {
            args,
            jsonpath_selector,
        })
    }
}

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

fn process_ndjson_file_path(settings: &Settings, file_path: &PathBuf) -> Result<FileStats> {
    let errors = Errors::default();

    let json_iter = parse_ndjson_file_path(&settings.args, file_path, &errors)?;
    let file_stats = process_json_iterable(settings, json_iter, &errors);

    if !settings.args.quiet {
        errors.eprint();
    }

    Ok(file_stats)
}

fn process_ndjson_file_path_par(settings: &Settings, file_path: &PathBuf) -> Result<FileStats> {
    let errors = ErrorsPar::default();

    let json_iter = parse_ndjson_bufreader_par(&settings.args, file_path, &errors)?;
    let file_stats = process_json_iterable_par(settings, json_iter, &errors);

    if !settings.args.quiet {
        errors.eprint();
    }

    Ok(file_stats)
}

fn run_stdin(settings: Settings) -> Result<()> {
    let stdin = io::stdin().lock();
    let errors = Errors::default();
    let json_iter = parse_ndjson_bufreader(&settings.args, stdin, &errors)?;
    let stdin_file_stats = process_json_iterable(&settings, json_iter, &errors);

    if !settings.args.quiet {
        errors.eprint();
    }

    stdin_file_stats.print()?;
    Ok(())
}

fn run_stdin_par(settings: Settings) -> Result<()> {
    let stdin = io_helpers::stdin::spawn_stdin_channel(1_000_000);
    let errors = ErrorsPar::default();
    let json_iter = parse_ndjson_receiver_par(&settings.args, stdin, &errors);
    let stdin_file_stats = process_json_iterable_par(&settings, json_iter, &errors);

    if !settings.args.quiet {
        errors.eprint();
    }

    stdin_file_stats.print()?;
    Ok(())
}

fn run_no_stdin(settings: Settings) -> Result<()> {
    if let Some(file_path) = &settings.args.file_path {
        let file_stats = process_ndjson_file_path(&settings, file_path)?;

        file_stats.print()?;
        return Ok(());
    }

    if let Some(pattern) = &settings.args.glob {
        let mut file_stats_list = Vec::new();

        println!("Glob '{}':", pattern);
        for entry in glob(pattern)? {
            let file_path = entry?;
            println!("File '{}':", file_path.display());
            let file_stats = process_ndjson_file_path(&settings, &file_path)?;

            file_stats.print()?;
            if settings.args.merge {
                file_stats_list.push(file_stats)
            }
        }
        if settings.args.merge {
            println!("Overall Stats");
            let overall_file_stats: FileStats = file_stats_list.iter().sum();
            // TODO: Fix handling of corrupt & empty lines
            overall_file_stats.print()?;
        }
        return Ok(());
    }
    Ok(())
}

fn run_no_stdin_par(settings: Settings) -> Result<()> {
    if let Some(file_path) = &settings.args.file_path {
        let file_stats = process_ndjson_file_path_par(&settings, file_path)?;

        file_stats.print()?;
        return Ok(());
    }

    if let Some(pattern) = &settings.args.glob {
        let mut file_stats_list = Vec::new();

        println!("Glob '{}':", pattern);
        for entry in glob(pattern)? {
            let file_path = entry?;
            println!("File '{}':", file_path.display());
            let file_stats = process_ndjson_file_path_par(&settings, &file_path)?;

            file_stats.print()?;
            if settings.args.merge {
                file_stats_list.push(file_stats)
            }
        }
        if settings.args.merge {
            println!("Overall Stats");
            let overall_file_stats: FileStats = file_stats_list.iter().sum();
            // TODO: Fix handling of corrupt & empty lines
            overall_file_stats.print()?;
        }
        return Ok(());
    }
    Ok(())
}

fn print_completions(args: Cli) {
    let mut cmd = Cli::into_app();
    let shell = args.generate_completions.expect("only called when needed");
    let bin_name = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, bin_name, &mut io::stdout());
}

pub fn run(args: Cli) -> Result<()> {
    let now = Instant::now();
    let settings = Settings::init(args)?;
    if settings.args.generate_completions.is_some() {
        print_completions(settings.args);
        return Ok(());
    } else if is_readable_stdin() {
        if settings.args.parallel {
            run_stdin_par(settings)?;
        } else {
            run_stdin(settings)?;
        }
    } else if settings.args == Cli::default() {
        let mut cmd = Cli::command();
        cmd.print_help()?;
        return Ok(());
    } else if settings.args.parallel {
        run_no_stdin_par(settings)?;
    } else {
        run_no_stdin(settings)?;
    }
    eprintln!("Completed in {}", format_duration(now.elapsed()));
    Ok(())
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
}
