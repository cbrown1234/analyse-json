use crate::json::paths::ValuePaths;
use crate::json::ValueType;
use crate::Cli;

use super::IndexMap;
use dashmap::DashMap;
use rayon::iter::ParallelBridge;
use rayon::prelude::ParallelIterator;
pub use serde_json::Value;
use std::error::{self, Error};
use std::fs::File;
use std::iter::Sum;
use std::ops::Add;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::{
    fmt,
    io::{self, prelude::*},
};

// TODO: extract stats to separate struct or add "file" id to *_lines
#[derive(Debug, PartialEq, Default, Clone)]
pub struct FileStats {
    pub keys_count: IndexMap<String, usize>,
    pub line_count: usize,
    pub bad_lines: Vec<usize>,
    pub keys_types_count: IndexMap<String, usize>,
    pub empty_lines: Vec<usize>,
}

impl FileStats {
    pub fn new() -> FileStats {
        FileStats {
            keys_count: IndexMap::new(),
            line_count: 0,
            bad_lines: Vec::new(),
            keys_types_count: IndexMap::new(),
            empty_lines: Vec::new(),
        }
    }

    pub fn key_occurance(&self) -> IndexMap<String, f64> {
        self.keys_count
            .iter()
            .map(|(k, v)| (k.to_owned(), 100f64 * *v as f64 / self.line_count as f64))
            .collect()
    }

    pub fn key_type_occurance(&self) -> IndexMap<String, f64> {
        self.keys_types_count
            .iter()
            .map(|(k, v)| (k.to_owned(), 100f64 * *v as f64 / self.line_count as f64))
            .collect()
    }
}

impl fmt::Display for FileStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Keys:\n{:#?}\n", self.keys_count.keys())?;
        writeln!(f, "Key occurance counts:\n{:#?}", self.keys_count)?;
        writeln!(f, "Key occurance rate:")?;
        for (k, v) in self.key_occurance() {
            writeln!(f, "{}: {}%", k, v)?;
        }
        writeln!(f, "Key type occurance rate:")?;
        for (k, v) in self.key_type_occurance() {
            writeln!(f, "{}: {}%", k, v)?;
        }
        writeln!(f, "Corrupted lines:\n{:?}", self.bad_lines)?;
        writeln!(f, "Empty lines:\n{:?}", self.empty_lines)
    }
}

impl Add for FileStats {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let mut output = self;

        for (k, v) in rhs.keys_count {
            let counter = output.keys_count.entry(k).or_insert(0);
            *counter += v
        }

        for (k, v) in rhs.keys_types_count {
            let counter = output.keys_types_count.entry(k).or_insert(0);
            *counter += v
        }

        output.line_count += rhs.line_count;

        // Not sure these are compatible
        output.bad_lines = Vec::new();
        output.empty_lines = Vec::new();

        output
    }
}

impl Add<&Self> for FileStats {
    type Output = Self;

    fn add(self, rhs: &Self) -> Self::Output {
        self.add(rhs.clone())
    }
}

impl<'a> Sum<&'a Self> for FileStats {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(Self::default(), |acc, x| acc + x)
    }
}

// https://stackoverflow.com/questions/26368288/how-do-i-stop-iteration-and-return-an-error-when-iteratormap-returns-a-result
fn until_err<T, E>(err: &mut &mut Result<(), E>, item: Result<T, E>) -> Option<T> {
    match item {
        Ok(item) => Some(item),
        Err(e) => {
            **err = Err(e);
            None
        }
    }
}

pub fn parse_json_iterable<E: 'static + Error>(
    args: &Cli,
    json_iter: impl Iterator<Item = Result<String, E>>,
) -> Result<FileStats, Box<dyn error::Error>> {
    let mut fs = FileStats::new();

    let json_iter = parse_iter(args, json_iter);
    let jsonpath = args.jsonpath_selector()?;

    for (i, json_candidate) in json_iter.enumerate() {
        let json_candidate = json_candidate?;
        let iter_number = i + 1;
        fs.line_count = iter_number;

        let mut json: Value = match serde_json::from_str(&json_candidate) {
            Ok(v) => v,
            Err(_) => {
                fs.bad_lines.push(iter_number);
                continue;
            }
        };

        if let Some(ref selector) = jsonpath {
            let mut json_list = selector.find(&json);
            if let Some(json_1) = json_list.next() {
                // TODO: handle multiple search results
                assert_eq!(None, json_list.next());
                json = json_1.to_owned()
            } else {
                fs.empty_lines.push(iter_number);
                continue;
            }
        }

        for value_path in json.value_paths(args.explode_arrays) {
            let path = value_path.jsonpath();
            let counter = fs.keys_count.entry(path.to_owned()).or_insert(0);
            *counter += 1;

            let type_ = value_path.value.value_type();
            let path_type = format!("{}::{}", path, type_);
            let counter = fs.keys_types_count.entry(path_type).or_insert(0);
            *counter += 1;
        }
    }
    Ok(fs)
}

