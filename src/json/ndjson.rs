pub mod errors;
pub mod stats;

use crate::io_helpers::buf_reader::get_bufreader;
use crate::json::paths::ValuePaths;
use crate::json::{Value, ValueType};
use crate::{io_helpers, Cli, Settings};

use self::errors::collection::{
    Errors, ErrorsPar, IndexedNDJSONError, IntoEnumeratedErrFiltered, IntoErrFiltered,
    NDJSONProcessingErrors,
};
use self::errors::NDJSONError;
pub use self::stats::{FileStats, Stats};

use dashmap::DashMap;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::iter::ParallelBridge;
use rayon::prelude::ParallelIterator;

use std::error::Error;
use std::fs::File;
use std::io::{self, prelude::*};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Receiver;

// Reusable types for function signatures
type IJSONCandidateResult = (usize, Result<String, io::Error>);
type IJSONCandidate = (usize, String);
type IdJSON = (String, Value);
type IdJSONIter<'a> = Box<dyn Iterator<Item = IdJSON> + 'a>;
type NDJSONErrors = Errors<IndexedNDJSONError>;
type NDJSONErrorsPar = ErrorsPar<IndexedNDJSONError>;

// Should these return a struct?
pub trait IndexedString {
    fn to_indexed_strings(self) -> Box<dyn Iterator<Item = IJSONCandidateResult>>;
}

impl IndexedString for Receiver<String> {
    fn to_indexed_strings(self) -> Box<dyn Iterator<Item = IJSONCandidateResult>> {
        Box::new(self.into_iter().enumerate().map(|(i, s)| (i + 1, Ok(s))))
    }
}

impl IndexedString for Box<dyn BufRead> {
    fn to_indexed_strings(self) -> Box<dyn Iterator<Item = IJSONCandidateResult>> {
        Box::new(self.lines().enumerate().map(|(i, s)| (i + 1, s)))
    }
}

// TODO: add or switch to method on `Receiver<String>`?
/// Indexes data from the mpsc channel, converts it to serde JSON `Value`s and filters out data that does not
/// parse as JSON to the `errors` container. Single threaded.
///
/// See also: [`parse_ndjson_receiver_par`]
pub fn parse_ndjson_receiver<'a>(
    _args: &Cli,
    receiver: Receiver<String>,
    errors: &NDJSONErrors,
) -> Result<IdJSONIter<'a>, Box<dyn Error>> {
    let json_iter = receiver
        .to_indexed_strings()
        .to_err_filtered(errors.new_ref())
        .map(|(i, json_candidate)| {
            (
                i.to_string(),
                serde_json::from_str::<Value>(&json_candidate),
            )
        })
        .to_err_filtered(errors.new_ref());

    Ok(Box::new(json_iter))
}

// TODO: add or switch to method on `Receiver<String>`?
/// Indexes data from the mpsc channel, converts it to serde JSON `Value`s and filters out data that does not
/// parse as JSON to the `errors` container. Multithreaded version of [`parse_ndjson_receiver`].
///
/// See also: [`parse_ndjson_receiver`], [`parse_ndjson_bufreader_par`] & [`parse_ndjson_iter_par`]
pub fn parse_ndjson_receiver_par<'a>(
    args: &Cli,
    receiver: Receiver<String>,
    errors: &'a NDJSONErrorsPar,
) -> impl ParallelIterator<Item = IdJSON> + 'a {
    let receiver = receiver
        .into_iter()
        .enumerate()
        .map(|(i, json_candidate)| (i + 1, json_candidate));
    parse_ndjson_iter_par(args, receiver, errors)
}

// TODO: rename or switch function args?
// TODO: add or switch to method on `&PathBuf`?
/// Indexes data from the file_path with a bufreader, converts it to serde JSON `Value`s
/// and filters out data that does not
/// parse as JSON to the `errors` container. Multithreaded version of [`parse_ndjson_bufreader`].
///
/// See also: [`parse_ndjson_bufreader`], [`parse_ndjson_receiver_par`] & [`parse_ndjson_iter_par`]
pub fn parse_ndjson_bufreader_par<'a>(
    args: &Cli,
    file_path: &PathBuf,
    errors: &'a NDJSONErrorsPar,
) -> Result<impl ParallelIterator<Item = IdJSON> + 'a, NDJSONError> {
    let reader = get_bufreader(args, file_path)?;

    let iter = reader.lines().enumerate();
    let iter = iter.filter_map(|(i, line)| {
        let i = i + 1; // count lines from 1
        let io_errors = errors.new_ref();
        match line {
            Err(e) => {
                io_errors.push(IndexedNDJSONError {
                    location: i.to_string(),
                    error: NDJSONError::IOError(e),
                });
                None
            }
            Ok(json) => Some((i, json)),
        }
    });

    Ok(parse_ndjson_iter_par(args, iter, errors))
}

