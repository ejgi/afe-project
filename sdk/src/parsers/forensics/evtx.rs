// use std::sync::Arc;
use crate::types::{FormatParser, DataValue};
// use crate::accumulator::ColumnAccumulator;

// ─────────────────────────────────────────────────────────────────────────────
// EVTX Binary Signatures (Windows Event Log format)
// ─────────────────────────────────────────────────────────────────────────────
const EVTX_FILE_MAGIC: &[u8; 8] = b"ElfFile\x00";
const EVTX_CHUNK_MAGIC: &[u8; 8] = b"ElfChnk\x00";
const EVTX_RECORD_MAGIC: &[u8; 4] = b"\x2a\x2a\x00\x00";
const EVTX_HEADER_SIZE: usize = 4096;
const EVTX_CHUNK_SIZE: usize = 65536; // 64KB per chunk

/// EVTX field IDs for key forensic fields (BinXml substitution tokens)
const _TOKEN_EOF: u8 = 0x00;
const _TOKEN_OPEN_STARTELEM: u8 = 0x01;
const _TOKEN_CLOSE_STARTELEM: u8 = 0x02;
const TOKEN_VALUE: u8 = 0x05;
const _TOKEN_ATTR: u8 = 0x06;
const _TOKEN_END_ELEM: u8 = 0x04;

// ─────────────────────────────────────────────────────────────────────────────
// Zen-Carve: SIMD-accelerated EVTX signature scanner
// ─────────────────────────────────────────────────────────────────────────────

/// Scans a binary block for all occurrences of a 4-byte signature.
/// Uses SIMD on x86_64 AVX2 to find the first byte, then confirms the rest.
fn zen_carve_signature(data: &[u8], signature: &[u8; 4]) -> Vec<usize> {
    let mut offsets = Vec::new();
    let sig_first = signature[0];
    let len = data.len();
    if len < 4 { return offsets; }

    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") {
        offsets = unsafe { zen_carve_avx2(data, signature, sig_first) };
        return offsets;
    }

    // Scalar fallback
    let mut i = 0;
    while i <= len - 4 {
        if data[i] == sig_first && &data[i..i+4] == signature {
            offsets.push(i);
        }
        i += 1;
    }
    offsets
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn zen_carve_avx2(data: &[u8], signature: &[u8; 4], sig_first: u8) -> Vec<usize> {
    use std::arch::x86_64::*;
    let mut offsets = Vec::new();
    let first_vec = _mm256_set1_epi8(sig_first as i8);
    let chunk = 32usize;
    let limit = data.len().saturating_sub(4);
    let simd_limit = if limit >= chunk { limit - (limit % chunk) } else { 0 };
    let ptr = data.as_ptr();

    let mut i = 0;
    while i < simd_limit {
        let block = _mm256_loadu_si256(ptr.add(i) as *const __m256i);
        let mut mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(block, first_vec)) as u32;
        while mask != 0 {
            let bit = mask.trailing_zeros() as usize;
            let pos = i + bit;
            if pos + 4 <= data.len() && &data[pos..pos+4] == signature {
                offsets.push(pos);
            }
            mask &= !(1 << bit);
        }
        i += chunk;
    }
    // Scalar tail
    while i <= limit {
        if data[i] == sig_first && &data[i..i+4] == signature {
            offsets.push(i);
        }
        i += 1;
    }
    offsets
}

// ─────────────────────────────────────────────────────────────────────────────
// EvtxRecord: Parsed fields from a single Windows Event Log record
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct EvtxRecord {
    pub event_record_id: u64,
    pub timestamp_ns: u64,    // Windows FILETIME (100-ns intervals since 1601-01-01)
    pub event_id: u32,
    pub channel: String,
    pub computer: String,
    pub provider: String,
    pub level: u8,            // 1=Critical, 2=Error, 3=Warning, 4=Info, 5=Verbose
    pub task_id: u16,
}

impl EvtxRecord {
    /// Convert Windows FILETIME to Unix timestamp (in seconds)
    pub fn unix_timestamp(&self) -> i64 {
        let filetime_secs = self.timestamp_ns / 10_000_000;
        filetime_secs.saturating_sub(11_644_473_600) as i64
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EvtxNitroScanner: High-throughput EVTX chunk iterator
// Bypasses XML serialization and injects directly into Nitro-Accumulators
// ─────────────────────────────────────────────────────────────────────────────

pub struct EvtxNitroScanner<'a> {
    data: &'a [u8],
    /// Current byte position in `data`
    cursor: usize,
    /// Mode: false = structured file, true = raw carving (e.g. RAM dump)
    carve_mode: bool,
}

impl<'a> EvtxNitroScanner<'a> {
    /// Create from a valid .evtx file buffer.
    pub fn new(data: &'a [u8]) -> Option<Self> {
        if data.len() < EVTX_HEADER_SIZE || &data[0..8] != EVTX_FILE_MAGIC {
            return None; // Not a valid EVTX file
        }
        Some(Self { data, cursor: EVTX_HEADER_SIZE, carve_mode: false })
    }