pub fn parse_json_iterable_par<E>(
    args: &Cli,
    json_iter: impl Iterator<Item = Result<String, E>> + Send,
) -> Result<FileStats, Box<dyn error::Error>>
where
    E: 'static + Error + Send,
{
    let keys_count: DashMap<String, usize> = DashMap::new();
    let keys_types_count: DashMap<String, usize> = DashMap::new();
    let mut bad_lines: Vec<usize> = Vec::new();
    let bad_lines_mutex = Mutex::new(&mut bad_lines);
    let line_count = AtomicUsize::new(0);
    let mut empty_lines: Vec<usize> = Vec::new();
    let empty_lines_mutex = Mutex::new(&mut empty_lines);

    let json_iter = parse_iter(args, json_iter);
    let jsonpath = args.jsonpath_selector()?;

    // Bubble up upstream errors
    let mut err = Ok(());
    let json_iter = json_iter.scan(&mut err, until_err);

    json_iter
        .enumerate()
        .par_bridge()
        .map(|(i, json_candidate)| (i, serde_json::from_str(&json_candidate)))
        .inspect(|(i, j): &(usize, Result<Value, serde_json::Error>)| {
            let line_num = i + 1;
            if j.is_err() {
                let mut bad_lines = bad_lines_mutex.lock().unwrap();
                bad_lines.push(line_num);
            }
            line_count.fetch_max(line_num, Ordering::Release);
        })
        .filter(|(_i, j)| j.is_ok())
        .map(|(i, j)| (i, j.unwrap()))
        .for_each(|(i, mut json)| {
            let mut continue_ = false;
            if let Some(ref selector) = jsonpath {
                let mut json_list = selector.find(&json);
                if let Some(json_1) = json_list.next() {
                    // TODO: handle multiple search results
                    assert_eq!(None, json_list.next());
                    json = json_1.to_owned()
                } else {
                    let line_num = i + 1;
                    let mut empty_lines = empty_lines_mutex.lock().unwrap();
                    empty_lines.push(line_num);
                    // continue; doesn't work in for_each
                    continue_ = true;
                }
            }
            if !continue_ {
                for value_path in json.value_paths(args.explode_arrays) {
                    let path = value_path.jsonpath();
                    let mut counter = keys_count.entry(path.to_owned()).or_insert(0);
                    *counter.value_mut() += 1;

                    let type_ = value_path.value.value_type();
                    let path_type = format!("{}::{}", path, type_);
                    let mut counter = keys_types_count.entry(path_type).or_insert(0);
                    *counter.value_mut() += 1;
                }
            }
        });

    err?;

    let fs = FileStats {
        keys_count: keys_count
            .into_read_only()
            .iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
        line_count: line_count.load(Ordering::Acquire),
        keys_types_count: keys_types_count
            .into_read_only()
            .iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
        bad_lines,
        empty_lines,
    };
    Ok(fs)
}

// // TODO: impliment method to handle
// trait Stats {
//     fn stats(&self) -> FileStats;
// }

// impl<T: impl Iterator<Item = Result<String, E>> + Send> Stats for T {
//     fn stats(&self) {
//         parse_json_iterable_par(&self)
//     }
// }

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

pub fn parse_ndjson_bufreader(
    args: &Cli,
    bufreader: impl BufRead + Send,
) -> Result<FileStats, Box<dyn error::Error>> {
    let json_iter = bufreader.lines();
    parse_json_iterable_par(args, json_iter)
}

