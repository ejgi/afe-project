use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataValue {
    Float(f64),
    String(String),
    Int(i64),
    Null,
}

pub trait FormatParser: Send + Sync {
    fn probe(&self, buffer: &[u8]) -> bool;
    fn find_boundaries(&self, mmap: &[u8], start: usize, end: usize) -> (usize, usize);
    fn parse_unit(&self, data: &[u8]) -> anyhow::Result<Vec<DataValue>>;
}
