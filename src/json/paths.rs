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
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            paths.push(root)
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    use serde_json::Value;
    #[test]
    fn typical_parse_json_paths() {
        let v =
            Value::from_str("{\"key1\": \"value1\", \"key2\": {\"subkey1\": \"value1\"}}").unwrap();
        let v_expected = vec![String::from("$.key1"), String::from("$.key2.subkey1")];
        assert_eq!(parse_json_paths(&v), v_expected);
    }

    #[test]
    fn trivial_parse_json_paths() {
        let v = Value::from_str("1").unwrap();
        let v_expected = vec![String::from("$")];
        assert_eq!(parse_json_paths(&v), v_expected);
    }
}
