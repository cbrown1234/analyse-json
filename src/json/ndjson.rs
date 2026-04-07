pub mod errors;
pub mod stats;

use std::fmt::Write;

use crate::io_helpers::buf_reader::get_bufreader;
use crate::io_helpers::stdin::BackgroundRead;
use crate::json::paths::ValuePaths;
use crate::json::{Value, ValueType};
use crate::{Cli, Settings};

use self::errors::NDJSONError;
pub use self::stats::{FileStats, Stats};

use indexmap::map::RawEntryApiV1;
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Either;
use rayon::iter::ParallelBridge;
use rayon::prelude::ParallelIterator;

use std::io::{self, prelude::*};
use std::iter::Zip;
use std::ops::RangeFrom;
use std::path::PathBuf;

// Reusable types for function signatures

trait Indexed: Iterator {
    fn indexed(self) -> Zip<RangeFrom<usize>, Self>
    where
        Self: Sized,
    {
        (1usize..).zip(self)
    }
}

impl<T> Indexed for T where T: Iterator {}

type IdJSONResult = (String, Result<Value, NDJSONError>);
type IdJSONResultIter<'a> = Box<dyn Iterator<Item = IdJSONResult> + 'a>;

trait ToNDJSON<'a> {
    fn parse_ndjson(self, args: &Cli) -> impl Iterator<Item = IdJSONResult> + 'a;
}

trait ToNDJSONPar<'a>: ToNDJSON<'a> {
    fn parse_ndjson_par(self, args: &Cli) -> impl ParallelIterator<Item = IdJSONResult> + 'a;
}

// TODO: IntoIterator or Iterator?
impl<'a, T: Iterator<Item = io::Result<String>> + 'a> ToNDJSON<'a> for T {
    fn parse_ndjson(self, _args: &Cli) -> impl Iterator<Item = IdJSONResult> + 'a {
        self.map(|result| result.map_err(|e| e.into()))
            .indexed()
            .map(|(i, json_candidate)| {
                (
                    i.to_string(),
                    json_candidate
                        .and_then(|jc| serde_json::from_str::<Value>(&jc).map_err(|e| e.into())),
                )
            })
    }
}

impl<'a, T: Iterator<Item = io::Result<String>> + Send + 'a> ToNDJSONPar<'a> for T {
    fn parse_ndjson_par(self, args: &Cli) -> impl ParallelIterator<Item = IdJSONResult> + 'a {
        let iter = self
            .map(|result| result.map_err(|e| e.into()))
            .indexed()
            // limit the lines before moving to the parallel processing where the lines would become non-deterministic
            .take(args.lines.unwrap_or(usize::MAX));

        iter.par_bridge().map(|(i, json_candidate)| {
            (
                i.to_string(),
                json_candidate
                    .and_then(|jc| serde_json::from_str::<Value>(&jc).map_err(|e| e.into())),
            )
        })
    }
}

