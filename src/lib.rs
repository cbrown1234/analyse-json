use anyhow::{Context, Result};
use clap::builder::styling::AnsiColor;
use clap::builder::Styles;
use clap::CommandFactory;
use clap::Parser;
use clap_complete::Shell;
use glob::glob;
use grep_cli::is_readable_stdin;
use humantime::format_duration;
use json::ndjson::JSONStats;
use serde_json_path::JsonPath;
use std::io;
use std::path::PathBuf;
use std::time::Instant;

use crate::json::ndjson;

mod io_helpers;
pub mod json;

fn styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Yellow.on_default())
        .usage(AnsiColor::Green.on_default())
        .literal(AnsiColor::Green.on_default())
        .placeholder(AnsiColor::Green.on_default())
}

#[derive(Parser, Default, PartialEq, Eq)]
#[clap(author, version, about, long_about = None, styles = styles())]
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

    /// JSONpath query to filter/limit the inspection to e.g. `'$.a_key.an_array[0]'`
    #[clap(long)]
    jsonpath: Option<String>,

    /// Walk the elements of arrays grouping elements paths together under `$.path.to.array[*]`?
    /// See also `--explode-arrays`
    #[clap(long)]
    inspect_arrays: bool,

    /// Walk the elements of arrays treating arrays like a map of their enumerated elements?
    /// (E.g. $.path.to.array[0], $.path.to.array[1], ...)
    /// See also `--inspect-arrays`
    #[clap(long, conflicts_with = "inspect_arrays")]
    explode_arrays: bool,

    /// Include combined results for all files when using glob
    #[clap(long)]
    merge: bool,

    /// Use multi-threaded version of the processing
    #[clap(long)]
    parallel: bool,

    /// Silence error logging
    #[clap(short, long)]
    quiet: bool,

    /// Output shell completions for the chosen shell to stdout
    #[clap(value_enum, long, id = "SHELL")]
    generate_completions: Option<Shell>,
}

impl Cli {
    fn jsonpath_selector(&self) -> Result<Option<JsonPath>> {
        let jsonpath_selector = if let Some(jsonpath) = &self.jsonpath {
            let path = JsonPath::parse(jsonpath)
                .with_context(|| format!("Failed to parse jsonpath query string: {jsonpath}"))?;
            Some(path)
        } else {
            None
        };
        Ok(jsonpath_selector)
    }
}

/// Wrapper around [`Cli`] to hold derived attributes
pub struct Settings {
    args: Cli,
    jsonpath_selector: Option<JsonPath>,
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

fn process_ndjson_file_path(settings: &Settings, file_path: &PathBuf) -> Result<ndjson::Stats> {
    let stats = file_path.json_stats(settings).with_context(|| {
        format!(
            "Failed to collect stats for JSON file: {}",
            file_path.display()
        )
    })?;

    Ok(stats)
}

fn run_stdin(settings: Settings) -> Result<()> {
    let stats = io::stdin()
        .json_stats(&settings)
        .context("Failed to collect stats for JSON stdin")?;

    stats.print()?;
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
        let file_paths = glob(pattern).context(
            "Failed to parse glob pattern, try quoting '<pattern>' to avoid shell parsing",
        )?;
        for entry in file_paths {
            let file_path = entry?;
            println!("File '{}':", file_path.display());
            let file_stats = ndjson::FileStats::new(
                file_path.to_string_lossy().into_owned(),
                process_ndjson_file_path(&settings, &file_path)?,
            );

            file_stats.stats.print().with_context(|| {
                format!("Failed to print stats for file: {}", file_path.display())
            })?;
            if settings.args.merge {
                file_stats_list.push(file_stats)
            }
        }
        if settings.args.merge {
            println!("Overall Stats");
            let overall_file_stats: ndjson::Stats = file_stats_list.iter().sum();
            overall_file_stats
                .print()
                .context("Failed to print combined stats")?;
        }
        return Ok(());
    }
    Ok(())
}

fn print_completions(args: Cli) {
    let mut cmd = Cli::command();
    let shell = args
        .generate_completions
        .expect("function only called when argument specified");
    let bin_name = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, bin_name, &mut io::stdout());
}

pub fn run(args: Cli) -> Result<()> {
    let now = Instant::now();
    let settings = Settings::init(args).context("Failed to initialise settings from CLI args")?;
    if settings.args.generate_completions.is_some() {
        print_completions(settings.args);
        return Ok(());
    } else if is_readable_stdin() {
        run_stdin(settings).context("Failed to process stdin")?;
    } else if settings.args == Cli::default() {
        let mut cmd = Cli::command();
        cmd.print_help().context("Failed to pring CLI help")?;
        return Ok(());
    } else {
        run_no_stdin(settings).context("Failed to process file(s)")?;
    }
    eprintln!("Completed in {}", format_duration(now.elapsed()));
    Ok(())
}

#[test]
fn verify_cli() {
    Cli::command().debug_assert()
}
