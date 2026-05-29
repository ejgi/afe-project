use crate::types::{FormatParser, DataValue};

/// Highly optimized CSV parser fallback implementing FormatParser.
/// Note: The absolute highest performance path (Zero-RAM streaming) 
/// still runs natively in `analytics.rs`, this is the generic abstraction
/// for standard domain routing.
pub struct CsvParser {
    delimiter: u8,
}

impl CsvParser {
    pub fn new(delimiter: u8) -> Self {
        Self { delimiter }
    }
}

impl FormatParser for CsvParser {
    fn probe(&self, buffer: &[u8]) -> bool {
        // If we see our delimiter in the first line before a newline, it's likely a CSV
        if let Some(pos) = memchr::memchr(b'\n', buffer) {
            memchr::memchr(self.delimiter, &buffer[..pos]).is_some()
        } else {
            // No newline, check if delimiter exists at all
            memchr::memchr(self.delimiter, buffer).is_some()
        }
    }

    fn find_boundaries(&self, mmap: &[u8], start: usize, end: usize) -> (usize, usize) {
        // Start is inclusive, find the next newline
        let s_idx = start;
        let e_idx = memchr::memchr(b'\n', &mmap[s_idx..end])
            .map(|pos| s_idx + pos)
            .unwrap_or(end);
        
        (s_idx, e_idx)
    }

    fn parse_unit(&self, data: &[u8]) -> anyhow::Result<Vec<DataValue>> {
        let mut row = Vec::new();
        for field in data.split(|&b| b == self.delimiter) {
            let s = std::str::from_utf8(field).unwrap_or("").trim();
            if s.is_empty() {
                row.push(DataValue::Null);
            } else if let Ok(f) = s.parse::<f64>() {
                row.push(DataValue::Float(f));
            } else if let Ok(i) = s.parse::<i64>() {
                row.push(DataValue::Int(i));
            } else {
                row.push(DataValue::String(s.to_string()));
            }
        }
        Ok(row)
    }
}