// TODO: Consider switching to match _par implementation without Box<_> (needs benchmarking)
/// Handles the jsonpath query expansion of the Iterators values. Single threaded
///
/// See also [`expand_jsonpath_query_result_par`]
pub fn expand_jsonpath_query_result<'a>(
    settings: &'a Settings,
    json_iter: impl Iterator<Item = IdJSONResult> + 'a,
) -> IdJSONResultIter<'a> {
    let json_iter_out: IdJSONResultIter<'a>;
    if let Some(ref selector) = settings.jsonpath_selector {
        let path = settings.args.jsonpath.to_owned();
        let path = path.expect("must exist for jsonpath_selector to exist");
        let expanded = json_iter.flat_map(move |(id, json_result)| {
            let Ok(json) = json_result else {
                return vec![(id.to_owned(), json_result)];
            };
            let selected = selector.query(&json);
            if selected.is_empty() {
                return vec![(id, Err(NDJSONError::EmptyQuery))];
            }
            selected
                .into_iter()
                .enumerate()
                .map(|(i, json)| (format!("{id}:{path}[{i}]"), Ok(json.to_owned())))
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
/// See also [`expand_jsonpath_query_result`]
pub fn expand_jsonpath_query_result_par<'a>(
    settings: &'a Settings,
    json_iter: impl ParallelIterator<Item = IdJSONResult> + 'a,
) -> impl ParallelIterator<Item = IdJSONResult> + 'a {
    json_iter.flat_map(move |(id, json_result)| {
        let Some(ref selector) = settings.jsonpath_selector else {
            return vec![(id, json_result)];
        };
        let Ok(json_input) = json_result else {
            return vec![(id, json_result)];
        };

        let path = settings.args.jsonpath.to_owned();
        let path = path.expect("must exist for jsonpath_selector to exist");

        let selected = selector.query(&json_input);
        if selected.is_empty() {
            return vec![(id, Err(NDJSONError::EmptyQuery))];
        }
        selected
            .into_iter()
            .enumerate()
            .map(|(i, json)| (format!("{id}:{path}[{i}]"), Ok(json.to_owned())))
            .collect::<Vec<_>>()
    })
}

fn make_spinner() -> ProgressBar {
    ProgressBar::new_spinner().with_style(
        ProgressStyle::with_template("{spinner} {elapsed_precise} Lines: {pos:>10}\t{per_sec}\n")
            .unwrap(),
    )
}

fn count_json(json: &Value, args: &Cli, stats: &mut Stats) {
    stats.line_count += 1;
    let mut path_type = String::with_capacity(100);
    for value_path in json.value_paths(args.explode_arrays, args.inspect_arrays) {
        let path = value_path.jsonpath();
        *stats.keys_count.entry(path.to_owned()).or_insert(0) += 1;

        let type_ = value_path.value.value_type();
        path_type.clear();
        write!(path_type, "{}::{}", path, type_).unwrap();
        let (_, counter) = stats
            .keys_types_count
            .raw_entry_mut_v1()
            .from_key(&path_type)
            .or_insert_with(|| (path_type.to_owned(), 0));
        *counter += 1;
    }
}

/// Main function processing the JSON data, collecting key information about the content.
/// Single threaded.
///
/// See also [`process_json_result_iterable_par`]
pub fn process_json_result_iterable(
    settings: &Settings,
    json_iter: impl Iterator<Item = IdJSONResult>,
) -> Stats {
    let mut fs = Stats::default();
    let args = &settings.args;

    let json_iter = limit(args, json_iter);
    let json_iter = expand_jsonpath_query_result(settings, json_iter);

    let spinner = make_spinner();

    for (id, json_result) in json_iter {
        match json_result {
            Ok(json) => {
                spinner.inc(1);
                count_json(&json, args, &mut fs);
            }
            Err(NDJSONError::JSONParsingError(_) | NDJSONError::IOError(_)) => {
                fs.bad_lines.push(id)
            }
            Err(NDJSONError::EmptyQuery) => fs.empty_lines.push(id),
        };
    }
    spinner.finish();

    fs
}

/// Main function processing the JSON data, collecting key information about the content.
/// Multi-threaded version of [`process_json_result_iterable`].
///
/// See also [`process_json_result_iterable`]
pub fn process_json_result_iterable_par<'a>(
    settings: &Settings,
    json_iter: impl ParallelIterator<Item = IdJSONResult> + 'a,
) -> Stats {
    let args = &settings.args;
    let spinner = make_spinner();
    let json_iter = expand_jsonpath_query_result_par(settings, json_iter);
    let stats = json_iter
        .fold(Stats::default, |mut acc, (id, result)| {
            match result {
                Ok(json) => {
                    spinner.inc(1);
                    count_json(&json, args, &mut acc);
                }
                Err(NDJSONError::JSONParsingError(_) | NDJSONError::IOError(_)) => {
                    acc.bad_lines.push(id)
                }
                Err(NDJSONError::EmptyQuery) => acc.empty_lines.push(id),
            }
            acc
        })
        .reduce(Stats::default, |a, b| a + b);
    spinner.finish();
    stats
}

/// Apply line limiting from the arg to the Iterator
pub fn limit<I, T>(args: &Cli, iter: I) -> impl Iterator<Item = T>
where
    I: Iterator<Item = T>,
{
    if let Some(n) = args.lines {
        Either::Left(iter.take(n))
    } else {
        Either::Right(iter)
    }
}

pub trait JSONStats {
    fn json_stats(self, settings: &Settings) -> Result<Stats, NDJSONError>;
}

// TODO: Add tests
impl JSONStats for io::Stdin {
    fn json_stats(self, settings: &Settings) -> Result<Stats, NDJSONError> {
        let stats = if settings.args.parallel {
            let stdin = self.background_read_lines(1_000_000);
            let json_iter = stdin.into_iter().parse_ndjson_par(&settings.args);
            process_json_result_iterable_par(settings, json_iter)
        } else {
            let stdin = self.lock();
            let json_iter = stdin.lines().parse_ndjson(&settings.args);
            process_json_result_iterable(settings, json_iter)
        };
        Ok(stats)
    }
}

