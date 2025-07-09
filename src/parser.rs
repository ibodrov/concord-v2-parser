use crate::error::{ErrorKind, ParseError};
use crate::input::{next_kv, Event, Input, Marker};
use crate::model::{
    ConcordDocument, Configuration, Flow, FlowStep, Form, FormField, Loop, LoopMode, Retry, StepDefinition,
    Value, KV,
};
use crate::parse_until;

fn parse_value<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Value, ParseError> {
    let (value, _) = input.next_value()?;
    Ok(value)
}

fn parse_bool<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<bool, ParseError> {
    match input.next_value()? {
        (Value::Boolean(result), ..) => Ok(result),
        (value, marker) => Err(ParseError {
            location: Some((input.current_document_path(), marker).into()),
            kind: ErrorKind::UnexpectedSyntax,
            msg: format!("Expected a bool value, got '{value:?}"),
        }),
    }
}

fn parse_string<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<String, ParseError> {
    let (value, _) = input.next_string()?;
    Ok(value)
}

fn parse_form_field<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<FormField, ParseError> {
    input.next_mapping_start()?;

    let (name, marker) = input.next_string()?;
    input.enter_context(format!("'{name}' field"));

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
    input.enter_context(format!("'{name}' form"));

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
    input.next_mapping_start()?;
    let result = parse_until!(input, Event::MappingEnd, parse_form);
    input.next_mapping_end()?;
    Ok(result)
}

fn parse_meta<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Vec<KV>, ParseError> {
    input.next_mapping_start()?;
    let result = parse_until!(input, Event::MappingEnd, next_kv);
    input.next_mapping_end()?;
    Ok(result)
}

fn parse_loop_mode<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<LoopMode, ParseError> {
    let (mode, marker) = input.next_string()?;
    match mode.as_str() {
        "parallel" => Ok(LoopMode::Parallel),
        "serial" => Ok(LoopMode::Serial),
        unknown => Err(ParseError {
            location: Some((input.current_document_path(), marker).into()),
            kind: ErrorKind::UnexpectedSyntax,
            msg: format!("Unexpected loop mode '{unknown}'. Only 'parallel' and 'serial' are supported."),
        }),
    }
}

fn parse_loop<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Loop, ParseError> {
    let (_, marker) = input.next_mapping_start()?;

    let location = (input.current_document_path(), marker).into();
    let mut items = None;
    let mut mode = None;
    let mut parallelism = None;

    while let Ok(Some((element, _))) = input.peek_string() {
        input.try_next()?;
        match element.as_str() {
            "items" => items = Some(input.with_context("loop items", parse_value)?),
            "mode" => mode = Some(input.with_context("loop mode", parse_loop_mode)?),
            "parallelism" => parallelism = Some(input.with_context("loop parallelism", parse_value)?),
            element => {
                return Err(ParseError {
                    location: Some(location),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unexpected loop element '{element}'"),
                })
            }
        }
    }
    input.next_mapping_end()?;

    let Some(items) = items else {
        return Err(ParseError {
            location: Some(location),
            kind: ErrorKind::UnexpectedSyntax,
            msg: "The 'items' field is required in the loop".to_owned(),
        });
    };

    Ok(Loop {
        location,
        items,
        mode,
        parallelism,
    })
}

fn parse_retry<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Retry, ParseError> {
    let (_, marker) = input.next_mapping_start()?;

    let location = (input.current_document_path(), marker).into();
    let mut times = None;
    let mut delay = None;
    let mut retry_input = None;

    while let Ok(Some((element, _))) = input.peek_string() {
        input.try_next()?;
        match element.as_str() {
            "times" => times = Some(input.with_context("retry 'times' option", parse_value)?),
            "delay" => delay = Some(input.with_context("retry delay", parse_value)?),
            "in" => retry_input = Some(input.with_context("retry input", parse_value)?),
            element => {
                return Err(ParseError {
                    location: Some(location),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unexpected loop element '{element}'"),
                })
            }
        }
    }
    input.next_mapping_end()?;

    Ok(Retry {
        location,
        times,
        delay,
        input: retry_input,
    })
}

