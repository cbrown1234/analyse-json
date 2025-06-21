use super::serde_json::value::Index;
use super::{IndexMap, Value, ValueType};

/// Wrapper around [`Value`] keeping track of its location within the root parent JSON
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuePath<'a> {
    pub value: &'a Value,
    pub path: Vec<String>, // TODO: Switch to Vec<Index> ?
}

impl<'a> ValuePath<'a> {
    /// Returns a [`ValuePath`] (Serde JSON [`Value`] wrapper) useful for tracking the location
    /// of indexes into the JSON
    ///
    /// # Arguments
    /// * `value` - Serde JSON Value
    /// * `path` - Vec of Strings, components of the JSON path to self.value from the root ($)
    ///
    /// # Examples
    ///
    /// ```
    /// use analyse_json::json::paths::ValuePath;
    ///
    /// let value = serde_json::json!({"key": "value"});
    /// let vp = ValuePath::new(&value, None);
    /// ```
    pub fn new(value: &'a Value, path: Option<Vec<String>>) -> ValuePath<'a> {
        let path = path.unwrap_or_default();
        ValuePath { value, path }
    }

    /// JSONpath of `value`s location within the root parent JSON
    pub fn jsonpath(&self) -> String {
        let mut jsonpath = String::from("$");
        for part in &self.path {
            if part.starts_with('[') {
                jsonpath.push_str(part);
            } else {
                jsonpath.push('.');
                jsonpath.push_str(part);
            }
        }
        jsonpath
    }

    // Work around until indexing via a trait is supported
    // https://github.com/rust-lang/rfcs/issues/997
    /// Index into the inner value, tracks the jsonpath location
    pub fn index(&self, index: impl JSONPathIndex) -> ValuePath<'a> {
        let mut child_path = self.path.to_vec();
        child_path.push(index.jsonpath());
        ValuePath {
            value: &self.value[index],
            path: child_path,
        }
    }

    /// Index into the inner value, enables custom override to the tracked jsonpath location
    pub fn index_custom(
        &self,
        index: impl Index,
        index_custom: impl JSONPathIndex,
    ) -> ValuePath<'a> {
        let mut child_path = self.path.to_vec();
        child_path.push(index_custom.jsonpath());
        ValuePath {
            value: &self.value[index],
            path: child_path,
        }
    }

    /// Lists all of the `ValuePath`s children by walking the inner value.
    /// Includes flags for how to walk arrays
    fn value_paths(self, explode_array: bool, inspect_arrays: bool) -> Vec<ValuePath<'a>> {
        let mut paths = Vec::new();

        match self.value {
            Value::Object(map) => {
                for (k, _) in map {
                    let vp = self.index(k);
                    let inner_paths = vp.value_paths(explode_array, inspect_arrays);
                    paths.extend(inner_paths)
                }
            }
            Value::Array(array) => {
                if inspect_arrays {
                    for (i, _array_value) in array.iter().enumerate() {
                        let vp = self.index_custom(i, "[*]");
                        let inner_paths = vp.value_paths(explode_array, inspect_arrays);
                        paths.extend(inner_paths)
                    }
                } else if explode_array {
                    for (i, _array_value) in array.iter().enumerate() {
                        let vp = self.index(i);
                        let inner_paths = vp.value_paths(explode_array, inspect_arrays);
                        paths.extend(inner_paths)
                    }
                } else {
                    paths.push(self)
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => paths.push(self),
        }
        paths
    }
}

pub trait JSONPathIndex: Index {
    fn jsonpath(&self) -> String;
}