// https://github.com/rayon-rs/rayon/issues/628
// https://users.rust-lang.org/t/how-to-wrap-a-non-object-safe-trait-in-an-object-safe-one/33904
/// Processes indexed data from the Iterator, converts it to serde JSON `Value`s
/// and filters out data that does not parse as JSON to the `errors` container.
///
/// See also: [`parse_ndjson_receiver_par`] & [`parse_ndjson_bufreader_par`]
pub fn parse_ndjson_iter_par<'a>(
    args: &Cli,
    iter: impl Iterator<Item = IJSONCandidate> + Send + 'a,
    errors: &'a NDJSONErrorsPar,
) -> impl ParallelIterator<Item = IdJSON> + 'a {
    let iter = iter.take(args.lines.unwrap_or(usize::MAX));

    let json_iter = iter.par_bridge().map(|(i, json_candidate)| {
        (
            i.to_string(),
            serde_json::from_str::<Value>(&json_candidate),
        )
    });

    json_iter.filter_map(|(id, json)| {
        let json_parse_errors = errors.new_ref();
        match json {
            Err(e) => {
                json_parse_errors.push(IndexedNDJSONError {
                    location: id,
                    error: NDJSONError::JSONParsingError(e),
                });
                None
            }
            Ok(json) => Some((id, json)),
        }
    })
}

/// Indexes data from the bufreader, converts it to serde JSON `Value`s
/// and filters out data that does not
/// parse as JSON to the `errors` container. Single threaded version of [`parse_ndjson_bufreader_par`].
///
/// See also: [`parse_ndjson_bufreader_par`], [`parse_ndjson_file`], [`parse_ndjson_file_path`] & [`parse_ndjson_receiver`]
pub fn parse_ndjson_bufreader<'a>(
    _args: &Cli,
    reader: impl BufRead + 'a,
    errors: &NDJSONErrors,
) -> IdJSONIter<'a> {
    let json_iter = reader.lines();

    let json_iter = json_iter.to_enumerated_err_filtered(errors.new_ref());

    let json_iter = json_iter.map(|(i, json_candidate)| {
        (
            i.to_string(),
            serde_json::from_str::<Value>(&json_candidate),
        )
    });
    let json_iter = json_iter.to_err_filtered(errors.new_ref());

    Box::new(json_iter)
}

/// Indexes data from the file, converts it to serde JSON `Value`s
/// and filters out data that does not
/// parse as JSON to the `errors` container. Single threaded.
///
/// See also: [`parse_ndjson_bufreader`], [`parse_ndjson_file_path`] & [`parse_ndjson_receiver`]
pub fn parse_ndjson_file<'a>(args: &Cli, file: File, errors: &NDJSONErrors) -> IdJSONIter<'a> {
    let reader = io::BufReader::new(file);
    parse_ndjson_bufreader(args, reader, errors)
}

/// Indexes data from the file_path, converts it to serde JSON `Value`s
/// and filters out data that does not
/// parse as JSON to the `errors` container. Single threaded.
///
/// See also: [`parse_ndjson_bufreader`], [`parse_ndjson_file`] & [`parse_ndjson_receiver`]
pub fn parse_ndjson_file_path<'a>(
    args: &Cli,
    file_path: &PathBuf,
    errors: &NDJSONErrors,
) -> Result<IdJSONIter<'a>, NDJSONError> {
    let reader = get_bufreader(args, file_path)?;
    Ok(parse_ndjson_bufreader(args, reader, errors))
}

