use std::fmt::{Debug, Formatter};

#[derive(Default, Clone)]
pub struct DocumentPath(Vec<String>);

impl DocumentPath {
    pub fn new(value: &[String]) -> Self {
        Self(Vec::from(value))
    }

    pub fn none() -> Self {
        Self(vec!["n/a".to_owned()])
    }
}

impl Debug for DocumentPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut i = 0;
        let len = self.0.len();
        loop {
            if i >= len {
                break;
            }
            if i + 1 < len {
                write!(f, "{}->", self.0[i])?;
            } else {
                write!(f, "{}", self.0[i])?;
            }

            i += 1;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Location {
    pub path: DocumentPath,
    pub index: usize,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug)]
pub enum Value {
    String(String),
    Boolean(bool),
    Float(String), // keep float numbers as strings to avoid any conversion issues
    Integer(i64),
    Array(Vec<Value>),
    Mapping(Vec<KV>),
}

#[derive(Debug)]
pub struct KV {
    pub location: Location,
    pub key: String,
    pub value: Value,
}

#[derive(Debug)]
pub enum LoopMode {
    Serial,
    Parallel,
}

#[derive(Debug)]
pub struct Loop {
    pub location: Location,
    pub items: Value,
    pub mode: Option<LoopMode>,
    pub parallelism: Option<Value>,
}

#[derive(Debug)]
pub struct Retry {
    pub location: Location,
    pub times: Option<Value>,
    pub delay: Option<Value>,
    pub input: Option<Value>,
}

#[derive(Debug)]
pub struct Configuration {
    pub location: Location,
    pub values: Vec<KV>,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum StepDefinition {
    TaskCall {
        task_name: String,
        input: Option<Value>,
        output: Option<Value>,
        error: Option<Vec<FlowStep>>,
        ignore_errors: Option<bool>,
        looping: Option<Loop>,
        meta: Option<Vec<KV>>,
        retry: Option<Retry>,
    },
    Expression {
        expr: String,
        output: Option<Value>,
        error: Option<Vec<FlowStep>>,
        meta: Option<Vec<KV>>,
    },
    Script {
        language_or_ref: String,
        body: Option<String>,
        input: Option<Value>,
        output: Option<Value>,
        error: Option<Vec<FlowStep>>,
        looping: Option<Loop>,
        meta: Option<Vec<KV>>,
        retry: Option<Retry>,
    },
    FlowCall {
        flow_name: String,
        input: Option<Value>,
        output: Option<Value>,
        error: Option<Vec<FlowStep>>,
        looping: Option<Loop>,
        meta: Option<Vec<KV>>,
        retry: Option<Retry>,
    },
}

#[derive(Debug)]
pub struct FlowStep {
    pub location: Location,
    pub step_name: Option<String>,
    pub step: StepDefinition,
}

#[derive(Debug)]
pub struct Flow {
    pub location: Location,
    pub name: String,
    pub steps: Vec<FlowStep>,
}

#[derive(Debug)]
pub struct FormField {
    pub location: Location,
    pub name: String,
    pub options: Vec<KV>,
}

#[derive(Debug)]
pub struct Form {
    pub location: Location,
    pub name: String,
    pub fields: Vec<FormField>,
}

#[derive(Debug)]
pub struct ConcordDocument {
    pub configuration: Option<Configuration>,
    pub flows: Option<Vec<Flow>>,
    pub forms: Option<Vec<Form>>,
}
