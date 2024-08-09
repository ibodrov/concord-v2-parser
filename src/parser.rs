use std::{collections::HashMap, fmt::Display};

use crate::model::{self, ConcordDocument, ConcordFlow, ConcordFlowStep, Value, KV};

pub type Event = yaml_rust2::Event;
pub type Marker = yaml_rust2::scanner::Marker;

pub struct Input {
    items: Vec<(Event, Marker)>,
    idx: usize,
}

impl TryFrom<&str> for Input {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut parser = yaml_rust2::parser::Parser::new(value.chars());
        let mut items = Vec::new();
        loop {
            let (ev, marker) = parser.next_token()?;
            let done = ev == Event::StreamEnd;
            items.push((ev, marker));
            if done {
                break;
            }
        }
        Ok(Input { items, idx: 0 })
    }
}

macro_rules! match_next {
    ($input:ident, $pat:pat) => {
        match $input.next()? {
            (ev @ $pat, marker) => Ok((ev, marker)),
            (ev, marker) => Err(ParseError {
                marker: Some(marker.clone()),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected {}, got {ev:?}", stringify!($pat)),
            }),
        }
    };
}

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

impl Input {
    fn check_eof(&self) -> Result<(), ParseError> {
        if self.idx >= self.items.len() {
            Err(ParseError {
                marker: None,
                kind: ErrorKind::ScanError,
                msg: "EOF".to_string(),
            })
        } else {
            Ok(())
        }
    }

    fn next(&mut self) -> Result<(Event, Marker), ParseError> {
        self.check_eof()?;
        let (event, marker) = &self.items[self.idx];
        self.idx += 1;
        Ok((event.clone(), *marker))
    }

    fn next_stream_start(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::StreamStart)
    }

    fn next_stream_end(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::StreamEnd)
    }

    fn next_document_start(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::DocumentStart)
    }

    fn next_document_end(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::DocumentEnd)
    }

    fn next_mapping_start(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::MappingStart(..))
    }

    fn next_mapping_end(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::MappingEnd)
    }

    fn next_sequence_start(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::SequenceStart(..))
    }

    fn next_sequence_end(&mut self) -> Result<(Event, Marker), ParseError> {
        match_next!(self, Event::SequenceEnd)
    }

    fn next_string(&mut self) -> Result<(String, Marker), ParseError> {
        match self.next()? {
            (Event::Scalar(value, ..), marker) => Ok((value.to_owned(), marker)),
            (ev, marker) => Err(ParseError {
                marker: Some(marker),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected to peek a scalar, got {ev:?}"),
            }),
        }
    }

    fn next_string_constant(&mut self, value: &str) -> Result<(), ParseError> {
        match self.next()? {
            (Event::Scalar(scalar, ..), _) if scalar == value => Ok(()),
            (ev, marker) => Err(ParseError {
                marker: Some(marker),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected a string {value}, got {ev:?}"),
            }),
        }
    }

    fn next_kv(&mut self) -> Result<KV, ParseError> {
        let (key, _) = self.next_string()?;
        let (value, _) = self.next_value()?;
        Ok((key, value))
    }

    fn next_value(&mut self) -> Result<(Value, Marker), ParseError> {
        match self.next()? {
            (Event::Scalar(scalar, style, ..), marker) => {
                use yaml_rust2::scanner::TScalarStyle::*;
                match style {
                    SingleQuoted | DoubleQuoted => Ok((Value::String(scalar), marker)),
                    Plain => {
                        if parse_f64(&scalar).is_ok() {
                            Ok((Value::Float(scalar), marker))
                        } else if let Ok(value) = scalar.parse::<i64>() {
                            Ok((Value::Integer(value), marker))
                        } else if let Ok(value) = scalar.parse::<bool>() {
                            // TODO handle "yes/no", etc
                            Ok((Value::Boolean(value), marker))
                        } else {
                            Ok((Value::String(scalar), marker))
                        }
                    }
                    _ => Err(ParseError {
                        marker: Some(marker),
                        kind: ErrorKind::UnexpectedSyntax,
                        msg: format!("Unsupported value syntax, got \"{scalar}\" as {style:?}"),
                    }),
                }
            }
            (Event::MappingStart(..), marker) => {
                let result = parse_until!(self, Event::MappingEnd, next_kv)
                    .into_iter()
                    .collect();
                self.next_mapping_end()?;
                Ok((Value::Mapping(result), marker))
            }
            (ev, marker) => Err(ParseError {
                marker: Some(marker),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected a value, got {ev:?}"),
            }),
        }
    }

    fn peek(&mut self) -> Result<&(Event, Marker), ParseError> {
        self.check_eof()?;
        Ok(&self.items[self.idx])
    }

    fn peek_string(&mut self) -> Result<Option<(String, Marker)>, ParseError> {
        match self.peek()? {
            (Event::Scalar(value, ..), marker) => Ok(Some((value.to_owned(), *marker))),
            (ev, marker) => Err(ParseError {
                marker: Some(*marker),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected to peek a scalar, got {ev:?}"),
            }),
        }
    }

    fn peek_string_constant(&mut self, value: &str) -> Result<bool, ParseError> {
        match self.peek()? {
            (Event::Scalar(scalar, ..), _) => Ok(scalar == value),
            _ => Ok(false),
        }
    }
}

