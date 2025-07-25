use crate::error::{ErrorKind, ParseError};
use crate::model::{DocumentPath, Location, Value, KV};
use std::str::Chars;

pub type Event = yaml_rust2::Event;
pub type Marker = yaml_rust2::scanner::Marker;

impl From<(DocumentPath, Marker)> for Location {
    fn from((path, marker): (DocumentPath, Marker)) -> Self {
        Location {
            path,
            index: marker.index(),
            line: marker.line(),
            col: marker.col(),
        }
    }
}

impl From<(DocumentPath, &Marker)> for Location {
    fn from((path, marker): (DocumentPath, &Marker)) -> Self {
        Location {
            path,
            index: marker.index(),
            line: marker.line(),
            col: marker.col(),
        }
    }
}

// from https://github.com/chyh1990/yaml-rust/blob/master/src/yaml.rs
// with minor changes (Option -> Result)
fn parse_f64(value: &str) -> Result<f64, ParseError> {
    match value {
        ".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF" => Ok(f64::INFINITY),
        "-.inf" | "-.Inf" | "-.INF" => Ok(f64::NEG_INFINITY),
        ".nan" | "NaN" | ".NAN" => Ok(f64::NAN),
        _ => value.parse::<f64>().map_err(|e| ParseError {
            location: None,
            kind: ErrorKind::UnexpectedSyntax,
            msg: format!("Invalid float number {value}: {e}"),
        }),
    }
}

pub struct Input<T: Iterator<Item = char>> {
    document_path: Vec<String>,
    yaml: yaml_rust2::parser::Parser<T>,
    eof: bool,
}

impl<'a> TryFrom<&'a str> for Input<Chars<'a>> {
    type Error = ParseError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        let yaml = yaml_rust2::parser::Parser::new(value.chars());
        Ok(Input {
            document_path: Vec::new(),
            yaml,
            eof: false,
        })
    }
}

#[macro_export]
macro_rules! match_next {
    ($input:ident, $pat:pat) => {
        match $input.try_next()? {
            (ev @ $pat, marker) => Ok((ev, marker)),
            (ev, marker) => Err(ParseError {
                location: Some(($input.current_document_path(), marker).into()),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected {}, got {ev:?}", stringify!($pat)),
            }),
        }
    };
}

#[macro_export]
macro_rules! parse_until {
    ($input:ident, $pat:pat, $parser:ident) => {{
        let mut items = Vec::new();
        loop {
            let item = $parser($input)?;
            items.push(item);
            if matches!($input.peek()?, ($pat, _)) {
                break;
            }
        }
        items
    }};
}

impl<T: Iterator<Item = char>> Input<T> {
    pub fn enter_context<S: ToString>(&mut self, name: S) {
        self.document_path.push(name.to_string());
    }

    pub fn leave_context(&mut self) {
        self.document_path.pop();
    }

    pub fn with_context<S: ToString, O, F>(&mut self, name: S, f: F) -> Result<O, ParseError>
    where
        F: Fn(&mut Self) -> Result<O, ParseError>,
    {
        self.enter_context(name);
        let result = f(self)?;
        self.leave_context();
        Ok(result)
    }

    pub fn current_document_path(&self) -> DocumentPath {
        DocumentPath::new(&self.document_path)
    }

    pub fn try_next(&mut self) -> Result<(Event, Marker), ParseError> {
        if self.eof {
            return Err(ParseError {
                location: None,
                kind: ErrorKind::ScanError,
                msg: "EOF".to_owned(),
            });
        }
        let (event, marker) = &self.yaml.next_token()?;
        if matches!(event, Event::StreamEnd) {
            self.eof = true;
        }
        Ok((event.clone(), *marker))
    }

    pub fn next_stream_start(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::StreamStart)
    }

    pub fn next_stream_end(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::StreamEnd)
    }

    pub fn next_document_start(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::DocumentStart)
    }

    pub fn next_document_end(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::DocumentEnd)
    }

    pub fn next_mapping_start(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::MappingStart(..))
    }

    pub fn next_mapping_end(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::MappingEnd)
    }

    pub fn next_sequence_start(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::SequenceStart(..))
    }

    pub fn next_sequence_end(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::SequenceEnd)
    }

    pub fn next_string(&mut self) -> Result<(String, Marker), ParseError> {
        match self.try_next()? {
            (Event::Scalar(value, ..), marker) => Ok((value, marker)),
            (ev, marker) => Err(ParseError {
                location: Some((self.current_document_path(), marker).into()),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected a string value, got {ev:?}"),
            }),
        }
    }

    pub fn next_kv(&mut self) -> Result<KV, ParseError> {
        let (key, marker) = self.next_string()?;
        self.enter_context(format!("'{key}'"));
        let (value, _) = self.next_value()?;
        self.leave_context();
        Ok(KV {
            location: (self.current_document_path(), marker).into(),
            key,
            value,
        })
    }

    pub fn next_value(&mut self) -> Result<(Value, Marker), ParseError> {
        let (event, marker) = self.try_next()?;
        let value = self.parse_value(event, marker)?;
        Ok((value, marker))
    }

    fn parse_value(&mut self, event: Event, marker: Marker) -> Result<Value, ParseError> {
        match event {
            Event::Scalar(scalar, style, ..) => {
                use yaml_rust2::scanner::TScalarStyle::*;
                match style {
                    SingleQuoted | DoubleQuoted => Ok(Value::String(scalar)),
                    Plain => {
                        if scalar.contains(".") && parse_f64(&scalar).is_ok() {
                            Ok(Value::Float(scalar))
                        } else if let Ok(value) = scalar.parse::<i64>() {
                            Ok(Value::Integer(value))
                        } else if let Ok(value) = scalar.parse::<bool>() {
                            // TODO handle "yes/no", etc
                            Ok(Value::Boolean(value))
                        } else {
                            Ok(Value::String(scalar))
                        }
                    }
                    Literal | Folded => Ok(Value::String(scalar)),
                }
            }
            Event::SequenceStart(..) => {
                let result = parse_until!(self, Event::SequenceEnd, next_value)
                    .into_iter()
                    .map(|(v, _)| v)
                    .collect();
                self.next_sequence_end()?;
                Ok(Value::Array(result))
            }
            Event::MappingStart(..) => {
                let result = parse_until!(self, Event::MappingEnd, next_kv)
                    .into_iter()
                    .collect();
                self.next_mapping_end()?;
                Ok(Value::Mapping(result))
            }
            ev => Err(ParseError {
                location: Some((self.current_document_path(), marker).into()),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected a value, got {ev:?}"),
            }),
        }
    }

    pub fn peek(&mut self) -> Result<&(Event, Marker), ParseError> {
        let result = self.yaml.peek()?;
        Ok(result)
    }

    pub fn peek_string(&mut self) -> Result<Option<(String, Marker)>, ParseError> {
        match self.peek().cloned()? {
            (Event::Scalar(value, ..), marker) => Ok(Some((value, marker))),
            (ev, marker) => Err(ParseError {
                location: Some((self.current_document_path(), marker).into()),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected to peek a scalar, got {ev:?}"),
            }),
        }
    }

    pub fn peek_value(&mut self) -> Result<(Value, Marker), ParseError> {
        let (event, marker) = self.peek().cloned()?;
        let value = self.parse_value(event, marker)?;
        Ok((value, marker))
    }
}

pub fn next_value<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<(Value, Marker), ParseError> {
    input.next_value()
}

pub fn next_kv<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<KV, ParseError> {
    input.next_kv()
}
