use super::IndexMap;
use dashmap::DashMap;
use rayon::iter::ParallelBridge;
use rayon::prelude::ParallelIterator;
pub use serde_json::Value;
use std::error::Error;
use std::fs::File;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::{
    fmt,
    io::{self, prelude::*},
};

#[derive(Debug, PartialEq, Default)]
pub struct FileStats {
    pub keys_count: IndexMap<String, usize>,
    pub line_count: usize,
    pub bad_lines: Vec<usize>,
    pub keys_types_count: IndexMap<String, usize>,
}

impl FileStats {
    pub fn new() -> FileStats {
        FileStats {
            keys_count: IndexMap::new(),
            line_count: 0,
            bad_lines: Vec::new(),
            keys_types_count: IndexMap::new(),
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
        writeln!(f, "Corrupted lines:\n{:?}", self.bad_lines)
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

fn parse_json_iterable<E>(
    json_iter: impl Iterator<Item = Result<String, E>> + Send,
) -> Result<FileStats, E>
where
    E: Error + Send,
{
    let keys_count: DashMap<String, usize> = DashMap::new();
    let keys_types_count: DashMap<String, usize> = DashMap::new();
    let mut bad_lines: Vec<usize> = Vec::new();
    let bad_lines_mutex = Mutex::new(&mut bad_lines);
    let line_count = AtomicUsize::new(0);

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
            line_count.fetch_max(line_num, Ordering::AcqRel);
        })
        .filter_map(|(_i, j)| j.ok())
        .for_each(|json| {
            for key in json.paths().iter() {
                let mut counter = keys_count.entry(key.to_owned()).or_insert(0);
                *counter.value_mut() += 1;
            }

            for (k, v) in json.path_types().iter() {
                let path_type = format!("{}::{}", k, v);
                let mut counter = keys_types_count.entry(path_type).or_insert(0);
                *counter.value_mut() += 1;
            }
        });

    err?;

    let fs = FileStats {
        keys_count: keys_count
            .into_read_only()
            .iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
        line_count: line_count.load(Ordering::SeqCst),
        keys_types_count: keys_types_count
            .into_read_only()
            .iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
        bad_lines: bad_lines,
    };
    Ok(fs)
}

pub fn parse_ndjson_file(file: File) -> Result<FileStats, std::io::Error> {
    let reader = io::BufReader::new(file);

    parse_json_iterable(reader.lines())
}

pub trait Paths {
    fn paths(&self) -> Vec<String>;
}

impl Paths for Value {
    fn paths(&self) -> Vec<String> {
        super::paths::parse_json_paths(self)
    }
}

pub trait PathTypes {
    fn path_types(&self) -> IndexMap<String, String>;
}

impl PathTypes for Value {
    fn path_types(&self) -> IndexMap<String, String> {
        super::paths::parse_json_paths_types(self)
    }
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
            keys_count: [("$.key1".to_string(), 2), ("$.key2".to_string(), 1)]
                .iter()
                .cloned()
                .collect(),
            line_count: 3,
            keys_types_count: [
                ("$.key1::Number".to_string(), 2),
                ("$.key2::Number".to_string(), 1),
            ]
            .iter()
            .cloned()
            .collect(),
            ..Default::default()
        };

        let file_stats = parse_ndjson_file(tmpfile).unwrap();
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

        let file_stats = parse_ndjson_file(tmpfile).unwrap();
        assert_eq!(expected, file_stats);
    }
}
