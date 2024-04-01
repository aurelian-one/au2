use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuError {
    #[error("'{0}': no such key")]
    NoSuchKey(String),
    #[error("'{0}': incorrect type, expected {1}")]
    IncorrectType(String, String),
    #[error("'{0}': {1}")]
    NestedError(String, Box<dyn std::error::Error>),
    #[error("'{0}': invalid: {1}")]
    InvalidField(String, String),
}