fn parse_task_call<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<StepDefinition, ParseError> {
    let (task_name, marker) = input.next_string()?;
    input.enter_context(format!("'{task_name}' task call"));

    let location = (input.current_document_path(), marker).into();
    let mut task_input = None;
    let mut task_output = None;
    let mut error = None;
    let mut ignore_errors = None;
    let mut looping = None;
    let mut meta = None;
    let mut retry = None;

    while let Ok(Some((element, _))) = input.peek_string() {
        input.try_next()?;
        match element.as_str() {
            "in" => task_input = Some(input.with_context("'in' parameters", parse_value)?),
            "out" => task_output = Some(input.with_context("'out' parameters", parse_value)?),
            "error" => error = Some(input.with_context("'error' block", parse_flow_steps)?),
            "ignoreErrors" => ignore_errors = Some(input.with_context("'ignoreErrors' option", parse_bool)?),
            "loop" => looping = Some(input.with_context("'loop' option", parse_loop)?),
            "meta" => meta = Some(input.with_context("'meta' block", parse_meta)?),
            "retry" => retry = Some(input.with_context("'retry' option", parse_retry)?),
            element => {
                return Err(ParseError {
                    location: Some(location),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unexpected task call element '{element}'"),
                })
            }
        }
    }

    input.leave_context();

    let error = error.map(|(steps, _)| steps);

    Ok(StepDefinition::TaskCall {
        task_name,
        input: task_input,
        output: task_output,
        error,
        ignore_errors,
        looping,
        meta,
        retry,
    })
}

fn parse_single_argument_task<T: Iterator<Item = char>>(
    input: &mut Input<T>,
    task_name: &str,
    parameter_name: &str,
) -> Result<StepDefinition, ParseError> {
    input.enter_context(format!("'{task_name}' step"));
    let (value, value_marker) = input.next_value()?;
    let task_input = Value::Mapping(vec![KV {
        location: (input.current_document_path(), value_marker).into(),
        key: parameter_name.to_owned(),
        value,
    }]);
    input.leave_context();
    Ok(StepDefinition::TaskCall {
        task_name: task_name.to_owned(),
        input: Some(task_input),
        output: None,
        error: None,
        ignore_errors: None,
        looping: None,
        meta: None,
        retry: None,
    })
}

fn parse_expr<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<StepDefinition, ParseError> {
    let (expr, marker) = input.next_string()?;
    input.enter_context(format!("expression '{expr}'"));

    let location = (input.current_document_path(), marker).into();
    let mut expr_output = None;
    let mut error = None;
    let mut meta = None;

    while let Ok(Some((element, _))) = input.peek_string() {
        input.try_next()?;
        match element.as_str() {
            "out" => expr_output = Some(input.with_context("'out' parameters", parse_value)?),
            "error" => error = Some(input.with_context("'error' block", parse_flow_steps)?),
            "meta" => meta = Some(input.with_context("'meta' block", parse_meta)?),
            element => {
                return Err(ParseError {
                    location: Some(location),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unexpected expr step element '{element}'"),
                })
            }
        }
    }

    input.leave_context();

    let error = error.map(|(steps, _)| steps);

    Ok(StepDefinition::Expression {
        expr,
        output: expr_output,
        error,
        meta,
    })
}

fn parse_script<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<StepDefinition, ParseError> {
    let (language_or_ref, marker) = input.next_string()?;
    input.enter_context(format!("script '{language_or_ref}"));

    let location = (input.current_document_path(), marker).into();
    let mut body = None;
    let mut script_input = None;
    let mut script_output = None;
    let mut error = None;
    let mut looping = None;
    let mut meta = None;
    let mut retry = None;

    while let Ok(Some((element, _))) = input.peek_string() {
        input.try_next()?;
        match element.as_str() {
            "body" => body = Some(input.with_context("script body", parse_string)?),
            "in" => script_input = Some(input.with_context("'in' parameters", parse_value)?),
            "out" => script_output = Some(input.with_context("'out' parameters", parse_value)?),
            "error" => error = Some(input.with_context("'error' block", parse_flow_steps)?),
            "loop" => looping = Some(input.with_context("'loop' option", parse_loop)?),
            "meta" => meta = Some(input.with_context("'meta' block", parse_meta)?),
            "retry" => retry = Some(input.with_context("'retry' option", parse_retry)?),
            element => {
                return Err(ParseError {
                    location: Some(location),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unexpected script step element '{element}'"),
                })
            }
        }
    }

    input.leave_context();

    let error = error.map(|(steps, _)| steps);

    Ok(StepDefinition::Script {
        language_or_ref,
        body,
        input: script_input,
        output: script_output,
        error,
        looping,
        meta,
        retry,
    })
}

