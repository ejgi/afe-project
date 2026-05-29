use std::io::{Write, BufWriter};
use std::fs::File;
use std::io::Result;
use rayon::prelude::*;
use crate::engine::BigDataEngine;

impl BigDataEngine {
    pub fn export_rows(
        &self,
        output_path: &std::path::Path,
        format: &str,
        options: crate::types::AnalysisOptions,
        filter_col: Option<usize>,
        filter_min: Option<f64>,
        filter_max: Option<f64>,
        filter_text_col: Option<usize>,
        filter_text: Option<&str>,
        zone_map: Option<&crate::types::ZoneMap>,
        filter_ast: Option<&crate::filter::Expr>,
        date_col: Option<usize>,
        date_from: Option<u32>,
        date_to: Option<u32>,
    ) -> Result<u64> {
        let data = &self.mmap;
        let delimiter = options.delimiter.unwrap_or(self.delimiter);
        let regex_str = options.regex_pattern.as_ref()
            .or(options.blueprint.as_ref().and_then(|bp| bp.regex_pattern.as_ref()));
        let compiled_regex = regex_str.map(|r| regex::bytes::Regex::new(r).expect("Invalid regex"));

        // --- Header detection ---
        let mut data_start = 0;
        if data.len() >= 3 && &data[0..3] == b"\xEF\xBB\xBF" {
            data_start = 3;
        }
        let mut header_end = data_start;
        if self.has_header {
            while header_end < data.len() && data[header_end] != b'\n' { header_end += 1; }
        }

        // Parse headers for JSON mode
        let headers: Vec<String> = if self.has_header {
            if self.rfc_4180 {
                let mut cols = Vec::new();
                let mut col_idx = 0;
                while let Some(field) = self.extract_field_rfc4180(&data[data_start..header_end], col_idx) {
                    cols.push(String::from_utf8_lossy(field).trim().to_string());
                    col_idx += 1;
                }
                cols
            } else {
                std::str::from_utf8(&data[data_start..header_end])
                    .unwrap_or("")
                    .split(delimiter as char)
                    .map(|s| s.trim().to_string())
                    .collect()
            }
        } else {
            Vec::new()
        };

        // --- Determine which offsets to scan (ZoneMap pre-filter) ---
        let mut temp_offsets = Vec::new();
        let scan_offsets: &[u64] = if let (Some(zm), Some(fc)) = (zone_map, filter_col) {
            for zone in &zm.zones {
                let stats = &zone.column_stats[fc];
                let mut skip = false;
                if let (Some(zmax), Some(fmin)) = (stats.max, filter_min) {
                    if zmax < fmin { skip = true; }
                }
                if let (Some(zmin), Some(fmax)) = (stats.min, filter_max) {
                    if zmin > fmax { skip = true; }
                }
                if !skip {
                    let s = zone.start_row as usize;
                    let e = zone.end_row as usize;
                    temp_offsets.extend_from_slice(&self.offsets[s..e]);
                }
            }
            &temp_offsets
        } else {
            &self.offsets
        };

        // --- PARALLEL FILTER: collect matching (offset, row_bytes) pairs ---
        // Split offsets into chunks ~ 100k rows each and process in parallel.
        let chunk_size = 100_000.max(scan_offsets.len() / rayon::current_num_threads().max(1) + 1);

        // LIMIT PARALLELISM ON HDD to avoid head thrashing
        let is_mech = match options.hardware_mode {
            crate::types::HardwareMode::Auto => crate::utils::is_rotational(&self.path),
            crate::types::HardwareMode::HDD => true,
            crate::types::HardwareMode::SSD => false,
        };
        let num_threads = if is_mech { 2 } else { rayon::current_num_threads() };
        let pool = rayon::ThreadPoolBuilder::new().num_threads(num_threads).build().unwrap();

        let matched_chunks: Vec<Vec<(u64, Vec<u8>)>> = pool.install(|| {
            scan_offsets
                .par_chunks(chunk_size)
                .map(|chunk| {
                    let mut local: Vec<(u64, Vec<u8>)> = Vec::new();
                    for (idx, &offset) in chunk.iter().enumerate() {
                        let s = offset as usize;
                        // Find line end
                        let e = if idx + 1 < chunk.len() {
                            (chunk[idx + 1] as usize).saturating_sub(1)
                        } else {
                            memchr::memchr(b'\n', &data[s..])
                                .map(|pos| s + pos)
                                .unwrap_or(data.len())
                        };
                        let line = &data[s..e];
                        if line.is_empty() { continue; }

                        // Apply regex pre-filter
                        if let Some(ref re) = compiled_regex {
                            if !re.is_match(line) { continue; }
                        }

                        if !self.row_matches(
                            line,
                            filter_col,
                            filter_min,
                            filter_max,
                            filter_text_col,
                            filter_text,
                            filter_ast,
                            date_col,
                            date_from,
                            date_to,
                        ) { continue; }

                        local.push((offset, line.to_vec()));
                    }
                    local
                })
                .collect()
        });

        // --- SEQUENTIAL WRITE (ordered by offset to preserve CSV order) ---
        let file = File::create(output_path)?;
        let mut out = BufWriter::with_capacity(4 * 1024 * 1024, file);
        let mut exported: u64 = 0;

        // Write header
        if self.has_header && format == "csv" {
            out.write_all(&data[data_start..header_end])?;
            out.write_all(b"\n")?;
        } else if format == "json" {
            out.write_all(b"[\n")?;
        }

        // Flatten chunks (already ordered because par_chunks preserves chunk order and
        // rows within each chunk are processed in offset order)
        for chunk in &matched_chunks {
            for (_, row_bytes) in chunk {
                if format == "json" {
                    // Build JSON object
                    let fields: Vec<String> = if self.rfc_4180 {
                        let mut cols = Vec::new();
                        let mut col_idx = 0;
                        while let Some(field) = self.extract_field_rfc4180(row_bytes, col_idx) {
                            cols.push(String::from_utf8_lossy(field).trim().to_string());
                            col_idx += 1;
                        }
                        cols
                    } else {
                        std::str::from_utf8(row_bytes)
                            .unwrap_or("")
                            .split(delimiter as char)
                            .map(|s| s.trim().to_string())
                            .collect()
                    };

                    let mut row_map = std::collections::HashMap::new();
                    let eff_headers: Vec<String> = if !headers.is_empty() {
                        headers.clone()
                    } else {
                        (0..fields.len()).map(|i| format!("col_{}", i)).collect()
                    };
                    for (i, h) in eff_headers.iter().enumerate() {
                        if let Some(val) = fields.get(i) {
                            row_map.insert(h.clone(), val.clone());
                        }
                    }
                    if exported > 0 { out.write_all(b",\n")?; }
                    let json_row = serde_json::to_string(&row_map).unwrap();
                    out.write_all(json_row.as_bytes())?;
                } else {
                    // CSV
                    out.write_all(row_bytes)?;
                    out.write_all(b"\n")?;
                }
                exported += 1;
            }
        }

        if format == "json" {
            out.write_all(b"\n]")?;
        }

        Ok(exported)
    }
}
