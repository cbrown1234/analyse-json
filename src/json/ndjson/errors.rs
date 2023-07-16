use thiserror::Error;

pub mod collection;

/// Wrapper around the various errors we can encounter while processing the data
#[derive(Error, Debug)]
pub enum NDJSONError {
    #[error("failed to read input data")]
    IOError(#[from] std::io::Error),
    #[error("line failed to parse as valid JSON")]
    JSONParsingError(#[from] serde_json::Error),
    #[error("line returned empty for ther given query")]
    EmptyQuery,
    #[error("line failed due to a JsonPath Query error")]
    QueryJsonPathError(#[from] jsonpath_lib::JsonPathError),
}
