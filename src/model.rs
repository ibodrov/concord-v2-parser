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
pub struct Location {
    pub index: usize,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug)]
pub enum FlowStep {
    TaskCall {
        location: Location,
        name: String,
        input: Option<Value>,
        output: Option<Value>,
    },
}

#[derive(Debug)]
pub struct Flow {
    pub location: Location,
    pub name: String,
    pub steps: Vec<FlowStep>,
}

#[derive(Debug)]
pub struct Configuration {
    pub location: Location,
    pub values: Vec<KV>,
}

#[derive(Debug)]
pub struct ConcordDocument {
    pub configuration: Option<Configuration>,
    pub flows: Option<Vec<Flow>>,
}