impl JSONStats for &PathBuf {
    fn json_stats(self, settings: &Settings) -> Result<Stats, NDJSONError> {
        let stats;
        let reader = get_bufreader(&settings.args, self)?;
        if settings.args.parallel {
            let json_iter = reader.lines().parse_ndjson_par(&settings.args);
            stats = process_json_result_iterable_par(settings, json_iter);
        } else {
            let json_iter = reader.lines().parse_ndjson(&settings.args);
            stats = process_json_result_iterable(settings, json_iter);
        }
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use crate::json::IndexMap;
    use serde_json::json;

    use super::*;
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
        let mut indexed = buf_reader.lines().indexed();

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

        let expected = Stats {
            keys_count: IndexMap::from([("$.key1".to_string(), 2), ("$.key2".to_string(), 1)]),
            line_count: 3,
            bad_lines: vec![],
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 2),
                ("$.key2::Number".to_string(), 1),
            ]),
            empty_lines: vec![],
        };

        let args = Cli::default();
        let settings = Settings::init(args).unwrap();

        let actual = path.json_stats(&settings).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn simple_json_stats_par() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key2": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        tmpfile.seek(SeekFrom::Start(0)).unwrap();
        let path = tmpfile.path().to_path_buf();

        let expected = Stats {
            keys_count: IndexMap::from([("$.key1".to_string(), 2), ("$.key2".to_string(), 1)]),
            line_count: 3,
            bad_lines: vec![],
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 2),
                ("$.key2::Number".to_string(), 1),
            ]),
            empty_lines: vec![],
        };

        let settings = Settings::init(Cli {
            parallel: true,
            ..Cli::default()
        })
        .unwrap();

        let actual = path.json_stats(&settings).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn simple_ndjson() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key2": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        tmpfile.seek(SeekFrom::Start(0)).unwrap();
        let path = tmpfile.path().to_path_buf();

        let settings = Settings::init(Cli::default()).unwrap();
        let stats = path.json_stats(&settings).unwrap();
        assert_eq!(stats.line_count, 3);
        assert!(stats.bad_lines.is_empty());
    }

    #[test]
    fn bad_ndjson_file() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        writeln!(tmpfile, r#"not valid json"#).unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        tmpfile.seek(SeekFrom::Start(0)).unwrap();
        let path = tmpfile.path().to_path_buf();

        let settings = Settings::init(Cli::default()).unwrap();
        let stats = path.json_stats(&settings).unwrap();
        assert_eq!(stats.line_count, 2);
        assert_eq!(stats.bad_lines, vec!["2".to_string()]);
    }

    #[test]
    fn simple_expand_jsonpath_query() {
        let json_iter_in: Vec<IdJSONResult> = vec![
            (1.to_string(), Ok(json!({"key1": [1, 2, 3]}))),
            (2.to_string(), Ok(json!({"key2": 123}))),
            (3.to_string(), Ok(json!({"key1": [4, 5]}))),
        ];
        let json_iter_in = json_iter_in.into_iter();

        let settings = Settings::init(Cli {
            jsonpath: Some("$.key1[*]".to_string()),
            ..Cli::default()
        })
        .unwrap();

        let expected: Vec<IdJSONResult> = vec![
            ("1:$.key1[*][0]".to_string(), Ok(json!(1))),
            ("1:$.key1[*][1]".to_string(), Ok(json!(2))),
            ("1:$.key1[*][2]".to_string(), Ok(json!(3))),
            ("2:$.key1[*]".to_string(), Err(NDJSONError::EmptyQuery)),
            ("3:$.key1[*][0]".to_string(), Ok(json!(4))),
            ("3:$.key1[*][1]".to_string(), Ok(json!(5))),
        ];

        let json_iter = expand_jsonpath_query_result(&settings, json_iter_in);
        let results: Vec<IdJSONResult> = json_iter.collect();
        // Check non-error results match
        let ok_results: Vec<(&String, &Value)> = results
            .iter()
            .filter_map(|(id, r)| r.as_ref().ok().map(|v| (id, v)))
            .collect();
        let expected_ok: Vec<(&String, &Value)> = expected
            .iter()
            .filter_map(|(id, r)| r.as_ref().ok().map(|v| (id, v)))
            .collect();
        assert_eq!(ok_results, expected_ok);
        // Check empty query errors
        let empty_count = results
            .iter()
            .filter(|(_, r)| matches!(r, Err(NDJSONError::EmptyQuery)))
            .count();
        assert_eq!(empty_count, 1);
    }

    #[test]
    fn simple_process_json_iterable() {
        let json_iter_in: Vec<IdJSONResult> = vec![
            (1.to_string(), Ok(json!({"key1": 123}))),
            (2.to_string(), Ok(json!({"key2": 123}))),
            (3.to_string(), Ok(json!({"key1": 123}))),
        ];
        let json_iter_in = json_iter_in.into_iter();

        let settings = Settings::init(Cli::default()).unwrap();

        let expected = Stats {
            keys_count: IndexMap::from([("$.key1".to_string(), 2), ("$.key2".to_string(), 1)]),
            line_count: 3,
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 2),
                ("$.key2::Number".to_string(), 1),
            ]),
            ..Default::default()
        };

        let stats = process_json_result_iterable(&settings, json_iter_in);
        assert_eq!(expected, stats);
    }

    #[test]
    fn simple_process_json_result_iterable() {
        let json_iter_in: Vec<IdJSONResult> = vec![
            (1.to_string(), Ok(json!({"key1": 123}))),
            (2.to_string(), Ok(json!({"key2": 123}))),
            (3.to_string(), Ok(json!({"key1": 123}))),
        ];
        let json_iter_in = json_iter_in.into_iter();

        let args = Cli::default();
        let settings = Settings::init(args).unwrap();

        let expected = Stats {
            keys_count: IndexMap::from([("$.key1".to_string(), 2), ("$.key2".to_string(), 1)]),
            line_count: 3,
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 2),
                ("$.key2::Number".to_string(), 1),
            ]),
            ..Default::default()
        };

        let stats = process_json_result_iterable(&settings, json_iter_in);
        assert_eq!(expected, stats);
    }

    #[test]
    fn bad_process_json_result_iterable_path_query() {
        let json_iter_in: Vec<IdJSONResult> = vec![
            (1.to_string(), Ok(json!({"key1": 123}))),
            (2.to_string(), Ok(json!({"key2": 123}))),
            (3.to_string(), Ok(json!({"key1": 123}))),
        ];
        let json_iter_in = json_iter_in.into_iter();

        let settings = Settings::init(Cli {
            jsonpath: Some("$.key1".to_string()),
            ..Cli::default()
        })
        .unwrap();

        let expected = Stats {
            keys_count: IndexMap::from([("$".to_string(), 2)]),
            line_count: 2,
            keys_types_count: IndexMap::from([("$::Number".to_string(), 2)]),
            empty_lines: vec![2.to_string()],
            ..Default::default()
        };

        let stats = process_json_result_iterable(&settings, json_iter_in);
        assert_eq!(expected, stats);
    }

    #[test]
    fn simple_process_json_iterable_par() {
        let iter: Vec<IdJSONResult> = vec![
            (1.to_string(), Ok(json!({"key1": 123}))),
            (2.to_string(), Ok(json!({"key2": 123}))),
            (3.to_string(), Ok(json!({"key1": 123}))),
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

        let settings = Settings::init(Cli::default()).unwrap();
        let stats = process_json_result_iterable_par(&settings, iter);
        assert_eq!(expected, stats);
    }

    #[test]
    fn simple_process_json_iterable_par_jsonpath() {
        let iter: Vec<IdJSONResult> = vec![
            (1.to_string(), Ok(json!({"key1": 123}))),
            (2.to_string(), Ok(json!({"a": {"key2": 123}}))),
            (3.to_string(), Ok(json!({"key1": 123}))),
        ];
        let iter = iter.into_iter().par_bridge();

        let expected = Stats {
            keys_count: IndexMap::from([("$.key2".to_string(), 1)]),
            line_count: 1,
            keys_types_count: IndexMap::from([("$.key2::Number".to_string(), 1)]),
            empty_lines: vec![1.to_string(), 3.to_string()],
            ..Default::default()
        };

        let settings = Settings::init(Cli {
            jsonpath: Some("$.a".to_string()),
            ..Cli::default()
        })
        .unwrap();
        let mut stats = process_json_result_iterable_par(&settings, iter);
        stats.empty_lines.sort_by(|a, b| {
            a.parse::<usize>()
                .unwrap()
                .cmp(&b.parse::<usize>().unwrap())
        });
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

        let file_stats = [lhs.clone(), rhs.clone()];
        let actual_ref = lhs.clone() + &rhs;
        let actual = lhs + rhs;

        assert_eq!(actual, expected);
        assert_eq!(actual_ref, expected);
        assert_eq!(file_stats.iter().sum::<Stats>(), expected);
    }
}