fn parse_flow_call<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<StepDefinition, ParseError> {
    let (flow_name, marker) = input.next_string()?;
    input.enter_context(format!("call '{flow_name}"));

    let location = (input.current_document_path(), marker).into();
    let mut call_input = None;
    let mut call_output = None;
    let mut error = None;
    let mut looping = None;
    let mut meta = None;
    let mut retry = None;

    while let Ok(Some((element, _))) = input.peek_string() {
        input.try_next()?;
        match element.as_str() {
            "in" => call_input = Some(input.with_context("'in' parameters", parse_value)?),
            "out" => call_output = Some(input.with_context("'out' parameters", parse_value)?),
            "error" => error = Some(input.with_context("'error' block", parse_flow_steps)?),
            "loop" => looping = Some(input.with_context("'loop' option", parse_loop)?),
            "meta" => meta = Some(input.with_context("'meta' block", parse_meta)?),
            "retry" => retry = Some(input.with_context("'retry' option", parse_retry)?),
            element => {
                return Err(ParseError {
                    location: Some(location),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unexpected flow call element '{element}'"),
                })
            }
        }
    }

    input.leave_context();

    let error = error.map(|(steps, _)| steps);

    Ok(StepDefinition::FlowCall {
        flow_name,
        input: call_input,
        output: call_output,
        error,
        looping,
        meta,
        retry,
    })
}

fn parse_checkpoint<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<StepDefinition, ParseError> {
    let (name, marker) = input.next_string()?;
    input.enter_context(format!("checkpoint '{name}"));

    let location = (input.current_document_path(), marker).into();
    let mut meta = None;

    while let Ok(Some((element, _))) = input.peek_string() {
        input.try_next()?;
        match element.as_str() {
            "meta" => meta = Some(input.with_context("'meta' block", parse_meta)?),
            element => {
                return Err(ParseError {
                    location: Some(location),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unexpected checkpoint element '{element}'"),
                })
            }
        }
    }

    input.leave_context();

    Ok(StepDefinition::Checkpoint { name, meta })
}

fn parse_if<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<StepDefinition, ParseError> {
    let (expression, marker) = input.next_string()?;
    input.enter_context(format!("if '{expression}"));

    let location = (input.current_document_path(), marker).into();
    let mut then_steps = None;
    let mut else_steps = None;
    let mut meta = None;

    while let Ok(Some((element, _))) = input.peek_string() {
        input.try_next()?;
        match element.as_str() {
            "then" => then_steps = Some(input.with_context("'then' block", parse_flow_steps)?),
            "else" => else_steps = Some(input.with_context("'else' block", parse_flow_steps)?),
            "meta" => meta = Some(input.with_context("'meta' block", parse_meta)?),
            element => {
                return Err(ParseError {
                    location: Some(location),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unexpected if block element '{element}'"),
                })
            }
        }
    }

    input.leave_context();

    let Some((then_steps, _)) = then_steps else {
        return Err(ParseError {
            location: Some(location),
            kind: ErrorKind::UnexpectedSyntax,
            msg: "The 'then' steps are required in 'if' block".to_owned(),
        });
    };

    let else_steps = else_steps.map(|(steps, _)| steps);

    Ok(StepDefinition::If {
        expression,
        then_steps,
        else_steps,
        meta,
    })
}

fn parse_set_variables<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<StepDefinition, ParseError> {
    input.enter_context("set");

    let (_, marker) = input.next_mapping_start()?;
    let vars = parse_until!(input, Event::MappingEnd, next_kv);
    input.next_mapping_end()?;

    let location = (input.current_document_path(), marker).into();
    let mut meta = None;

    while let Ok(Some((element, _))) = input.peek_string() {
        input.try_next()?;
        match element.as_str() {
            "meta" => meta = Some(input.with_context("'meta' block", parse_meta)?),
            element => {
                return Err(ParseError {
                    location: Some(location),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unexpected checkpoint element '{element}'"),
                })
            }
        }
    }

    input.leave_context();

    Ok(StepDefinition::SetVariables { vars, meta })
}

fn parse_parallel_block<T: Iterator<Item = char>>(
    input: &mut Input<T>,
) -> Result<StepDefinition, ParseError> {
    input.enter_context("'parallel' block".to_string());

    let (steps, marker) = parse_flow_steps(input)?;

    let location = (input.current_document_path(), marker).into();
    let mut block_output = None;
    let mut meta = None;

    while let Ok(Some((element, _))) = input.peek_string() {
        input.try_next()?;
        match element.as_str() {
            "out" => block_output = Some(input.with_context("'out' parameters", parse_value)?),
            "meta" => meta = Some(input.with_context("'meta' block", parse_meta)?),
            element => {
                return Err(ParseError {
                    location: Some(location),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unexpected parallel block element '{element}'"),
                })
            }
        }
    }

    input.leave_context();

    Ok(StepDefinition::ParallelBlock {
        steps,
        output: block_output,
        meta,
    })
}

