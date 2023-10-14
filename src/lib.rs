use std::collections::HashMap;

pub type Event = yaml_rust::Event;
pub type Marker = yaml_rust::scanner::Marker;

// pub type Input<'a> = yaml_rust::parser::Parser<Chars<'a>>;

pub struct Input {
    items: Vec<(Event, Marker)>,
    idx: usize,
}

impl TryFrom<&str> for Input {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut parser = yaml_rust::parser::Parser::new(value.chars());
        let mut items = Vec::new();
        loop {
            let (ev, marker) = parser.next()?;
            let done = ev == Event::StreamEnd;
            items.push((ev, marker));
            if done {
                break;
            }
        }
        Ok(Input { items, idx: 0 })
    }
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
        println!("next: [{}] {event:?} @ {marker:?}", self.idx);
        self.idx += 1;
        Ok((event.clone(), *marker))
    }

    fn next_mapping_start(&mut self) -> Result<(Event, Marker), ParseError> {
        match self.next()? {
            item @ (Event::MappingStart(..), _) => Ok(item),
            (ev, marker) => Err(ParseError {
                marker: Some(marker),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected an object, got {ev:?}"),
            }),
        }
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
}

#[derive(Debug)]
pub enum ErrorKind {
    ScanError,
    UnexpectedSyntax,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ParseError {
    marker: Option<Marker>,
    kind: ErrorKind,
    msg: String,
}

impl From<yaml_rust::ScanError> for ParseError {
    fn from(value: yaml_rust::ScanError) -> Self {
        Self {
            marker: Some(*value.marker()),
            kind: ErrorKind::ScanError,
            msg: value.to_string(),
        }
    }
}

macro_rules! consume_event {
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

#[derive(Debug)]
pub enum Value {
    String(String),
    Boolean(bool),
    Number(f64),
    Array(Vec<Value>),
    Mapping(HashMap<String, Value>),
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

fn consume_value(input: &mut Input) -> Result<(Value, Marker), ParseError> {
    match input.next()? {
        (Event::Scalar(scalar, style, ..), marker) => {
            use yaml_rust::scanner::TScalarStyle::*;
            match style {
                SingleQuoted | DoubleQuoted => Ok((Value::String(scalar), marker)),
                Plain => {
                    if let Ok(value) = parse_f64(&scalar) {
                        Ok((Value::Number(value), marker))
                    } else if let Ok(value) = scalar.parse::<bool>() {
                        Ok((Value::Boolean(value), marker))
                    } else {
                        Err(ParseError {
                            marker: Some(marker),
                            kind: ErrorKind::UnexpectedSyntax,
                            msg: format!("Unsupported plain value syntax, got \"{scalar}\""),
                        })
                    }
                }
                _ => Err(ParseError {
                    marker: Some(marker),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unsupported value syntax, got \"{scalar}\" as {style:?}"),
                }),
            }
        }
        (ev, marker) => Err(ParseError {
            marker: Some(marker),
            kind: ErrorKind::UnexpectedSyntax,
            msg: format!("Expected a value, got {ev:?}"),
        }),
    }
}

fn consume_string_constant(input: &mut Input, value: &str) -> Result<(), ParseError> {
    match input.next()? {
        (Event::Scalar(scalar, ..), _) if scalar == value => Ok(()),
        (ev, marker) => Err(ParseError {
            marker: Some(marker),
            kind: ErrorKind::UnexpectedSyntax,
            msg: format!("Expected a string {value}, got {ev:?}"),
        }),
    }
}

fn peek_string_constant(input: &mut Input, value: &str) -> Result<bool, ParseError> {
    match input.peek()? {
        (Event::Scalar(scalar, ..), _) => Ok(scalar == value),
        _ => Ok(false),
    }
}

#[derive(Debug)]
pub enum ConcordFlowStep {
    TaskCall {
        name: String,
        input: HashMap<String, Value>,
    },
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ConcordFlow {
    name: String,
    steps: Vec<ConcordFlowStep>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ConcordDocument {
    flows: Vec<ConcordFlow>,
}

// TODO convert string->actual type
fn parse_kv(input: &mut Input) -> Result<(String, Value), ParseError> {
    let (key, _) = input.next_string()?;
    let (value, _) = consume_value(input)?;
    Ok((key, value))
}

fn parse_step(input: &mut Input) -> Result<ConcordFlowStep, ParseError> {
    input.next_mapping_start()?;

    let step = match input.next()? {
        (Event::Scalar(key, ..), _) if key == "log" => {
            let (msg, _) = input.next_string()?;
            ConcordFlowStep::TaskCall {
                name: "log".to_owned(),
                input: HashMap::from([("msg".to_owned(), Value::String(msg))]),
            }
        }
        (Event::Scalar(key, ..), _) if key == "task" => {
            let (name, _) = input.next_string()?;
            let mut input_parameters = HashMap::new();
            if peek_string_constant(input, "in")? {
                consume_event!(input, Event::Scalar(..))?;
                input.next_mapping_start()?;
                let kvs = parse_until!(input, Event::MappingEnd, parse_kv);
                input_parameters.extend(kvs);
                consume_event!(input, Event::MappingEnd)?;
            };
            ConcordFlowStep::TaskCall {
                name,
                input: input_parameters,
            }
        }
        (ev, marker) => {
            return Err(ParseError {
                marker: Some(marker),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected a flow step, got {ev:?}"),
            })
        }
    };

    consume_event!(input, Event::MappingEnd)?;

    Ok(step)
}

fn parse_flow(input: &mut Input) -> Result<ConcordFlow, ParseError> {
    let (name, _) = input.next_string()?;
    consume_event!(input, Event::SequenceStart(..))?;

    let mut steps = Vec::new();
    loop {
        let step = parse_step(input)?;
        steps.push(step);
        if matches!(input.peek()?, (Event::SequenceEnd, _)) {
            break;
        }
    }
    consume_event!(input, Event::SequenceEnd)?;

    Ok(ConcordFlow { name, steps })
}

fn parse_flows(input: &mut Input) -> Result<Vec<ConcordFlow>, ParseError> {
    consume_string_constant(input, "flows")?;
    input.next_mapping_start()?;
    let result = parse_until!(input, Event::MappingEnd, parse_flow);
    consume_event!(input, Event::MappingEnd)?;
    Ok(result)
}

fn parse_document(input: &mut Input) -> Result<ConcordDocument, ParseError> {
    consume_event!(input, Event::DocumentStart)?;
    input.next_mapping_start()?;

    // top-level elements
    let mut flows = Vec::new();
    if let Some((top_level_element, marker)) = input.peek_string()? {
        match top_level_element.as_str() {
            "flows" => {
                for flow in parse_flows(input)? {
                    flows.push(flow);
                }
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

    consume_event!(input, Event::MappingEnd)?;
    consume_event!(input, Event::DocumentEnd)?;

    Ok(ConcordDocument { flows })
}

pub fn parse_stream(input: &mut Input) -> Result<Vec<ConcordDocument>, ParseError> {
    consume_event!(input, Event::StreamStart)?;
    let result = parse_until!(input, Event::StreamEnd, parse_document);
    consume_event!(input, Event::StreamEnd)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches_f64(value: Option<&Value>, expected: f64) -> bool {
        matches!(value, Some(Value::Number(value)) if value.to_bits() == expected.to_bits())
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
        assert!(result.is_err());
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
        "#;

        let mut input = Input::try_from(src).unwrap();
        let result = parse_stream(&mut input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].flows.len(), 1);
        assert_eq!(result[0].flows[0].steps.len(), 1);
        assert!(match &result[0].flows[0].steps[0] {
            ConcordFlowStep::TaskCall { name, input } => {
                assert_eq!(name, "foo");
                assert_eq!(input.len(), 3);
                assert!(matches_f64(input.get("a"), 1.23456789));
                assert!(matches!(input.get("b"), Some(Value::String(value)) if value == "Hello!"));
                assert!(matches!(input.get("c"), Some(Value::Boolean(false))));
                true
            }
        });
    }
}
