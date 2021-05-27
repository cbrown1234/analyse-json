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
