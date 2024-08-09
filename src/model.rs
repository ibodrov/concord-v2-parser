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

pub type KV = (String, Value);

#[derive(Debug)]
pub struct Marker {
    pub index: usize,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug)]
pub enum ConcordFlowStep {
    TaskCall {
        marker: Marker,
        name: String,
        input: HashMap<String, Value>,
    },
}

#[derive(Debug)]
pub struct ConcordFlow {
    pub name: String,
    pub steps: Vec<ConcordFlowStep>,
    pub marker: Marker,
}

#[derive(Debug)]
pub struct ConcordDocument {
    pub configuration: Vec<KV>,
    pub flows: Vec<ConcordFlow>,
}