impl JSONPathIndex for usize {
    fn jsonpath(&self) -> String {
        format!("[{}]", self)
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

impl<T> JSONPathIndex for &T
where
    T: ?Sized + JSONPathIndex,
{
    fn jsonpath(&self) -> String {
        (**self).jsonpath()
    }
}

pub trait ValuePaths {
    fn value_paths(&self, explode_array: bool, inspect_arrays: bool) -> Vec<ValuePath>;
}

impl ValuePaths for Value {
    /// Lists all of the `ValuePath` children by walking `Value`.
    ///
    /// Useful for generating full lists of JSONpaths within the Value
    /// with [`ValuePath::jsonpath`]
    ///
    /// See also [`Value::json_paths`] from [`JSONPaths`]
    fn value_paths(&self, explode_array: bool, inspect_arrays: bool) -> Vec<ValuePath> {
        let base_valuepath = ValuePath::new(self, None);
        base_valuepath.value_paths(explode_array, inspect_arrays)
    }
}

// See https://github.com/rust-lang/rfcs/issues/997
// impl<'a, I> std::ops::Index<I> for ValuePath<'a>
// where I: JSONPathIndex {
//     type Output = ValuePath<'a>;
//     fn index(&self, index: I) -> &Self::Output {
//         let mut child_path = self.path.clone();
//         child_path.push(index.jsonpath());
//         &ValuePath { value: &self.value[index], path: child_path }
//     }
// }

pub trait JSONPaths {
    /// Lists all of the json_paths by walking a JSON `self`.
    /// Includes flags for how to walk arrays
    fn json_paths(&self, explode_array: bool, inspect_arrays: bool) -> Vec<String>;

    /// Maps all of the json_paths to the type they contain by walking a JSON `self`.
    /// Includes flags for how to walk arrays
    fn json_paths_types(
        &self,
        explode_array: bool,
        inspect_arrays: bool,
    ) -> IndexMap<String, String>;
}

impl JSONPaths for Value {
    fn json_paths(&self, explode_array: bool, inspect_arrays: bool) -> Vec<String> {
        self.value_paths(explode_array, inspect_arrays)
            .into_iter()
            .map(|value_path| value_path.jsonpath())
            .collect()
    }

    fn json_paths_types(
        &self,
        explode_array: bool,
        inspect_arrays: bool,
    ) -> IndexMap<String, String> {
        self.value_paths(explode_array, inspect_arrays)
            .into_iter()
            .map(|value_path| (value_path.jsonpath(), value_path.value.value_type()))
            .collect()
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
        let vps = v.value_paths(false, false);

        let vp_0 = ValuePath::new(&v, None);
        let vp_1 = ValuePath::new(&v["key1"], Some(vec!["key1".to_string()]));
        let vp_2 = ValuePath::new(&v["key2"], Some(vec!["key2".to_string()]));
        let vp_1_alt = vp_0.index("key1");
        let vp_2_alt = vp_0.index("key2");

        let expected = vec![vp_1, vp_2];
        let expected_alt = vec![vp_1_alt, vp_2_alt];

        assert_eq!(vps, expected);
        assert_eq!(vps, expected_alt);
        assert_eq!(v.value_paths(false, false), expected);
        assert_eq!(v.value_paths(false, false), expected_alt);
    }

    #[test]
    fn parse_valuepaths_explode_array() {
        let v = json!({"key1": "value1", "key2": ["a", "b"]});
        let vps = v.value_paths(true, false);

        let vp_0 = ValuePath::new(&v, None);
        let vp_1 = ValuePath::new(&v["key1"], Some(vec!["key1".to_string()]));
        let vp_2_1 = ValuePath::new(
            &v["key2"][0],
            Some(vec!["key2".to_string(), "[0]".to_string()]),
        );
        let vp_2_2 = ValuePath::new(
            &v["key2"][1],
            Some(vec!["key2".to_string(), "[1]".to_string()]),
        );
        let vp_1_alt = vp_0.index("key1");
        let vp_2_1_alt = vp_0.index("key2").index(0);
        let vp_2_2_alt = vp_0.index("key2").index(1);

        assert_eq!(vps, vec![vp_1, vp_2_1, vp_2_2]);
        assert_eq!(vps, vec![vp_1_alt, vp_2_1_alt, vp_2_2_alt]);
    }

    #[test]
    fn parse_valuepaths_inspect_array() {
        let v = json!({"key1": "value1", "key2": ["a", "b"]});
        let vps = v.value_paths(false, true);

        let vp_1 = ValuePath::new(&v["key1"], Some(vec!["key1".to_string()]));
        let vp_2_1 = ValuePath::new(
            &v["key2"][0],
            Some(vec!["key2".to_string(), "[*]".to_string()]),
        );
        let vp_2_2 = ValuePath::new(
            &v["key2"][1],
            Some(vec!["key2".to_string(), "[*]".to_string()]),
        );

        assert_eq!(vps, vec![vp_1, vp_2_1, vp_2_2]);
    }

    #[test]
    fn typical_parse_json_paths() {
        let v = json!({"key1": "value1", "key2": {"subkey1": "value1"}});
        let out_expected = vec![String::from("$.key1"), String::from("$.key2.subkey1")];
        assert_eq!(v.json_paths(false, false), out_expected);
    }

    #[test]
    fn trivial_parse_json_paths() {
        let v = json!(1);
        let out_expected = vec![String::from("$")];
        assert_eq!(v.json_paths(false, false), out_expected);
    }

    #[test]
    fn typical_parse_json_paths_types() {
        let v = json!({"key1": "value1", "key2": {"subkey1": ["value1"]}});
        let mut out_expected = IndexMap::new();
        out_expected.insert("$.key1".to_string(), "String".to_string());
        out_expected.insert("$.key2.subkey1".to_string(), "Array".to_string());
        assert_eq!(v.json_paths_types(false, false), out_expected);
    }

    #[test]
    fn trivial_parse_json_paths_types() {
        let v = json!(1);
        let mut out_expected = IndexMap::new();
        out_expected.insert("$".to_string(), "Number".to_string());
        assert_eq!(v.json_paths_types(false, false), out_expected);
    }
}