/// Handles the jsonpath query expansion of the Iterators values. Single threaded
///
/// See also [`expand_jsonpath_query_par`]
pub fn expand_jsonpath_query<'a>(
    settings: &'a Settings,
    json_iter: impl Iterator<Item = IdJSON> + 'a,
    errors: &NDJSONErrors,
) -> IdJSONIter<'a> {
    let select_errors = errors.new_ref();
    let missing = errors.new_ref();
    let json_iter_out: IdJSONIter<'a>;
    if let Some(ref selector) = settings.jsonpath_selector {
        let path = settings.args.jsonpath.to_owned();
        let path = path.expect("must exist for jsonpath_selector to exist");
        let expanded = json_iter.flat_map(move |(ref id, ref json)| {
            let mut select_errored = false;
            let selected = selector.select(json).unwrap_or_else(|e| {
                select_errors.push(IndexedNDJSONError::new(
                    id.to_owned(),
                    NDJSONError::QueryJsonPathError(e),
                ));
                select_errored = true;
                vec![]
            });
            if selected.is_empty() && !select_errored {
                missing.push(IndexedNDJSONError::new(
                    id.to_owned(),
                    NDJSONError::EmptyQuery,
                ))
            }
            selected
                .into_iter()
                .enumerate()
                .map(|(i, json)| (format!("{id}:{path}[{i}]"), json.to_owned()))
                .collect::<Vec<_>>()
        });
        json_iter_out = Box::new(expanded);
    } else {
        json_iter_out = Box::new(json_iter);
    }
    json_iter_out
}

/// Handles the jsonpath query expansion of the Iterators values. Multi-threaded.
///
/// See also [`expand_jsonpath_query`]
pub fn expand_jsonpath_query_par<'a>(
    settings: &'a Settings,
    json_iter: impl ParallelIterator<Item = IdJSON> + 'a,
    errors: &NDJSONErrorsPar,
) -> impl ParallelIterator<Item = IdJSON> + 'a {
    let select_errors = errors.new_ref();
    let missing = errors.new_ref();

    json_iter.flat_map(move |(id, json)| {
        if let Some(ref selector) = settings.jsonpath_selector {
            let path = settings.args.jsonpath.to_owned();
            let path = path.expect("must exist for jsonpath_selector to exist");

            let mut select_errored = false;
            let selected = selector.select(&json).unwrap_or_else(|e| {
                select_errors.push(IndexedNDJSONError::new(
                    id.to_owned(),
                    NDJSONError::QueryJsonPathError(e),
                ));
                select_errored = true;
                vec![]
            });
            if selected.is_empty() && !select_errored {
                missing.push(IndexedNDJSONError::new(
                    id.to_owned(),
                    NDJSONError::EmptyQuery,
                ))
            }
            selected
                .into_iter()
                .enumerate()
                .map(|(i, json)| (format!("{id}:{path}[{i}]"), json.to_owned()))
                .collect::<Vec<_>>()
        } else {
            vec![(id, json)]
        }
    })
}

/// Apply pre-processing based on settings from CLI args. Single threaded.
///
/// See also [`apply_settings_par`]
pub fn apply_settings<'a>(
    settings: &'a Settings,
    json_iter: impl Iterator<Item = IdJSON> + 'a,
    errors: &NDJSONErrors,
) -> IdJSONIter<'a> {
    let args = &settings.args;

    let json_iter = limit(args, json_iter);
    expand_jsonpath_query(settings, json_iter, errors)
}

/// Apply pre-processing based on settings from CLI args. Multi-threaded.
///
/// See also [`apply_settings`]
pub fn apply_settings_par<'a>(
    settings: &'a Settings,
    json_iter: impl ParallelIterator<Item = IdJSON> + 'a,
    errors: &NDJSONErrorsPar,
) -> impl ParallelIterator<Item = IdJSON> + 'a {
    expand_jsonpath_query_par(settings, json_iter, errors)
}

