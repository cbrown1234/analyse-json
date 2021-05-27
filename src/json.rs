pub mod paths {
    use serde_json::Value;

    pub fn parse_json_paths(json: &Value) -> Vec<String> {
        let root = String::from("$");
        let mut paths = Vec::new();
        _parse_json_paths(json, root, &mut paths);
        paths
    }

    fn _parse_json_paths<'a>(
        json: &Value,
        root: String,
        paths: &'a mut Vec<String>,
    ) -> &'a mut Vec<String> {
        match json {
            Value::Object(map) => {
                for (k, v) in map {
                    let mut obj_root = root.clone();
                    obj_root.push_str(".");
                    obj_root.push_str(k);
                    _parse_json_paths(v, obj_root, paths);
                }
            }
            Value::Null => paths.push(root),
            Value::Bool(_) => paths.push(root),
            Value::Number(_) => paths.push(root),
            Value::String(_) => paths.push(root),
            Value::Array(_) => paths.push(root),
        }
        paths
    }

    #[cfg(test)]
    mod tests {
        use std::str::FromStr;

        use serde_json::Value;
        #[test]
        fn test_parse_json_paths() {
            let v = Value::from_str("{\"key1\": \"value1\", \"key2\": {\"subkey1\": \"value1\"}}")
                .unwrap();
            let v_expected = vec![String::from("$.key1"), String::from("$.key2.subkey1")];
            assert_eq!(super::parse_json_paths(&v), v_expected);
        }
    }
}

pub mod ndjson {
    use serde_json::Value;
    use std::io::{self, prelude::*};
    use std::{collections::HashMap, fs::File};
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
}
