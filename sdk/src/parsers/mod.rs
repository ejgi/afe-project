pub mod big_data;
pub mod forensics;

use crate::types::FormatParser;

/// The Central Dispatcher that routes files to their appropriate domain parsers
pub struct Dispatcher {
    parsers: Vec<Box<dyn FormatParser>>,
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            parsers: vec![
                // Domain: Big Data
                Box::new(big_data::json::JsonParser::new()),
                Box::new(big_data::csv::CsvParser::new(b',')),
                Box::new(big_data::csv::CsvParser::new(b'\t')),
                Box::new(big_data::csv::CsvParser::new(b';')),
                Box::new(big_data::csv::CsvParser::new(b'|')),
                
                // Domain: Forensics
                Box::new(forensics::evtx::EvtxParser::new()),
                Box::new(forensics::pcap::PcapParser::new()),
                Box::new(forensics::raw_logs::RawLogsParser::new()),
            ],
        }
    }

    /// Detect if any of the registered parsers can handle this file
    pub fn probe(&self, buffer: &[u8]) -> Option<&dyn FormatParser> {
        for parser in &self.parsers {
            if parser.probe(buffer) {
                return Some(parser.as_ref());
            }
        }
        None
    }

    /// Manually get a parser by name
    pub fn get_parser(&self, name: &str) -> Option<&dyn FormatParser> {
        match name.to_lowercase().as_str() {
            "csv" => Some(self.parsers[1].as_ref()), // Default CSV
            "json" | "ndjson" => Some(self.parsers[0].as_ref()),
            "evtx" => Some(self.parsers[5].as_ref()),
            "pcap" | "pcapng" => Some(self.parsers[6].as_ref()),
            "logs" | "raw" => Some(self.parsers[7].as_ref()),
            _ => None,
        }
    }
}
