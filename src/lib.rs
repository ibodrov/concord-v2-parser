use std::{collections::HashMap, str::Chars};

pub type Event = yaml_rust::Event;
pub type Marker = yaml_rust::scanner::Marker;

pub type Input<'a> = yaml_rust::parser::Parser<Chars<'a>>;

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

fn next_event(input: &mut Input) -> Result<(Event, Marker), ParseError> {
    let (event, marker) = input.next()?;
    println!("! {event:?} @ {marker:?}");
    Ok((event, marker))
}

fn peek_string(input: &mut Input) -> Result<Option<(String, Marker)>, ParseError> {
    match input.peek()? {
        (Event::Scalar(value, ..), marker) => Ok(Some((value.to_owned(), *marker))),
        (ev, marker) => Err(ParseError {
            marker: Some(*marker),
            kind: ErrorKind::UnexpectedSyntax,
            msg: format!("Expected to peek a scalar, got {ev:?}"),
        }),
    }
}

macro_rules! consume_event {
    ($input:ident, $pat:pat) => {
        match next_event($input)? {
            (ev @ $pat, marker) => Ok((ev, marker)),
            (ev, marker) => Err(ParseError {
                marker: Some(marker.clone()),
                kind: ErrorKind::UnexpectedSyntax,
                msg: format!("Expected {}, got {ev:?}", stringify!($pat)),
            }),
        }
    };
}

macro_rules! peek_event {
    ($input:ident, $pat:pat) => {
        matches!($input.peek()?, ($pat, _))
    };
}

macro_rules! parse_until {
    ($input:ident, $pat:pat, $parser:ident) => {{
        let mut items = Vec::new();
        loop {
            let item = $parser($input)?;
            items.push(item);
            if peek_event!($input, $pat) {
                break;
            }
        }
        items
    }};
}

fn consume_string(input: &mut Input) -> Result<(String, Marker), ParseError> {
    match consume_event!(input, Event::Scalar(..))? {
        (Event::Scalar(value, ..), marker) => Ok((value.to_owned(), marker)),
        (ev, marker) => Err(ParseError {
            marker: Some(marker),
            kind: ErrorKind::UnexpectedSyntax,
            msg: format!("Expected to peek a scalar, got {ev:?}"),
        }),
    }
}

fn consume_string_constant(input: &mut Input, value: &str) -> Result<(), ParseError> {
    match next_event(input)? {
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
        input: HashMap<String, String>,
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
fn parse_kv(input: &mut Input) -> Result<(String, String), ParseError> {
    let (key, _) = consume_string(input)?;
    let (value, _) = consume_string(input)?;
    Ok((key, value))
}

fn parse_step(input: &mut Input) -> Result<ConcordFlowStep, ParseError> {
    consume_event!(input, Event::MappingStart(..))?;

    let step = match next_event(input)? {
        (Event::Scalar(key, ..), _) if key == "log" => {
            let (msg, _) = consume_string(input)?;
            ConcordFlowStep::TaskCall {
                name: "log".to_owned(),
                input: HashMap::from([("msg".to_owned(), msg)]),
            }
        }
        (Event::Scalar(key, ..), _) if key == "task" => {
            let (name, _) = consume_string(input)?;
            let mut input_parameters = HashMap::new();
            if peek_string_constant(input, "in")? {
                consume_event!(input, Event::Scalar(..))?;
                consume_event!(input, Event::MappingStart(..))?;
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
    let (name, _) = consume_string(input)?;
    consume_event!(input, Event::SequenceStart(..))?;

    let mut steps = Vec::new();
    loop {
        let step = parse_step(input)?;
        steps.push(step);
        if peek_event!(input, Event::SequenceEnd) {
            break;
        }
    }
    consume_event!(input, Event::SequenceEnd)?;

    Ok(ConcordFlow { name, steps })
}

fn parse_flows(input: &mut Input) -> Result<Vec<ConcordFlow>, ParseError> {
    consume_string_constant(input, "flows")?;
    consume_event!(input, Event::MappingStart(..))?;
    let result = parse_until!(input, Event::MappingEnd, parse_flow);
    consume_event!(input, Event::MappingEnd)?;
    Ok(result)
}

fn parse_document(input: &mut Input) -> Result<ConcordDocument, ParseError> {
    consume_event!(input, Event::DocumentStart)?;
    consume_event!(input, Event::MappingStart(_))?;

    // top-level elements
    let mut flows = Vec::new();
    if let Some((top_level_element, marker)) = peek_string(input)? {
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

    #[test]
    fn hello_world() {
        let src = r#"
        flows:
          default:
            - log: "Hello!"
        "#;

        let mut input = yaml_rust::parser::Parser::new(src.chars());
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

        let mut input = yaml_rust::parser::Parser::new(src.chars());
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

        let mut input = yaml_rust::parser::Parser::new(src.chars());
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

        let mut input = yaml_rust::parser::Parser::new(src.chars());
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

        let mut input = yaml_rust::parser::Parser::new(src.chars());
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
                a: 1
                b: "Hello!"
                c: false
        "#;

        let mut input = yaml_rust::parser::Parser::new(src.chars());
        let result = parse_stream(&mut input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].flows.len(), 1);
        assert_eq!(result[0].flows[0].steps.len(), 1);
        assert!(match &result[0].flows[0].steps[0] {
            ConcordFlowStep::TaskCall { name, input } => {
                assert_eq!(name, "foo");
                assert_eq!(input.len(), 3);
                true
            }
        });
    }
}
