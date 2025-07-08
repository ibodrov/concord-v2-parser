use crate::model::{DocumentPath, Location};
use std::fmt::Display;

#[derive(Debug)]
pub enum ErrorKind {
    ScanError,
    UnexpectedSyntax,
}

#[derive(Debug)]
pub struct ParseError {
    pub location: Option<Location>,
    pub kind: ErrorKind,
    pub msg: String,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} @ {:?}: {}", self.kind, self.location, self.msg)
    }
}

impl From<yaml_rust2::ScanError> for ParseError {
    fn from(value: yaml_rust2::ScanError) -> Self {
        Self {
            location: Some((DocumentPath::none(), value.marker()).into()),
            kind: ErrorKind::ScanError,
            msg: value.to_string(),
        }
    }
}
