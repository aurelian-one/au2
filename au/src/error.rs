use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuError {
    #[error("'{0}': no such key")]
    NoSuchKey(Box<str>),
    #[error("'{0}': incorrect type, expected {1}")]
    IncorrectType(Box<str>, Box<str>),
    #[error("'{0}': invalid: {1}")]
    InvalidField(Box<str>, Box<str>),
    #[error("'{0}': {1}")]
    InvalidOperation(Box<str>, Box<str>),
    #[error("'{0}': {1}")]
    NestedError(Box<str>, Box<dyn std::error::Error>),
}

// TODO - there's a common practice to further reduce the error scope per function so that
//  the primary public function each produce their own error enum. This can make it more clear
//  when an why particular errors can be thrown.
