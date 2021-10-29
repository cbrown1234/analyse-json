use super::IndexMap;
pub use serde_json::{Value, json};
use std::fs::File;
use std::{
    fmt,
    io::{self, prelude::*},
};
use jsonpath_rust::{JsonPathFinder, JsonPathQuery};
use jsonpath_rust::parser::parser::parse_json_path;
use std::error::Error;

#[derive(Debug, PartialEq, Default)]
pub struct FileStats {
    pub keys_count: IndexMap<String, usize>,
    pub line_count: usize,
    pub bad_lines: Vec<usize>,
    pub keys_types_count: IndexMap<String, usize>,
}

impl FileStats {
    fn new() -> FileStats {
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

fn parse_json_iterable<E>(
    json_iter: impl IntoIterator<Item = Result<String, E>>,
) -> Result<FileStats, E> {
    let mut fs = FileStats::new();

    for (i, json_candidate) in json_iter.into_iter().enumerate() {
        let json_candidate = json_candidate?;
        let iter_number = i + 1;
        fs.line_count = iter_number;

        let json: Value = match serde_json::from_str(&json_candidate) {
            Ok(v) => v,
            Err(_) => {
                fs.bad_lines.push(iter_number);
                continue;
            }
        };

        for key in json.paths().iter() {
            let counter = fs.keys_count.entry(key.to_owned()).or_insert(0);
            *counter += 1;
        }

        for (k, v) in json.path_types().iter() {
            let path_type = format!("{}::{}", k, v);
            let counter = fs.keys_types_count.entry(path_type).or_insert(0);
            *counter += 1;
        }
    }
    Ok(fs)
}

pub fn parse_ndjson_file(file: File) -> Result<FileStats, std::io::Error> {
    let reader = io::BufReader::new(file);

    Ok(parse_json_iterable(reader.lines())?)
}

pub trait Paths {
    fn paths(&self) -> Vec<String>;
}

impl Paths for Value {
    fn paths(&self) -> Vec<String> {
        super::paths::parse_json_paths(&self)
    }
}

pub trait PathTypes {
    fn path_types(&self) -> IndexMap<String, String>;
}

impl PathTypes for Value {
    fn path_types(&self) -> IndexMap<String, String> {
        super::paths::parse_json_paths_types(&self)
    }
}


// fn filter(json_iter: impl IntoIterator<Item = Value>, path : &str) -> Result<impl Iterator<Item = Vec<&Value>>, String> {
//     // let path = parse_json_path(path);
//     let mut finder = JsonPathFinder::from_str("1", path)?;
//     // for json in json_iter {
//     //     let x = json.path(path)?;
//     // }
//     // json_iter.into_iter().map(move |json| {json.path(path)})
//     // let path = parse_json_path(path).map_err(|e| e.to_string())?;
//     Ok(json_iter.into_iter().map(|json| {
//         finder.set_json(json);
//         finder.find_slice()
//     }))
// }

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
