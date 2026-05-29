use crate::types::{FormatParser, DataValue};

/// Foundational parser for raw text logs (Syslog, W3C, unstructured traces)
pub struct RawLogsParser;

impl RawLogsParser {
    pub fn new() -> Self {
        Self
    }
}

impl FormatParser for RawLogsParser {
    fn probe(&self, _buffer: &[u8]) -> bool {
        // Fallback parser: if everything else fails, we treat it as raw text logs
        true
    }

    fn find_boundaries(&self, mmap: &[u8], start: usize, end: usize) -> (usize, usize) {
        // Line-based boundary detection (same as CSV)
        let s_idx = start;
        let e_idx = memchr::memchr(b'\n', &mmap[s_idx..end])
            .map(|pos| s_idx + pos)
            .unwrap_or(end);
        
        (s_idx, e_idx)
    }

    fn parse_unit(&self, data: &[u8]) -> anyhow::Result<Vec<DataValue>> {
        // In raw logs, the entire line is a single column of DataValue::String
        let s = std::str::from_utf8(data).unwrap_or("<invalid utf8>").trim();
        Ok(vec![DataValue::String(s.to_string())])
    }
}