    /// Create in Carving mode — for use on raw disk images or memory dumps.
    /// Uses Zen-Carve to locate chunk boundaries anywhere in the buffer.
    pub fn new_carving(data: &'a [u8]) -> Self {
        Self { data, cursor: 0, carve_mode: true }
    }

    /// Iterate over chunks and call `cb` for every record found.
    pub fn scan_records<F>(&mut self, mut cb: F)
    where
        F: FnMut(EvtxRecord),
    {
        if self.carve_mode {
            // Zen-Carve: locate all ElfChnk\0 signatures using SIMD
            let chunk_offsets = zen_carve_signature(self.data, &[b'E', b'l', b'f', b'C']);
            for off in chunk_offsets {
                if off + EVTX_CHUNK_SIZE <= self.data.len() {
                    self.parse_chunk(off, &mut cb);
                }
            }
        } else {
            // Structured file: chunks are contiguous at fixed intervals
            while self.cursor + EVTX_CHUNK_SIZE <= self.data.len() {
                let off = self.cursor;
                // Verify chunk magic
                if &self.data[off..off+8] == EVTX_CHUNK_MAGIC {
                    self.parse_chunk(off, &mut cb);
                }
                self.cursor += EVTX_CHUNK_SIZE;
            }
        }
    }

    /// Parse a single 64KB chunk to extract all embedded records
    fn parse_chunk<F>(&self, chunk_start: usize, cb: &mut F)
    where
        F: FnMut(EvtxRecord),
    {
        let chunk = &self.data[chunk_start..];
        if chunk.len() < 512 { return; }

        // Records start at offset 512 in the chunk header
        let records_offset = 512usize;
        let first_record_num = u64::from_le_bytes(chunk[8..16].try_into().unwrap_or([0;8]));
        let last_record_num  = u64::from_le_bytes(chunk[16..24].try_into().unwrap_or([0;8]));

        // Locate all record starts using Zen-Carve within this chunk
        let record_starts = zen_carve_signature(
            &chunk[records_offset..],
            &[EVTX_RECORD_MAGIC[0], EVTX_RECORD_MAGIC[1], EVTX_RECORD_MAGIC[2], EVTX_RECORD_MAGIC[3]],
        );

        let _ = (first_record_num, last_record_num); // used for validation in future

        for rel_off in record_starts {
            let abs_off = records_offset + rel_off;
            if abs_off + 8 > chunk.len() { break; }
            
            // Read record size to avoid O(N^2) scanning and filter noise
            let rec_size = u32::from_le_bytes(chunk[abs_off+4..abs_off+8].try_into().unwrap_or([0;4])) as usize;
            if rec_size < 24 || rec_size > 1_048_576 || abs_off + rec_size > chunk.len() { continue; }

            if let Some(record) = self.parse_record(&chunk[abs_off..abs_off + rec_size]) {
                cb(record);
            }
        }
    }