/// Main function processing the JSON data, collecting key infomation about the content.
/// Single threaded.
///
/// See also [`process_json_iterable_par`]
pub fn process_json_iterable(
    settings: &Settings,
    json_iter: impl Iterator<Item = IdJSON>,
    errors: &NDJSONErrors,
) -> Stats {
    let mut fs = Stats::new();
    let args = &settings.args;

    let json_iter = apply_settings(settings, json_iter, errors);

    let spinner = ProgressBar::new_spinner().with_style(
        ProgressStyle::with_template("{spinner} {elapsed_precise} Lines: {pos:>10}\t{per_sec}\n")
            .unwrap(),
    );

    for (_id, json) in json_iter {
        spinner.inc(1);
        fs.line_count += 1;

        for value_path in json.value_paths(args.explode_arrays, args.inspect_arrays) {
            let path = value_path.jsonpath();
            let counter = fs.keys_count.entry(path.to_owned()).or_insert(0);
            *counter += 1;

            let type_ = value_path.value.value_type();
            let path_type = format!("{}::{}", path, type_);
            let counter = fs.keys_types_count.entry(path_type).or_insert(0);
            *counter += 1;
        }
    }
    spinner.finish();

    for indexed_error in errors.container.borrow().as_slice() {
        let IndexedNDJSONError { location, error } = indexed_error;
        let location = location.to_owned();
        match error {
            NDJSONError::JSONParsingError(_) => fs.bad_lines.push(location),
            NDJSONError::EmptyQuery => fs.empty_lines.push(location),
            NDJSONError::QueryJsonPathError(_) => fs.bad_lines.push(location),
            NDJSONError::IOError(_) => fs.bad_lines.push(location),
        }
    }
    fs
}

/// Main function processing the JSON data, collecting key infomation about the content.
/// Mulit-threaded version of [`process_json_iterable`].
///
/// See also [`process_json_iterable_par`]
pub fn process_json_iterable_par<'a>(
    settings: &Settings,
    json_iter: impl ParallelIterator<Item = IdJSON> + 'a,
    errors: &'a NDJSONErrorsPar,
) -> Stats {
    let mut fs = Stats::new();
    let args = &settings.args;

    let keys_count: DashMap<String, usize> = DashMap::new();
    let keys_types_count: DashMap<String, usize> = DashMap::new();
    let line_count = AtomicUsize::new(0);

    let json_iter = apply_settings_par(settings, json_iter, errors);

    let spinner = ProgressBar::new_spinner().with_style(
        ProgressStyle::with_template("{spinner} {elapsed_precise} Lines: {pos:>10}\t{per_sec}\n")
            .unwrap(),
    );

    json_iter.for_each(|(_id, json)| {
        line_count.fetch_add(1, Ordering::Release);

        for value_path in json.value_paths(args.explode_arrays, args.inspect_arrays) {
            let path = value_path.jsonpath();
            let mut counter = keys_count.entry(path.to_owned()).or_insert(0);
            *counter.value_mut() += 1;

            let type_ = value_path.value.value_type();
            let path_type = format!("{}::{}", path, type_);
            let mut counter = keys_types_count.entry(path_type).or_insert(0);
            *counter.value_mut() += 1;
        }
        spinner.inc(1);
    });

    spinner.finish();

    for indexed_error in errors.container.lock().unwrap().as_slice() {
        let IndexedNDJSONError { location, error } = indexed_error;
        let location = location.to_owned();
        match error {
            NDJSONError::JSONParsingError(_) => fs.bad_lines.push(location),
            NDJSONError::EmptyQuery => fs.empty_lines.push(location),
            NDJSONError::QueryJsonPathError(_) => fs.bad_lines.push(location),
            NDJSONError::IOError(_) => fs.bad_lines.push(location),
        }
    }

    fs.keys_count = keys_count
        .into_read_only()
        .iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    fs.line_count = line_count.load(Ordering::Acquire);
    fs.keys_types_count = keys_types_count
        .into_read_only()
        .iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    fs
}

/// Apply line limiting from the arg to the Iterator
///
/// See also [`parse_iter`]
pub fn limit<'a, I, T>(args: &Cli, iter: I) -> Box<dyn Iterator<Item = T> + 'a>
where
    I: Iterator<Item = T> + 'a,
{
    if let Some(n) = args.lines {
        Box::new(iter.take(n))
    } else {
        Box::new(iter)
    }
}

// TODO: Rename?
/// Early version of [`apply_settings`], kept as an example of alternative version of
/// [`limit`] that could be used without the need to `Box` the return value
///
/// See also [`limit`]
#[deprecated(note = "Superseded by `apply_settings`")]
pub fn parse_iter<E, I>(args: &Cli, iter: I) -> impl Iterator<Item = Result<String, E>>
where
    I: Iterator<Item = Result<String, E>>,
{
    if let Some(n) = args.lines {
        iter.take(n)
    } else {
        iter.take(usize::MAX)
    }
}

pub struct StatsResult {
    pub stats: Stats,
    pub errors: Box<dyn NDJSONProcessingErrors>,
}

