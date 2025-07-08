use crate::error::{ErrorKind, ParseError};
use crate::input::{next_kv, Event, Input, Marker};
use crate::model::{ConcordDocument, Configuration, Flow, FlowStep, Form, FormField, Value, KV};
use crate::parse_until;

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
        input.try_next()?;
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

    let step = match input.try_next()? {
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
