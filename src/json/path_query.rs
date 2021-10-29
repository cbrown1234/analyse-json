use jsonpath_rust::JsonPathFinder;
use serde_json::{json, Value};

pub fn a_test() {
    let finder = JsonPathFinder::from_str(r#"{"first":{"second":[{"active":1},{"passive":1}]}}"#, "$.first.second[?(@.active)]").unwrap();
    let slice_of_data: Vec<&Value> = finder.find_slice();
    assert_eq!(slice_of_data, vec![&json!({"active":1})]);
}

// // use JsonPathFinder;
// // use crate::{JsonPathFinder};
// use jsonpath_rust::JsonPathQuery;

// fn a_test() {
//     let finder = JsonPathFinder::from_str(r#"{"first":{"second":[{"active":1},{"passive":1}]}}"#, "$.first.second[?(@.active)]")?;
//     let slice_of_data: Vec<&Value> = finder.find();
//     assert_eq!(slice_of_data, vec![&json!({"active":1})]);
// }

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    #[test]
    fn basic() {
        a_test();
    }
}
