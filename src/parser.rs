use crate::model::{
    ConcordDocument, Configuration, DocumentPath, Flow, FlowStep, Form, FormField, Location, Value, KV,
};
use std::fmt::Display;
use std::str::Chars;

pub type Event = yaml_rust2::Event;
pub type Marker = yaml_rust2::scanner::Marker;

pub struct Input<T: Iterator<Item = char>> {
    document_path: Vec<String>,
    yaml: yaml_rust2::parser::Parser<T>,
}

impl<'a> TryFrom<&'a str> for Input<Chars<'a>> {
    type Error = ParseError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        let yaml = yaml_rust2::parser::Parser::new(value.chars());
        Ok(Input {
            document_path: Vec::new(),
            yaml,
        })
    }
}

macro_rules! match_next {
    ($input:ident, $pat:pat) => {
        match $input.next()? {
            (ev @ $pat, marker) => Ok((ev, marker)),
            (ev, marker) => Err(ParseError {
                location: Some(($input.current_document_path(), marker).into()),
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

impl<T: Iterator<Item = char>> Input<T> {
    fn enter_context(&mut self, name: &str) {
        self.document_path.push(name.to_owned());
    }

    fn leave_context(&mut self) {
        self.document_path.pop();
    }

    fn current_document_path(&self) -> DocumentPath {
        DocumentPath::new(&self.document_path)
    }

    fn next(&mut self) -> Result<(Event, Marker), ParseError> {
        let (event, marker) = &self.yaml.next_token()?;
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
                location: Some((self.current_document_path(), marker).into()),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected a string value, got {ev:?}"),
            }),
        }
    }

    fn next_string_constant(&mut self, value: &str) -> Result<Marker, ParseError> {
        match self.next()? {
            (Event::Scalar(scalar, ..), marker) if scalar == value => Ok(marker),
            (ev, marker) => Err(ParseError {
                location: Some((self.current_document_path(), marker).into()),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected a string {value}, got {ev:?}"),
            }),
        }
    }

    fn next_kv(&mut self) -> Result<KV, ParseError> {
        let (key, marker) = self.next_string()?;
        self.enter_context(&format!("'{key}'"));
        let value = self.next_value()?;
        self.leave_context();
        Ok(KV {
            location: (self.current_document_path(), marker).into(),
            key,
            value,
        })
    }

    fn next_value(&mut self) -> Result<Value, ParseError> {
        match self.next()? {
            (Event::Scalar(scalar, style, ..), marker) => {
                use yaml_rust2::scanner::TScalarStyle::*;
                match style {
                    SingleQuoted | DoubleQuoted => Ok(Value::String(scalar)),
                    Plain => {
                        if parse_f64(&scalar).is_ok() {
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
                    _ => Err(ParseError {
                        location: Some((self.current_document_path(), marker).into()),
                        kind: ErrorKind::UnexpectedSyntax,
                        msg: format!("Unsupported value syntax, got \"{scalar}\" as {style:?}"),
                    }),
                }
            }
            (Event::SequenceStart(..), ..) => {
                let result = parse_until!(self, Event::SequenceEnd, next_value);
                self.next_sequence_end()?;
                Ok(Value::Array(result))
            }
            (Event::MappingStart(..), ..) => {
                let result = parse_until!(self, Event::MappingEnd, next_kv)
                    .into_iter()
                    .collect();
                self.next_mapping_end()?;
                Ok(Value::Mapping(result))
            }
            (ev, marker) => Err(ParseError {
                location: Some((self.current_document_path(), marker).into()),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected a value, got {ev:?}"),
            }),
        }
    }

    fn peek(&mut self) -> Result<&(Event, Marker), ParseError> {
        let result = self.yaml.peek()?;
        Ok(result)
    }

    fn peek_string(&mut self) -> Result<Option<(String, Marker)>, ParseError> {
        match self.peek().cloned()? {
            (Event::Scalar(value, ..), marker) => Ok(Some((value.to_owned(), marker))),
            (ev, marker) => Err(ParseError {
                location: Some((self.current_document_path(), marker).into()),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected to peek a scalar, got {ev:?}"),
            }),
        }
    }
}

fn next_value<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Value, ParseError> {
    input.next_value()
}

fn next_kv<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<KV, ParseError> {
    input.next_kv()
}

#[derive(Debug)]
pub enum ErrorKind {
    ScanError,
    UnexpectedSyntax,
}

#[derive(Debug)]
pub struct ParseError {
    location: Option<Location>,
    kind: ErrorKind,
    msg: String,
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

fn parse_in_parameters<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Value, ParseError> {
    input.enter_context("in parameters");
    let result = input.next_value()?;
    input.leave_context();
    Ok(result)
}

fn parse_out_parameters<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Value, ParseError> {
    input.enter_context("out parameters");
    let result = input.next_value()?;
    input.leave_context();
    Ok(result)
}

fn parse_task_call<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<FlowStep, ParseError> {
    let (name, marker) = input.next_string()?;
    input.enter_context(&format!("'{name}' task call"));
    let mut task_input = None;
    let mut task_output = None;
    while let Ok(Some((element, marker))) = input.peek_string() {
        input.next()?;
        match element.as_str() {
            "in" => task_input = Some(parse_in_parameters(input)?),
            "out" => task_output = Some(parse_out_parameters(input)?),
            element => {
                return Err(ParseError {
                    location: Some((input.current_document_path(), marker).into()),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unexpected task call element {element}"),
                })
            }
        }
    }
    input.leave_context();
    Ok(FlowStep::TaskCall {
        name,
        input: task_input,
        output: task_output,
        location: (input.current_document_path(), marker).into(),
    })
}

fn parse_log_call<T: Iterator<Item = char>>(
    input: &mut Input<T>,
    task_marker: Marker,
) -> Result<FlowStep, ParseError> {
    input.enter_context("log step");
    let (msg, msg_marker) = input.next_string()?;
    let task_input = Value::Mapping(vec![KV {
        location: (input.current_document_path(), msg_marker).into(),
        key: "msg".to_owned(),
        value: Value::String(msg),
    }]);
    input.leave_context();
    Ok(FlowStep::TaskCall {
        location: (input.current_document_path(), task_marker).into(),
        name: "log".to_owned(),
        input: Some(task_input),
        output: None,
    })
}

impl From<(DocumentPath, yaml_rust2::scanner::Marker)> for Location {
    fn from((path, marker): (DocumentPath, yaml_rust2::scanner::Marker)) -> Self {
        Location {
            path,
            index: marker.index(),
            line: marker.line(),
            col: marker.col(),
        }
    }
}

impl From<(DocumentPath, &yaml_rust2::scanner::Marker)> for Location {
    fn from((path, marker): (DocumentPath, &yaml_rust2::scanner::Marker)) -> Self {
        Location {
            path,
            index: marker.index(),
            line: marker.line(),
            col: marker.col(),
        }
    }
}

fn parse_form_field<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<FormField, ParseError> {
    input.next_mapping_start()?;

    let (name, marker) = input.next_string()?;
    input.enter_context(&format!("'{name}' field"));

    input.next_mapping_start()?;
    let options = parse_until!(input, Event::MappingEnd, next_kv);
    input.next_mapping_end()?;

    input.next_mapping_end()?;
    input.leave_context();
    Ok(FormField {
        location: (input.current_document_path(), marker).into(),
        name,
        options,
    })
}

fn parse_form<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Form, ParseError> {
    let (name, marker) = input.next_string()?;
    input.enter_context(&format!("'{name}' form"));

    input.next_sequence_start()?;
    let fields = parse_until!(input, Event::SequenceEnd, parse_form_field);
    input.next_sequence_end()?;

    input.leave_context();

    Ok(Form {
        location: (input.current_document_path(), marker).into(),
        name,
        fields,
    })
}

fn parse_forms<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Vec<Form>, ParseError> {
    input.enter_context("forms");

    input.next_string_constant("forms")?;
    input.next_mapping_start()?;
    let result = parse_until!(input, Event::MappingEnd, parse_form);
    input.next_mapping_end()?;

    input.leave_context();

    Ok(result)
}

fn parse_step<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<FlowStep, ParseError> {
    input.next_mapping_start()?;

    let step = match input.next()? {
        (Event::Scalar(key, ..), task_marker) if key == "log" => parse_log_call(input, task_marker)?,
        (Event::Scalar(key, ..), _) if key == "task" => parse_task_call(input)?,
        (ev, marker) => {
            return Err(ParseError {
                location: Some((input.current_document_path(), marker).into()),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected a flow step, got {ev:?}"),
            })
        }
    };

    input.next_mapping_end()?;

    Ok(step)
}

fn parse_flow<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Flow, ParseError> {
    let (name, marker) = input.next_string()?;
    input.enter_context(&format!("'{name}' flow"));

    input.next_sequence_start()?;
    let steps = parse_until!(input, Event::SequenceEnd, parse_step);
    input.next_sequence_end()?;

    input.leave_context();

    Ok(Flow {
        location: (input.current_document_path(), marker).into(),
        name,
        steps,
    })
}

fn parse_flows<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Vec<Flow>, ParseError> {
    input.enter_context("flows");

    input.next_string_constant("flows")?;
    input.next_mapping_start()?;
    let result = parse_until!(input, Event::MappingEnd, parse_flow);
    input.next_mapping_end()?;

    input.leave_context();

    Ok(result)
}

fn parse_configuration<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Configuration, ParseError> {
    input.enter_context("configuration");

    let marker = input.next_string_constant("configuration")?;
    input.next_mapping_start()?;
    let values = parse_until!(input, Event::MappingEnd, next_kv);
    input.next_mapping_end()?;

    input.leave_context();

    Ok(Configuration {
        location: (input.current_document_path(), marker).into(),
        values,
    })
}

fn parse_document<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<ConcordDocument, ParseError> {
    input.enter_context("document");

    input.next_document_start()?;
    input.next_mapping_start()?;

    // top-level elements
    let mut configuration = None;
    let mut flows = None;
    let mut forms = None;

    while let Ok(Some((top_level_element, marker))) = input.peek_string() {
        match top_level_element.as_str() {
            "configuration" => {
                configuration = Some(parse_configuration(input)?);
            }
            "flows" => {
                flows = Some(parse_flows(input)?);
            }
            "forms" => {
                forms = Some(parse_forms(input)?);
            }
            element => {
                return Err(ParseError {
                    location: Some((input.current_document_path(), marker).into()),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unexpected top-level element {element}"),
                })
            }
        }
    }

    input.next_mapping_end()?;
    input.next_document_end()?;

    input.leave_context();

    Ok(ConcordDocument {
        configuration,
        flows,
        forms,
    })
}

pub fn parse_stream<T: Iterator<Item = char>>(
    input: &mut Input<T>,
) -> Result<Vec<ConcordDocument>, ParseError> {
    input.next_stream_start()?;
    let result = parse_until!(input, Event::StreamEnd, parse_document);
    input.next_stream_end()?;
    Ok(result)
}