fn parse_block<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<StepDefinition, ParseError> {
    input.enter_context("'parallel' block".to_string());

    let (steps, marker) = parse_flow_steps(input)?;

    let location = (input.current_document_path(), marker).into();
    let mut block_output = None;
    let mut error = None;
    let mut looping = None;
    let mut meta = None;

    while let Ok(Some((element, _))) = input.peek_string() {
        input.try_next()?;
        match element.as_str() {
            "out" => block_output = Some(input.with_context("'out' parameters", parse_value)?),
            "error" => error = Some(input.with_context("'error' block", parse_flow_steps)?),
            "loop" => looping = Some(input.with_context("'loop' option", parse_loop)?),
            "meta" => meta = Some(input.with_context("'meta' block", parse_meta)?),
            element => {
                return Err(ParseError {
                    location: Some(location),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unexpected parallel block element '{element}'"),
                })
            }
        }
    }

    input.leave_context();

    let error = error.map(|(steps, _)| steps);

    Ok(StepDefinition::Block {
        steps,
        output: block_output,
        error,
        looping,
        meta,
    })
}

fn parse_flow_step<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<FlowStep, ParseError> {
    let (_, step_marker) = input.next_mapping_start()?;

    let location = (input.current_document_path(), step_marker).into();
    let mut step_name = None;
    let mut step = None;

    while let Ok(Some((name_or_step, ..))) = input.peek_string() {
        input.try_next()?;
        match name_or_step.as_str() {
            "name" => step_name = Some(input.next_string()?.0),
            "log" => step = Some(parse_single_argument_task(input, "log", "msg")?),
            "throw" => step = Some(parse_single_argument_task(input, "throw", "exception")?),
            "task" => step = Some(parse_task_call(input)?),
            "expr" => step = Some(parse_expr(input)?),
            "script" => step = Some(parse_script(input)?),
            "call" => step = Some(parse_flow_call(input)?),
            "checkpoint" => step = Some(parse_checkpoint(input)?),
            "if" => step = Some(parse_if(input)?),
            "set" => step = Some(parse_set_variables(input)?),
            "parallel" => step = Some(parse_parallel_block(input)?),
            "try" | "block" => step = Some(parse_block(input)?),
            unknown => {
                return Err(ParseError {
                    location: Some(location),
                    kind: ErrorKind::UnexpectedSyntax,
                    msg: format!("Unknown step '{unknown}'"),
                })
            }
        }
    }

    input.next_mapping_end()?;

    let Some(step) = step else {
        return Err(ParseError {
            location: Some(location),
            kind: ErrorKind::UnexpectedSyntax,
            msg: "Expected a step".to_owned(),
        });
    };

    Ok(FlowStep {
        location,
        step_name,
        step,
    })
}

fn parse_flow_steps<T: Iterator<Item = char>>(
    input: &mut Input<T>,
) -> Result<(Vec<FlowStep>, Marker), ParseError> {
    let (_, marker) = input.next_sequence_start()?;
    let steps = parse_until!(input, Event::SequenceEnd, parse_flow_step);
    input.next_sequence_end()?;
    Ok((steps, marker))
}

fn parse_flow<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Flow, ParseError> {
    let (name, marker) = input.next_string()?;
    input.enter_context(format!("'{name}' flow"));
    let (steps, _) = parse_flow_steps(input)?;
    input.leave_context();
    Ok(Flow {
        location: (input.current_document_path(), marker).into(),
        name,
        steps,
    })
}

fn parse_flows<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Vec<Flow>, ParseError> {
    input.next_mapping_start()?;
    let result = parse_until!(input, Event::MappingEnd, parse_flow);
    input.next_mapping_end()?;
    Ok(result)
}

fn parse_configuration<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<Configuration, ParseError> {
    let (.., marker) = input.next_mapping_start()?;
    let values = parse_until!(input, Event::MappingEnd, next_kv);
    input.next_mapping_end()?;

    Ok(Configuration {
        location: (input.current_document_path(), marker).into(),
        values,
    })
}

fn parse_document<T: Iterator<Item = char>>(input: &mut Input<T>) -> Result<ConcordDocument, ParseError> {
    input.next_document_start()?;
    input.next_mapping_start()?;

    let mut configuration = None;
    let mut flows = None;
    let mut forms = None;

    while let Ok(Some((top_level_element, marker))) = input.peek_string() {
        input.try_next()?;
        match top_level_element.as_str() {
            "configuration" => {
                configuration = Some(input.with_context("configuration", parse_configuration)?);
            }
            "flows" => {
                flows = Some(input.with_context("flows", parse_flows)?);
            }
            "forms" => {
                forms = Some(input.with_context("forms", parse_forms)?);
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
    let result = input.with_context("document", |input| {
        Ok(parse_until!(input, Event::StreamEnd, parse_document))
    })?;
    input.next_stream_end()?;
    Ok(result)
}
