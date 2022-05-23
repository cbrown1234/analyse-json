use serde_json::{Value, value::Index};

use super::IndexMap;

#[derive(Debug, Clone, PartialEq)]
pub struct ValuePath<'a> {
    value: &'a Value,
    path: Vec<String>, // TODO: Switch to Vec<Index> ?
}

impl<'a> ValuePath<'a> {
    pub fn new(value: &'a Value, path: Option<Vec<String>>) -> Self {
        let path = path.unwrap_or(Vec::new());
        ValuePath { value, path }
    }

    pub fn jsonpath(&self) -> String {
        let mut jsonpath = String::from("$");
        for part in &self.path {
            if part.starts_with('[') {
                jsonpath.push_str(&part);
            } else {
                jsonpath.push('.');
                jsonpath.push_str(&part);
            }
        }
        jsonpath
    }

    pub fn index(&self, index: impl JSONPathIndex) -> ValuePath<'a> {
        let mut child_path = self.path.to_vec();
        child_path.push(index.jsonpath());
        ValuePath { value: &self.value[index], path: child_path }
    }
}

pub trait JSONPathIndex: Index {
    fn jsonpath(&self) -> String;
}

impl JSONPathIndex for usize {
    fn jsonpath(&self) -> String {
        format!("[{}]", self).to_string()
    }
}   

impl JSONPathIndex for str {
    fn jsonpath(&self) -> String {
        self.to_string()
    }
}

impl JSONPathIndex for String {
    fn jsonpath(&self) -> String {
        self.to_string()
    }
}

impl<'a, T> JSONPathIndex for &'a T
where T: ?Sized + JSONPathIndex
{
    fn jsonpath(&self) -> String {
        (**self).jsonpath()
    }
}

pub fn parse_value_paths(
    json: &Value,
    explode_array: bool,
) -> Vec<ValuePath> {
    let base_valuepath = ValuePath::new(json, None);
    _parse_value_paths(base_valuepath, explode_array)
}

pub fn _parse_value_paths(
    valuepath: ValuePath,
    explode_array: bool,
) -> Vec<ValuePath> {
    let mut paths = Vec::new();

    match valuepath.value {
        Value::Object(map) => {
            for (k, _) in map {
                let vp = valuepath.index(k);
                let inner_paths = _parse_value_paths(vp, explode_array);
                paths.extend(inner_paths)
            }
        }
        Value::Array(array) => {
            if explode_array {
                for (i, _array_value) in array.iter().enumerate() {
                    let vp = valuepath.index(i);
                    let inner_paths = _parse_value_paths(vp, explode_array);
                    paths.extend(inner_paths)
                }
            } else {
                paths.push(valuepath)
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            paths.push(valuepath)
        }
    }
    paths
}

// impl<'a, I> std::ops::Index<I> for ValuePath<'a>
// where I: JSONPathIndex {
//     type Output = ValuePath<'a>;
//     fn index(&self, index: I) -> &Self::Output {
//         let mut child_path = self.path.clone();
//         child_path.push(index.jsonpath());
//         &ValuePath { value: &self.value[index], path: child_path }
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

pub trait ValuePaths {
    fn value_paths(&self, explode_array: bool) -> Vec<ValuePath>;
}

impl ValuePaths for Value {
    fn value_paths(&self, explode_array: bool) -> Vec<ValuePath> {
        parse_value_paths(self, explode_array)
    }
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

        let v_1 = &v["key2"];
        let vp_1 = vp_0.index("key2");
        assert_eq!(vp_1.value, v_1);
        assert_eq!(vp_1.path, vec!["key2".to_string()]);
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
        assert_eq!(vp_2.path, vec!["key2".to_string(), "[0]".to_string()]);
        assert_eq!(vp_1.jsonpath(), "$.key2".to_string());
        assert_eq!(vp_2.jsonpath(), "$.key2[0]".to_string())
    }

    #[test]
    fn parse_valuepaths() {
        let v = json!({"key1": "value1", "key2": ["a", "b"]});
        let vps = parse_value_paths(&v, false);

        let vp_0 = ValuePath::new(&v, None);
        let vp_1 = ValuePath::new(&v["key1"], Some(vec!["key1".to_string()]));
        let vp_2 = ValuePath::new(&v["key2"], Some(vec!["key2".to_string()]));
        let vp_1_alt = vp_0.index("key1");
        let vp_2_alt = vp_0.index("key2");

        let expected = vec![vp_1, vp_2];
        let expected_alt = vec![vp_1_alt, vp_2_alt];

        assert_eq!(vps, expected);
        assert_eq!(vps, expected_alt);
        assert_eq!(v.value_paths(false), expected);
        assert_eq!(v.value_paths(false), expected_alt);
    }

    #[test]
    fn parse_valuepaths_explode_array() {
        let v = json!({"key1": "value1", "key2": ["a", "b"]});
        let vps = parse_value_paths(&v, true);

        let vp_0 = ValuePath::new(&v, None);
        let vp_1 = ValuePath::new(&v["key1"], Some(vec!["key1".to_string()]));
        let vp_2_1 = ValuePath::new(&v["key2"][0], Some(vec!["key2".to_string(), "[0]".to_string()]));
        let vp_2_2 = ValuePath::new(&v["key2"][1], Some(vec!["key2".to_string(), "[1]".to_string()]));
        let vp_1_alt = vp_0.index("key1");
        let vp_2_1_alt = vp_0.index("key2").index(0);
        let vp_2_2_alt = vp_0.index("key2").index(1);

        assert_eq!(vps, vec![vp_1, vp_2_1, vp_2_2]);
        assert_eq!(vps, vec![vp_1_alt, vp_2_1_alt, vp_2_2_alt]);
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
