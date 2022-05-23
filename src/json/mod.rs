pub use indexmap::IndexMap;
use serde_json::Value;

pub mod ndjson;
pub mod paths;

pub fn value_type(value: &Value) -> String {
    match value {
        Value::Object(_) => {
            "Object".to_string()
        }
        Value::Null => {
            "Null".to_string()
        }
        Value::Bool(_) => {
            "Bool".to_string()
        }
        Value::Number(_) => {
            "Number".to_string()
        }
        Value::String(_) => {
            "String".to_string()
        }
        Value::Array(_) => {
            "Array".to_string()
        }
    }
}

trait ValueType {
    fn value_type(&self) -> String;
}

impl ValueType for Value {
    fn value_type(&self) -> String {
        value_type(self)
    }
}