fn next_kv(input: &mut Input) -> Result<KV, ParseError> {
    input.next_kv()
}

#[derive(Debug)]
pub enum ErrorKind {
    ScanError,
    UnexpectedSyntax,
}

#[derive(Debug)]
pub struct ParseError {
    marker: Option<Marker>,
    kind: ErrorKind,
    msg: String,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} @ {:?}: {}", self.kind, self.marker, self.msg)
    }
}

impl From<yaml_rust2::ScanError> for ParseError {
    fn from(value: yaml_rust2::ScanError) -> Self {
        Self {
            marker: Some(*value.marker()),
            kind: ErrorKind::ScanError,
            msg: value.to_string(),
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
            marker: None,
            kind: ErrorKind::UnexpectedSyntax,
            msg: format!("Invalid float number {value}: {e}"),
        }),
    }
}

fn parse_task_call(input: &mut Input) -> Result<ConcordFlowStep, ParseError> {
    let (name, marker) = input.next_string()?;
    let mut input_parameters = HashMap::new();
    if input.peek_string_constant("in")? {
        input.next()?;
        input.next_mapping_start()?;
        let kvs = parse_until!(input, Event::MappingEnd, next_kv);
        input_parameters.extend(kvs);
        input.next_mapping_end()?;
    };
    Ok(ConcordFlowStep::TaskCall {
        name,
        input: input_parameters,
        marker: marker.into(),
    })
}

impl From<yaml_rust2::scanner::Marker> for model::Marker {
    fn from(value: yaml_rust2::scanner::Marker) -> Self {
        model::Marker {
            index: value.index(),
            line: value.line(),
            col: value.col(),
        }
    }
}

fn parse_step(input: &mut Input) -> Result<ConcordFlowStep, ParseError> {
    input.next_mapping_start()?;

    let step = match input.next()? {
        (Event::Scalar(key, ..), marker) if key == "log" => {
            let (msg, _) = input.next_string()?;
            ConcordFlowStep::TaskCall {
                name: "log".to_owned(),
                input: HashMap::from([("msg".to_owned(), Value::String(msg))]),
                marker: marker.into(),
            }
        }
        (Event::Scalar(key, ..), _) if key == "task" => parse_task_call(input)?,
        (ev, marker) => {
            return Err(ParseError {
                marker: Some(marker),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected a flow step, got {ev:?}"),
            })
        }
    };

    input.next_mapping_end()?;

    Ok(step)
}

fn parse_flow(input: &mut Input) -> Result<ConcordFlow, ParseError> {
    let (name, marker) = input.next_string()?;
    input.next_sequence_start()?;
    let steps = parse_until!(input, Event::SequenceEnd, parse_step);
    input.next_sequence_end()?;
    Ok(ConcordFlow {
        name,
        steps,
        marker: marker.into(),
    })
}

fn parse_flows(input: &mut Input) -> Result<Vec<ConcordFlow>, ParseError> {
    input.next_string_constant("flows")?;
    input.next_mapping_start()?;
    let result = parse_until!(input, Event::MappingEnd, parse_flow);
    input.next_mapping_end()?;
    Ok(result)
}

fn parse_configuration(input: &mut Input) -> Result<Vec<KV>, ParseError> {
    input.next_string_constant("configuration")?;
    input.next_mapping_start()?;
    let result = parse_until!(input, Event::MappingEnd, next_kv);
    Ok(result)
}

fn parse_document(input: &mut Input) -> Result<ConcordDocument, ParseError> {
    input.next_document_start()?;
    input.next_mapping_start()?;

    // top-level elements
    let mut configuration = Vec::new();
    let mut flows = Vec::new();

    while let Ok(Some((top_level_element, marker))) = input.peek_string() {
        match top_level_element.as_str() {
            "configuration" => {
                configuration.extend(parse_configuration(input)?);
                input.next_mapping_end()?;
            }
            "flows" => {
                flows.extend(parse_flows(input)?);
                input.next_mapping_end()?;
            }
            element => {
                return Err(ParseError {
                    marker: Some(marker),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unexpected top-level element {element}"),
                })
            }
        }
    }

    input.next_document_end()?;

    Ok(ConcordDocument { configuration, flows })
}