pub fn parse_ndjson_file(args: &Cli, file: File) -> Result<FileStats, Box<dyn error::Error>> {
    // if file.metadata().
    parse_ndjson_bufreader(args, io::BufReader::new(file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::{Seek, SeekFrom, Write};

    #[test]
    fn simple_ndjson_file() {
        let mut tmpfile: File = tempfile::tempfile().unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key2": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        tmpfile.seek(SeekFrom::Start(0)).unwrap();

        let expected = FileStats {
            keys_count: IndexMap::from([("$.key1".to_string(), 2), ("$.key2".to_string(), 1)]),
            line_count: 3,
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 2),
                ("$.key2::Number".to_string(), 1),
            ]),
            ..Default::default()
        };

        let args = Cli::default();
        let file_stats = parse_ndjson_file(&args, tmpfile).unwrap();
        assert_eq!(expected, file_stats);
    }

    #[test]
    fn simple_ndjson_iterable() {
        let iter: Vec<Result<String, std::io::Error>> = vec![
            Ok(r#"{"key1": 123}"#.to_string()),
            Ok(r#"{"key2": 123}"#.to_string()),
            Ok(r#"{"key1": 123}"#.to_string()),
        ];
        let iter = iter.into_iter();

        let expected = FileStats {
            keys_count: IndexMap::from([("$.key1".to_string(), 2), ("$.key2".to_string(), 1)]),
            line_count: 3,
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 2),
                ("$.key2::Number".to_string(), 1),
            ]),
            ..Default::default()
        };

        let args = Cli::default();
        let file_stats = parse_json_iterable(&args, iter).unwrap();
        assert_eq!(expected, file_stats);
    }

    #[test]
    fn simple_ndjson_iterable_par() {
        let iter: Vec<Result<String, std::io::Error>> = vec![
            Ok(r#"{"key1": 123}"#.to_string()),
            Ok(r#"{"key2": 123}"#.to_string()),
            Ok(r#"{"key1": 123}"#.to_string()),
        ];
        let iter = iter.into_iter();

        let expected = FileStats {
            keys_count: IndexMap::from([("$.key1".to_string(), 2), ("$.key2".to_string(), 1)]),
            line_count: 3,
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 2),
                ("$.key2::Number".to_string(), 1),
            ]),
            ..Default::default()
        };

        let args = Cli::default();
        let file_stats = parse_json_iterable_par(&args, iter).unwrap();
        assert_eq!(expected, file_stats);
    }

    #[test]
    fn bad_ndjson_file() {
        let mut tmpfile: File = tempfile::tempfile().unwrap();
        writeln!(tmpfile, "{{").unwrap();
        tmpfile.seek(SeekFrom::Start(0)).unwrap();

        let expected = FileStats {
            bad_lines: vec![1],
            line_count: 1,
            ..Default::default()
        };

        let args = Cli::default();
        let file_stats = parse_ndjson_file(&args, tmpfile).unwrap();
        assert_eq!(expected, file_stats);
    }

    #[test]
    fn simple_ndjson_iterable_jsonpath() {
        let iter: Vec<Result<String, std::io::Error>> = vec![
            Ok(r#"{"key1": 123}"#.to_string()),
            Ok(r#"{"a": {"key2": 123}}"#.to_string()),
            Ok(r#"{"key1": 123}"#.to_string()),
        ];
        let iter = iter.into_iter();

        let expected = FileStats {
            keys_count: IndexMap::from([("$.key2".to_string(), 1)]),
            line_count: 3,
            keys_types_count: IndexMap::from([("$.key2::Number".to_string(), 1)]),
            empty_lines: vec![1, 3],
            ..Default::default()
        };

        let mut args = Cli::default();
        args.jsonpath = Some("$.a".to_string());
        let file_stats = parse_json_iterable(&args, iter).unwrap();
        assert_eq!(expected, file_stats);
    }

    #[test]
    fn simple_ndjson_iterable_par_jsonpath() {
        let iter: Vec<Result<String, std::io::Error>> = vec![
            Ok(r#"{"key1": 123}"#.to_string()),
            Ok(r#"{"a": {"key2": 123}}"#.to_string()),
            Ok(r#"{"key1": 123}"#.to_string()),
        ];
        let iter = iter.into_iter();

        let expected = FileStats {
            keys_count: IndexMap::from([("$.key2".to_string(), 1)]),
            line_count: 3,
            keys_types_count: IndexMap::from([("$.key2::Number".to_string(), 1)]),
            empty_lines: vec![1, 3],
            ..Default::default()
        };

        let mut args = Cli::default();
        args.jsonpath = Some("$.a".to_string());
        let file_stats = parse_json_iterable_par(&args, iter).unwrap();
        assert_eq!(expected, file_stats);
    }

    #[test]
    fn add_filestats() {
        let lhs = FileStats {
            keys_count: IndexMap::from([("$.key1".to_string(), 3), ("$.key2".to_string(), 2)]),
            line_count: 3,
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 3),
                ("$.key2::Number".to_string(), 2),
            ]),
            ..Default::default()
        };
        let rhs = FileStats {
            keys_count: IndexMap::from([("$.key3".to_string(), 3), ("$.key2".to_string(), 2)]),
            line_count: 3,
            keys_types_count: IndexMap::from([
                ("$.key3::Number".to_string(), 3),
                ("$.key2::Number".to_string(), 2),
            ]),
            ..Default::default()
        };
        let expected = FileStats {
            keys_count: IndexMap::from([
                ("$.key1".to_string(), 3),
                ("$.key2".to_string(), 4),
                ("$.key3".to_string(), 3),
            ]),
            line_count: 6,
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 3),
                ("$.key2::Number".to_string(), 4),
                ("$.key3::Number".to_string(), 3),
            ]),
            ..Default::default()
        };

        let actual = lhs + rhs;

        assert_eq!(actual, expected)
    }
}
