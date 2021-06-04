use serde_json::Value;
use std::{collections::HashMap, fs::File};
use std::{
    fmt,
    io::{self, prelude::*},
};

#[derive(Debug, PartialEq, Default)]
pub struct FileStats {
    pub keys_count: HashMap<String, i32>,
    pub line_count: i64,
    pub bad_lines: Vec<i64>,
}

impl FileStats {
    fn new() -> FileStats {
        FileStats {
            keys_count: std::collections::HashMap::new(),
            line_count: 0,
            bad_lines: Vec::new(),
        }
    }

    pub fn key_occurance(&self) -> HashMap<String, f64> {
        self.keys_count
            .iter()
            .map(|(k, v)| (k.to_owned(), 100f64 * *v as f64 / self.line_count as f64))
            .collect()
    }
}

impl fmt::Display for FileStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Keys:\n{:#?}\n", self.keys_count.keys())?;
        writeln!(f, "Key occurance counts:\n{:#?}", self.keys_count)?;
        write!(f, "Key occurance rate:\n")?;
        for (k, v) in self.key_occurance() {
            writeln!(f, "{}: {}%", k, v)?;
        }
        writeln!(f, "Corrupted lines:\n{:?}", self.bad_lines)
    }
}

pub fn parse_ndjson_file(file: File) -> FileStats {
    let mut fs = FileStats::new();
    let reader = io::BufReader::new(file);

    for line in reader.lines() {
        fs.line_count += 1;
        let line = line.expect(&format!("Failed to read line {}", fs.line_count));

        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                fs.bad_lines.push(fs.line_count);
                continue;
            }
        };

        for key in v.paths().iter() {
            let counter = fs.keys_count.entry(key.to_owned()).or_insert(0);
            *counter += 1;
        }
    }
    fs
}

pub trait Paths {
    fn paths(&self) -> Vec<String>;
}

impl Paths for Value {
    fn paths(&self) -> Vec<String> {
        super::paths::parse_json_paths(&self)
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
            ..Default::default()
        };

        let file_stats = parse_ndjson_file(tmpfile);
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

        let file_stats = parse_ndjson_file(tmpfile);
        assert_eq!(expected, file_stats);
    }
}
