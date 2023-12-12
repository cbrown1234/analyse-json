use thiserror::Error;

pub mod collection;

/// Wrapper around the various errors we can encounter while processing the data
#[derive(Error, Debug)]
pub enum NDJSONError {
    #[error("Failed to read input data")]
    IOError(#[from] std::io::Error),
    #[error("Line failed to parse as valid JSON")]
    JSONParsingError(#[from] serde_json::Error),
    #[error("Line returned empty for the given query")]
    EmptyQuery,
}