pub fn parse_stream(input: &mut Input) -> Result<Vec<ConcordDocument>, ParseError> {
    input.next_stream_start()?;
    let result = parse_until!(input, Event::StreamEnd, parse_document);
    input.next_stream_end()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches_float(value: Option<&Value>, expected: &str) -> bool {
        matches!(value, Some(Value::Float(value)) if value == expected)
    }

    #[test]
    fn hello_world() {
        let src = r#"
        flows:
          default:
            - log: "Hello!"
        "#;

        let mut input = Input::try_from(src).unwrap();
        let result = parse_stream(&mut input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].flows.len(), 1);
        assert_eq!(result[0].flows[0].name, "default");
        assert_eq!(result[0].flows[0].steps.len(), 1);
    }

    #[test]
    fn configuration_block() {
        let src = r#"
        configuration:
          foo: "bar"
          baz: 123
        flows:
          default:
            - log: "Hello, ${foo}! Hi, ${baz}!"
        "#;

        let mut input = Input::try_from(src).unwrap();
        let result = parse_stream(&mut input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].configuration[0].0, "foo");
        assert!(matches!(result[0].configuration[0].1, Value::String(ref value) if value == "bar"));
        assert_eq!(result[0].configuration[1].0, "baz");
        assert!(matches!(result[0].configuration[1].1, Value::Float(ref value) if value == "123"));
        assert_eq!(result[0].flows.len(), 1);
        assert_eq!(result[0].flows[0].name, "default");
    }

    #[test]
    fn multiple_flows() {
        let src = r#"
        flows:
          default:
            - log: "Hello!"
          another_one:
            - log: "Yo!"
        "#;

        let mut input = Input::try_from(src).unwrap();
        let result = parse_stream(&mut input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].flows.len(), 2);
        assert_eq!(result[0].flows[0].name, "default");
        assert_eq!(result[0].flows[0].steps.len(), 1);
        assert_eq!(result[0].flows[1].name, "another_one");
        assert_eq!(result[0].flows[1].steps.len(), 1);
    }

    #[test]
    fn multiple_docs() {
        let src = "---\nflows:\n  default:\n    - log: \"Hello!\"\n---\nflows:\n  another_one:\n    - log: \"Bye!\"";

        let mut input = Input::try_from(src).unwrap();
        let result = parse_stream(&mut input).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].flows.len(), 1);
        assert_eq!(result[0].flows[0].name, "default");
        assert_eq!(result[0].flows[0].steps.len(), 1);
        assert_eq!(result[1].flows.len(), 1);
        assert_eq!(result[1].flows[0].name, "another_one");
        assert_eq!(result[1].flows[0].steps.len(), 1);
    }

    #[test]
    fn multiple_steps() {
        let src = r#"
        flows:
          default:
            - log: "Hello!"
            - log: "Bye!"
        "#;

        let mut input = Input::try_from(src).unwrap();
        let result = parse_stream(&mut input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].flows.len(), 1);
        assert_eq!(result[0].flows[0].name, "default");
        assert_eq!(result[0].flows[0].steps.len(), 2);
    }

    #[test]
    fn invalid_top_level_element() {
        let src = r#"
        flows:
          default:
            - log: "Hello!"
        
        gizmos: ["a", 1, false]
        "#;

        let mut input = Input::try_from(src).unwrap();
        let result = parse_stream(&mut input);
        assert!(matches!(result, Err(ParseError { marker: Some(..), .. })));
    }

    #[test]
    fn task_call() {
        let src = r#"
        flows:
          default:
            - task: foo
              in:
                a: 1.23456789
                b: "Hello!"
                c: false
                d:
                  x:
                    y:
                      z: true
        "#;

        let mut input = Input::try_from(src).unwrap();
        let result = parse_stream(&mut input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].flows.len(), 1);
        assert_eq!(result[0].flows[0].steps.len(), 1);
        assert_eq!(result[0].flows[0].marker.line, 3);
        assert!(match &result[0].flows[0].steps[0] {
            ConcordFlowStep::TaskCall { name, input, marker } => {
                assert_eq!(name, "foo");
                assert_eq!(input.len(), 4);
                assert!(matches_float(input.get("a"), "1.23456789"));
                assert!(matches!(input.get("b"), Some(Value::String(value)) if value == "Hello!"));
                assert!(matches!(input.get("c"), Some(Value::Boolean(false))));
                assert!(matches!(input.get("d"), Some(Value::Mapping(..))));
                assert_eq!(marker.line, 4);
                true
            }
        });
    }
}
