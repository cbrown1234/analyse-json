use serde_json::Value;

use super::IndexMap;

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
                obj_root.push('.');
                obj_root.push_str(k);
                _parse_json_paths(v, obj_root, paths);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            paths.push(root)
        }
    }
    paths
}

pub fn parse_json_paths_types(json: &Value) -> IndexMap<String, String> {
    let root = String::from("$");
    let mut paths_types = IndexMap::new();
    _parse_json_paths_types(json, root, &mut paths_types);
    paths_types
}

fn _parse_json_paths_types<'a>(
    json: &Value,
    root: String,
    paths_types: &'a mut IndexMap<String, String>,
) -> &'a mut IndexMap<String, String> {
    match json {
        Value::Object(map) => {
            for (k, v) in map {
                let mut obj_root = root.clone();
                obj_root.push('.');
                obj_root.push_str(k);
                _parse_json_paths_types(v, obj_root, paths_types);
            }
        }
        Value::Null => {
            paths_types.insert(root, "Null".to_string());
        }
        Value::Bool(_) => {
            paths_types.insert(root, "Bool".to_string());
        }
        Value::Number(_) => {
            paths_types.insert(root, "Number".to_string());
        }
        Value::String(_) => {
            paths_types.insert(root, "String".to_string());
        }
        Value::Array(_) => {
            paths_types.insert(root, "Array".to_string());
        }
    }
    paths_types
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    use serde_json::Value;
    #[test]
    fn typical_parse_json_paths() {
        let v = Value::from_str(r#"{"key1": "value1", "key2": {"subkey1": "value1"}}"#).unwrap();
        let out_expected = vec![String::from("$.key1"), String::from("$.key2.subkey1")];
        assert_eq!(parse_json_paths(&v), out_expected);
    }

    #[test]
    fn trivial_parse_json_paths() {
        let v = Value::from_str("1").unwrap();
        let out_expected = vec![String::from("$")];
        assert_eq!(parse_json_paths(&v), out_expected);
    }

    #[test]
    fn typical_parse_json_paths_types() {
        let v = Value::from_str(r#"{"key1": "value1", "key2": {"subkey1": ["value1"]}}"#).unwrap();
        let mut out_expected = IndexMap::new();
        out_expected.insert("$.key1".to_string(), "String".to_string());
        out_expected.insert("$.key2.subkey1".to_string(), "Array".to_string());
        assert_eq!(parse_json_paths_types(&v), out_expected);
    }

    #[test]
    fn trivial_parse_json_paths_types() {
        let v = Value::from_str("1").unwrap();
        let mut out_expected = IndexMap::new();
        out_expected.insert("$".to_string(), "Number".to_string());
        assert_eq!(parse_json_paths_types(&v), out_expected);
    }
}