pub trait JSONStats {
    fn json_stats(self, settings: &Settings) -> Result<StatsResult, NDJSONError>;
}

// TODO: Add tests
impl JSONStats for io::Stdin {
    fn json_stats(self, settings: &Settings) -> Result<StatsResult, NDJSONError> {
        let stats;
        let errors: Box<dyn NDJSONProcessingErrors>;
        if settings.args.parallel {
            let stdin = io_helpers::stdin::spawn_stdin_channel(self, 1_000_000);
            let _errors = ErrorsPar::default();
            let json_iter = parse_ndjson_receiver_par(&settings.args, stdin, &_errors);
            stats = process_json_iterable_par(settings, json_iter, &_errors);
            errors = Box::new(_errors);
        } else {
            let stdin = self.lock();
            let _errors = Errors::default();
            let json_iter = parse_ndjson_bufreader(&settings.args, stdin, &_errors);
            stats = process_json_iterable(settings, json_iter, &_errors);
            errors = Box::new(_errors);
        }
        Ok(StatsResult { stats, errors })
    }
}

impl JSONStats for &PathBuf {
    fn json_stats(self, settings: &Settings) -> Result<StatsResult, NDJSONError> {
        let stats;
        let errors: Box<dyn NDJSONProcessingErrors>;
        if settings.args.parallel {
            let _errors = ErrorsPar::default();
            let json_iter = parse_ndjson_bufreader_par(&settings.args, self, &_errors)?;
            stats = process_json_iterable_par(settings, json_iter, &_errors);
            errors = Box::new(_errors);
        } else {
            let _errors = Errors::default();
            let json_iter = parse_ndjson_file_path(&settings.args, self, &_errors)?;
            stats = process_json_iterable(settings, json_iter, &_errors);
            errors = Box::new(_errors);
        }
        Ok(StatsResult { stats, errors })
    }
}

#[cfg(test)]
mod tests {
    use crate::json::IndexMap;
    use serde_json::json;

    use super::*;
    use std::fs::File;
    use std::io::{Seek, SeekFrom, Write};

    // TODO: How to test stdin?

    #[test]
    fn line_read() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key2": 123}}"#).unwrap();
        tmpfile.seek(SeekFrom::Start(0)).unwrap();
        let path = tmpfile.path().to_path_buf();

        let args = Cli::default();
        let buf_reader: Box<dyn BufRead> = get_bufreader(&args, &path).unwrap();
        let mut indexed = buf_reader.to_indexed_strings();

        let (i, s) = indexed.next().unwrap();
        assert_eq!(1, i);
        assert_eq!(r#"{"key1": 123}"#.to_string(), s.unwrap());
        let (i, s) = indexed.next().unwrap();
        assert_eq!(2, i);
        assert_eq!(r#"{"key2": 123}"#.to_string(), s.unwrap());

        assert!(indexed.next().is_none());
    }

    #[test]
    fn simple_json_stats() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key2": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        tmpfile.seek(SeekFrom::Start(0)).unwrap();
        let path = tmpfile.path().to_path_buf();

        let expected = StatsResult {
            stats: Stats {
                keys_count: IndexMap::from([("$.key1".to_string(), 2), ("$.key2".to_string(), 1)]),
                line_count: 3,
                bad_lines: vec![],
                keys_types_count: IndexMap::from([
                    ("$.key1::Number".to_string(), 2),
                    ("$.key2::Number".to_string(), 1),
                ]),
                empty_lines: vec![],
            },
            errors: Box::new(Errors::<NDJSONError>::default()),
        };

        let args = Cli::default();
        let settings = Settings::init(args).unwrap();

        let actual = path.json_stats(&settings).unwrap();
        assert_eq!(expected.stats, actual.stats);
    }

    #[test]
    fn simple_json_stats_par() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key2": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        tmpfile.seek(SeekFrom::Start(0)).unwrap();
        let path = tmpfile.path().to_path_buf();

        let expected = StatsResult {
            stats: Stats {
                keys_count: IndexMap::from([("$.key1".to_string(), 2), ("$.key2".to_string(), 1)]),
                line_count: 3,
                bad_lines: vec![],
                keys_types_count: IndexMap::from([
                    ("$.key1::Number".to_string(), 2),
                    ("$.key2::Number".to_string(), 1),
                ]),
                empty_lines: vec![],
            },
            errors: Box::new(Errors::<NDJSONErrorsPar>::default()),
        };

