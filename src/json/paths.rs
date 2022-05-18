use serde_json::{Value, value::Index};

use super::IndexMap;

pub struct ValuePath<'a> {
    value: &'a Value,
    path: Vec<String>,
}

impl<'a> ValuePath<'a> {
    pub fn new(value: &'a Value, parent: Option<&Self>) -> Self {
        let path = if let Some(p) = parent {
            let mut child_path = p.path.clone();
            child_path.push(p.value.to_string());
            child_path
        } else {
            vec!["$".to_string()]
        };
        ValuePath { value, path: path }
    }

    pub fn jsonpath(&self) -> String {
        self.path.join(".")
    }

    pub fn index(&self, index: impl Index + ToString) -> ValuePath {
        let mut child_path = self.path.clone();
        child_path.push(index.to_string());
        ValuePath { value: &self.value[index], path: child_path }
    }
}


// impl<'a, I> std::ops::Index<I> for ValuePath<'a>
// where I: Index + ToString {
//     type Output = ValuePath<'a>;
//     fn index(&self, index: I) -> ValuePath {
//         let mut child_path = self.path.clone();
//         child_path.push(index.to_string());
//         ValuePath { value: &self.value[index], path: child_path }
//     }
// }

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

    use serde_json::json;

    #[test]
    fn basic_valuepath() {
        let v = json!({"key1": "value1", "key2": {"subkey1": "value1"}});
        let vp_0 = ValuePath::new(&v, None);


        assert_eq!(vp_0.jsonpath(), "$".to_string());
        assert_eq!(json!({"b": 1})["a"], Value::Null);

        let v_1 = &v["key2"];
        let vp_1 = vp_0.index("key2");
        assert_eq!(vp_1.value, v_1);
        assert_eq!(vp_1.path, vec!["$".to_string(), "key2".to_string()]);
        assert_eq!(vp_1.jsonpath(), "$.key2".to_string());
    }

    #[test]
    fn basic_valuepath_array() {
        let v = json!({"key1": "value1", "key2": ["a", "b"]});
        let vp_0 = ValuePath::new(&v, None);

        let v_2 = &v["key2"][0];
        let vp_1 = vp_0.index("key2");
        let vp_2 = vp_1.index(0);

        assert_eq!(vp_2.value, v_2);
        assert_eq!(vp_2.path, vec!["$".to_string(), "key2".to_string(), "[0]".to_string()]);
        assert_eq!(vp_1.jsonpath(), "$.key2".to_string());
    }

    #[test]
    fn typical_parse_json_paths() {
        let v = json!({"key1": "value1", "key2": {"subkey1": "value1"}});
        let out_expected = vec![String::from("$.key1"), String::from("$.key2.subkey1")];
        assert_eq!(parse_json_paths(&v), out_expected);
    }

    #[test]
    fn trivial_parse_json_paths() {
        let v = json!(1);
        let out_expected = vec![String::from("$")];
        assert_eq!(parse_json_paths(&v), out_expected);
    }

    #[test]
    fn typical_parse_json_paths_types() {
        let v = json!({"key1": "value1", "key2": {"subkey1": ["value1"]}});
        let mut out_expected = IndexMap::new();
        out_expected.insert("$.key1".to_string(), "String".to_string());
        out_expected.insert("$.key2.subkey1".to_string(), "Array".to_string());
        assert_eq!(parse_json_paths_types(&v), out_expected);
    }

    #[test]
    fn trivial_parse_json_paths_types() {
        let v = json!(1);
        let mut out_expected = IndexMap::new();
        out_expected.insert("$".to_string(), "Number".to_string());
        assert_eq!(parse_json_paths_types(&v), out_expected);
    }
}
