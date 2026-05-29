use crate::types::{FormatParser, DataValue};
use serde_json::Value;

/// JSON Line-Delimited (ndjson) parser.
pub struct JsonParser;

impl JsonParser {
    pub fn new() -> Self {
        Self
    }
}

impl FormatParser for JsonParser {
    fn probe(&self, buffer: &[u8]) -> bool {
        // Skip whitespace and check if first character is '{'
        let mut i = 0;
        while i < buffer.len() && buffer[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < buffer.len() && buffer[i] == b'{' {
            return true;
        }
        false
    }

    fn find_boundaries(&self, mmap: &[u8], start: usize, end: usize) -> (usize, usize) {
        // ndjson uses standard line boundaries
        let s_idx = start;
        let e_idx = memchr::memchr(b'\n', &mmap[s_idx..end])
            .map(|pos| s_idx + pos)
            .unwrap_or(end);
        
        (s_idx, e_idx)
    }

    fn parse_unit(&self, data: &[u8]) -> anyhow::Result<Vec<DataValue>> {
        let v: Value = serde_json::from_slice(data)?;
        let mut row = Vec::new();
        
        if let Some(obj) = v.as_object() {
            for (_, value) in obj {
                match value {
                    Value::Number(n) => {
                        if let Some(f) = n.as_f64() {
                            row.push(DataValue::Float(f));
                        } else if let Some(i) = n.as_i64() {
                            row.push(DataValue::Int(i));
                        }
                    }
                    Value::String(s) => row.push(DataValue::String(s.clone())),
                    Value::Bool(b) => row.push(DataValue::String(b.to_string())),
                    Value::Null => row.push(DataValue::Null),
                    _ => row.push(DataValue::String(value.to_string())),
                }
            }
        }
        
        Ok(row)
    }
}
