use std::collections::HashMap;

#[derive(Debug)]
pub enum Value {
    String(String),
    Boolean(bool),
    Float(String), // keep float numbers as strings to avoid any conversion issues
    Integer(i64),
    Array(Vec<Value>),
    Mapping(HashMap<String, Value>),
}

#[derive(Debug)]
pub enum ConcordFlowStep {
    TaskCall {
        name: String,
        input: HashMap<String, Value>,
    },
}

#[derive(Debug)]
pub struct ConcordFlow {
    pub name: String,
    pub steps: Vec<ConcordFlowStep>,
}

#[derive(Debug)]
pub struct ConcordDocument {
    pub flows: Vec<ConcordFlow>,
}