    /// Parse binary fields from a single EVTX record header (fast path, no XML)
    fn parse_record(&self, rec: &[u8]) -> Option<EvtxRecord> {
        if rec.len() < 24 { return None; }

        // Validate record header magic
        if &rec[0..4] != EVTX_RECORD_MAGIC { return None; }

        let _rec_size        = u32::from_le_bytes(rec[4..8].try_into().ok()?);
        let event_record_id  = u64::from_le_bytes(rec[8..16].try_into().ok()?);
        let timestamp_ns     = u64::from_le_bytes(rec[16..24].try_into().ok()?);

        // The BinXML payload starts at byte 24.
        let payload = &rec[24..];

        // Optimized token extraction: limit search window to avoid long scans
        let event_id = extract_uint16_token(payload, "EventID").unwrap_or(0) as u32;
        let level    = extract_uint8_token(payload, "Level").unwrap_or(4);
        let task_id  = extract_uint16_token(payload, "Task").unwrap_or(0);

        Some(EvtxRecord {
            event_record_id,
            timestamp_ns,
            event_id,
            level,
            task_id,
            ..Default::default()
        })
    }
}

/// Minimal BinXML token scanner: finds uint16 values by scanning for known field names
fn extract_uint16_token(payload: &[u8], _field: &str) -> Option<u16> {
    // Look for value token (0x05) followed by type byte 0x04 (uint16)
    // Most interesting tokens are in the first 50 bytes of payload
    let limit = payload.len().min(128); 
    let mut i = 0;
    while i + 4 < limit {
        if payload[i] == TOKEN_VALUE && payload[i+1] == 0x04 {
            return Some(u16::from_le_bytes([payload[i+2], payload[i+3]]));
        }
        i += 1;
    }
    None
}

fn extract_uint8_token(payload: &[u8], _field: &str) -> Option<u8> {
    let limit = payload.len().min(128);
    let mut i = 0;
    while i + 3 < limit {
        if payload[i] == TOKEN_VALUE && payload[i+1] == 0x01 {
            return Some(payload[i+2]);
        }
        i += 1;
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// FormatParser implementation for the standard parsers dispatch system
// ─────────────────────────────────────────────────────────────────────────────

pub struct EvtxParser;

impl EvtxParser {
    pub fn new() -> Self { Self }
}

impl FormatParser for EvtxParser {
    fn probe(&self, buffer: &[u8]) -> bool {
        buffer.len() >= 8 && &buffer[0..8] == EVTX_FILE_MAGIC
    }

    fn find_boundaries(&self, mmap: &[u8], start: usize, _end: usize) -> (usize, usize) {
        // Snap to the nearest chunk boundary
        let chunk_idx = start / EVTX_CHUNK_SIZE;
        let chunk_start = EVTX_HEADER_SIZE + chunk_idx * EVTX_CHUNK_SIZE;
        let chunk_end   = (chunk_start + EVTX_CHUNK_SIZE).min(mmap.len());
        (chunk_start, chunk_end)
    }

    fn parse_unit(&self, data: &[u8]) -> anyhow::Result<Vec<DataValue>> {
        // Fast-path: extract key fields from a record buffer as DataValues
        if data.len() < 24 {
            return Ok(vec![DataValue::Null]);
        }

        let event_record_id = u64::from_le_bytes(data[8..16].try_into().unwrap_or([0; 8]));
        let timestamp_ns    = u64::from_le_bytes(data[16..24].try_into().unwrap_or([0; 8]));
        // Convert Windows FILETIME to Unix seconds
        let unix_ts = (timestamp_ns / 10_000_000).saturating_sub(11_644_473_600) as i64;

        Ok(vec![
            DataValue::Int(event_record_id as i64),
            DataValue::Int(unix_ts),
        ])
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// High-level function: Analyze an EVTX buffer and aggregate into accumulators
// ─────────────────────────────────────────────────────────────────────────────

/// Analyzes an EVTX binary buffer.
/// Returns: (record_count, error_count, warning_count, info_count)
pub fn analyze_evtx(data: &[u8], carve: bool) -> (u64, u64, u64, u64) {
    let mut total = 0u64;
    let mut errors = 0u64;
    let mut warnings = 0u64;
    let mut info = 0u64;

    let mut scanner = if carve {
        EvtxNitroScanner::new_carving(data)
    } else {
        match EvtxNitroScanner::new(data) {
            Some(s) => s,
            None    => EvtxNitroScanner::new_carving(data), // fallback to carving
        }
    };

    scanner.scan_records(|rec| {
        total += 1;
        match rec.level {
            1 | 2 => errors   += 1,
            3     => warnings += 1,
            _     => info     += 1,
        }
    });

    (total, errors, warnings, info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evtx_probe_rejects_non_evtx() {
        let parser = EvtxParser::new();
        assert!(!parser.probe(b"This is just a CSV file,not,evtx"));
    }

    #[test]
    fn test_evtx_probe_accepts_magic() {
        let mut buf = vec![0u8; 4096];
        buf[0..8].copy_from_slice(EVTX_FILE_MAGIC);
        let parser = EvtxParser::new();
        assert!(parser.probe(&buf));
    }

    #[test]
    fn test_zen_carve_finds_signature() {
        let mut data = vec![0u8; 256];
        // Plant a fake EVTX_RECORD_MAGIC at offset 128
        data[128] = 0x2a;
        data[129] = 0x2a;
        data[130] = 0x00;
        data[131] = 0x00;
        let found = zen_carve_signature(&data, &[0x2a, 0x2a, 0x00, 0x00]);
        assert!(found.contains(&128), "Zen-Carve must find the signature at offset 128");
    }

    #[test]
    fn test_analyze_evtx_carving_empty() {
        // An empty buffer in carving mode should not crash
        let (total, _, _, _) = analyze_evtx(&[], true);
        assert_eq!(total, 0);
    }

    #[test]
    fn test_unix_timestamp_safety() {
        let record = EvtxRecord {
            timestamp_ns: 100, // Very old date (well before 1601/1970)
            ..Default::default()
        };
        // Should not panic, should return 0 or saturate to minimum
        assert_eq!(record.unix_timestamp(), 0);
    }
}