        let mut args = Cli::default();
        args.parallel = true;
        let settings = Settings::init(args).unwrap();

        let actual = path.json_stats(&settings).unwrap();
        assert_eq!(expected.stats, actual.stats);
    }

    #[test]
    fn simple_ndjson() {
        let mut tmpfile: File = tempfile::tempfile().unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key2": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        tmpfile.seek(SeekFrom::Start(0)).unwrap();
        let reader = io::BufReader::new(tmpfile);

        let expected: Vec<IdJSON> = vec![
            (1.to_string(), json!({"key1": 123})),
            (2.to_string(), json!({"key2": 123})),
            (3.to_string(), json!({"key1": 123})),
        ];

        let args = Cli::default();
        let errors = Errors::default();

        let json_iter = parse_ndjson_bufreader(&args, reader, &errors);
        assert_eq!(expected, json_iter.collect::<Vec<IdJSON>>());
        assert!(errors.container.borrow().is_empty())
    }

    #[test]
    fn bad_ndjson_file() {
        let mut tmpfile: File = tempfile::tempfile().unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        writeln!(tmpfile, r#"not valid json"#).unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        tmpfile.seek(SeekFrom::Start(0)).unwrap();
        let reader = io::BufReader::new(tmpfile);

        let expected: Vec<IdJSON> = vec![
            (1.to_string(), json!({"key1": 123})),
            (3.to_string(), json!({"key1": 123})),
        ];

        let args = Cli::default();
        let errors = Errors::default();

        let json_iter = parse_ndjson_bufreader(&args, reader, &errors);
        assert_eq!(expected, json_iter.collect::<Vec<IdJSON>>());
        assert!(errors.container.borrow().len() == 1)
    }

    #[test]
    fn simple_expand_jsonpath_query() {
        let json_iter_in: Vec<IdJSON> = vec![
            (1.to_string(), json!({"key1": [1, 2, 3]})),
            (2.to_string(), json!({"key2": 123})),
            (3.to_string(), json!({"key1": [4, 5]})),
        ];
        let json_iter_in = json_iter_in.iter().cloned();

        let mut args = Cli::default();
        args.jsonpath = Some("$.key1[*]".to_string());
        let settings = Settings::init(args).unwrap();
        let errors = Errors::default();

        let expected: Vec<IdJSON> = vec![
            ("1:$.key1[*][0]".to_string(), json!(1)),
            ("1:$.key1[*][1]".to_string(), json!(2)),
            ("1:$.key1[*][2]".to_string(), json!(3)),
            ("3:$.key1[*][0]".to_string(), json!(4)),
            ("3:$.key1[*][1]".to_string(), json!(5)),
        ];

        let json_iter = expand_jsonpath_query(&settings, json_iter_in, &errors);
        assert_eq!(expected, json_iter.collect::<Vec<IdJSON>>());
        assert!(errors.container.borrow().len() == 1)
    }

    #[test]
    fn simple_process_json_iterable() {
        let json_iter_in: Vec<IdJSON> = vec![
            (1.to_string(), json!({"key1": 123})),
            (2.to_string(), json!({"key2": 123})),
            (3.to_string(), json!({"key1": 123})),
        ];
        let json_iter_in = json_iter_in.iter().cloned();

        let args = Cli::default();
        let settings = Settings::init(args).unwrap();
        let errors = Errors::default();

        let expected = Stats {
            keys_count: IndexMap::from([("$.key1".to_string(), 2), ("$.key2".to_string(), 1)]),
            line_count: 3,
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 2),
                ("$.key2::Number".to_string(), 1),
            ]),
            ..Default::default()
        };

        let stats = process_json_iterable(&settings, json_iter_in, &errors);
        assert_eq!(expected, stats);
        assert!(errors.container.borrow().is_empty())
    }

    #[test]
    fn bad_process_json_iterable_path_query() {
        let json_iter_in: Vec<IdJSON> = vec![
            (1.to_string(), json!({"key1": 123})),
            (2.to_string(), json!({"key2": 123})),
            (3.to_string(), json!({"key1": 123})),
        ];
        let json_iter_in = json_iter_in.iter().cloned();

        let mut args = Cli::default();
        args.jsonpath = Some("$.key1".to_string());
        let settings = Settings::init(args).unwrap();
        let errors = Errors::default();

        let expected = Stats {
            keys_count: IndexMap::from([("$".to_string(), 2)]),
            line_count: 2,
            keys_types_count: IndexMap::from([("$::Number".to_string(), 2)]),
            empty_lines: vec![2.to_string()],
            ..Default::default()
        };

        let stats = process_json_iterable(&settings, json_iter_in, &errors);
        assert_eq!(expected, stats);
        assert!(errors.container.borrow().len() == 1)
    }

    #[test]
    fn simple_process_json_iterable_par() {
        let iter: Vec<(String, Value)> = vec![
            (1.to_string(), json!({"key1": 123})),
            (2.to_string(), json!({"key2": 123})),
            (3.to_string(), json!({"key1": 123})),
        ];
        let iter = iter.into_iter().par_bridge();

        let expected = Stats {
            keys_count: IndexMap::from([("$.key1".to_string(), 2), ("$.key2".to_string(), 1)]),
            line_count: 3,
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 2),
                ("$.key2::Number".to_string(), 1),
            ]),
            ..Default::default()
        };

        let args = Cli::default();
        let settings = Settings::init(args).unwrap();
        let errors = ErrorsPar::default();
        let stats = process_json_iterable_par(&settings, iter, &errors);
        assert_eq!(expected, stats);
    }

    #[test]
    fn simple_process_json_iterable_par_jsonpath() {
        let iter: Vec<(String, Value)> = vec![
            (1.to_string(), json!({"key1": 123})),
            (2.to_string(), json!({"a": {"key2": 123}})),
            (3.to_string(), json!({"key1": 123})),
        ];
        let iter = iter.into_iter().par_bridge();

        let expected = Stats {
            keys_count: IndexMap::from([("$.key2".to_string(), 1)]),
            line_count: 1,
            keys_types_count: IndexMap::from([("$.key2::Number".to_string(), 1)]),
            empty_lines: vec![1.to_string(), 3.to_string()],
            ..Default::default()
        };

        let mut args = Cli::default();
        args.jsonpath = Some("$.a".to_string());
        let settings = Settings::init(args).unwrap();
        let errors = ErrorsPar::default();
        let stats = process_json_iterable_par(&settings, iter, &errors);
        assert_eq!(expected, stats);
    }

    #[test]
    fn add_filestats() {
        let lhs = stats::FileStats {
            file_path: "file/1.json".to_string(),
            stats: Stats {
                keys_count: IndexMap::from([("$.key1".to_string(), 3), ("$.key2".to_string(), 2)]),
                line_count: 5,
                keys_types_count: IndexMap::from([
                    ("$.key1::Number".to_string(), 3),
                    ("$.key2::Number".to_string(), 2),
                ]),
                bad_lines: vec!["4".to_string()],
                empty_lines: vec!["5".to_string()],
            },
        };
        let rhs = stats::FileStats {
            file_path: "file/2.json".to_string(),
            stats: Stats {
                keys_count: IndexMap::from([("$.key3".to_string(), 3), ("$.key2".to_string(), 2)]),
                line_count: 7,
                keys_types_count: IndexMap::from([
                    ("$.key3::Number".to_string(), 3),
                    ("$.key2::Number".to_string(), 2),
                ]),
                bad_lines: vec!["1".to_string()],
                empty_lines: vec!["2".to_string()],
            },
        };
        let expected = Stats {
            keys_count: IndexMap::from([
                ("$.key1".to_string(), 3),
                ("$.key2".to_string(), 4),
                ("$.key3".to_string(), 3),
            ]),
            line_count: 12,
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 3),
                ("$.key2::Number".to_string(), 4),
                ("$.key3::Number".to_string(), 3),
            ]),
            bad_lines: vec!["file/1.json:4".to_string(), "file/2.json:1".to_string()],
            empty_lines: vec!["file/1.json:5".to_string(), "file/2.json:2".to_string()],
        };

        let vec_of_file_stats = vec![lhs.clone(), rhs.clone()];
        let actual_ref = lhs.clone() + &rhs;
        let actual = lhs + rhs;

        assert_eq!(actual, expected);
        assert_eq!(actual_ref, expected);
        assert_eq!(vec_of_file_stats.iter().sum::<Stats>(), expected);
    }
}
