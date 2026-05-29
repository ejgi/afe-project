use std::fs::File;
use std::io::Result;
use rayon::prelude::*;
use crate::types::{Zone, ZoneMap, ZoneStats};
use crate::utils::parse_numeric_fast;
use crate::BigDataEngine;

impl BigDataEngine {
    pub fn build_zone_map(&self, zone_size: u64, hardware_mode: crate::types::HardwareMode) -> Result<ZoneMap> {
        let is_mech = match hardware_mode {
            crate::types::HardwareMode::Auto => crate::utils::is_rotational(&self.path),
            crate::types::HardwareMode::HDD => true,
            crate::types::HardwareMode::SSD => false,
        };
        let num_threads = if is_mech { 2 } else { rayon::current_num_threads() };
        let pool = rayon::ThreadPoolBuilder::new().num_threads(num_threads).build().unwrap();

        let data = &self.mmap;
        let mut zones = Vec::new();
        
        // Initial detection of columns
        let mut data_start = 0;
        if data.len() >= 3 && &data[0..3] == b"\xEF\xBB\xBF" {
            data_start = 3;
        }

        // Skip preamble rows
        let mut cursor = data_start;
        for _ in 0..self.skip_rows {
            while cursor < data.len() && data[cursor] != b'\n' { cursor += 1; }
            if cursor < data.len() { cursor += 1; }
        }

        // Handle header
        if self.has_header {
            while cursor < data.len() && data[cursor] != b'\n' { cursor += 1; }
            if cursor < data.len() { cursor += 1; }
        }

        let body_start = cursor;
        let first_row = if cursor < data.len() {
            let mut e = cursor;
            while e < data.len() && data[e] != b'\n' { e += 1; }
            std::str::from_utf8(&data[cursor..e]).unwrap_or("")
        } else { "" };

        let num_cols = if !first_row.is_empty() {
            first_row.split(self.delimiter as char).count()
        } else { 0 };

        if num_cols == 0 {
            return Ok(ZoneMap { zones: Vec::new(), zone_size, num_cols: 0 });
        }

        // --- PARALLEL CHUNKING LOGIC ---
        // Divide the file into ~64MB chunks for parallel processing
        let chunk_size_bytes = 64 * 1024 * 1024;
        let mut chunk_starts = Vec::new();
        let mut chunk_cursor = body_start;
        while chunk_cursor < data.len() {
            chunk_starts.push(chunk_cursor);
            chunk_cursor += chunk_size_bytes;
            if chunk_cursor < data.len() {
                while chunk_cursor < data.len() && data[chunk_cursor] != b'\n' { chunk_cursor += 1; }
                if chunk_cursor < data.len() { chunk_cursor += 1; }
            }
        }

        let results: Vec<Vec<Zone>> = pool.install(|| {
            chunk_starts.into_par_iter().map(|start_offset| {
                let mut local_zones = Vec::new();
                let mut end_offset = (start_offset + chunk_size_bytes).min(data.len());
                if end_offset < data.len() {
                    while end_offset < data.len() && data[end_offset] != b'\n' { end_offset += 1; }
                    if end_offset < data.len() { end_offset += 1; }
                }

                let mut cursor = start_offset;
                let mut current_row_in_zone: u64 = 0;
                let mut zone_start_offset: u64 = start_offset as u64;
                let mut col_stats = vec![ZoneStats { min: None, max: None }; num_cols];

                while cursor < end_offset {
                    let line_start = cursor;
                    while cursor < end_offset && data[cursor] != b'\n' { cursor += 1; }
                    let line_end = cursor;
                    if cursor < end_offset { cursor += 1; }

                    let line = &data[line_start..line_end];
                    
                    for col_idx in 0..num_cols {
                        if let Some(field) = self.extract_field(line, col_idx) {
                            if let Some(v) = parse_numeric_fast(field) {
                                col_stats[col_idx].min = Some(col_stats[col_idx].min.map_or(v, |m: f64| m.min(v)));
                                col_stats[col_idx].max = Some(col_stats[col_idx].max.map_or(v, |m: f64| m.max(v)));
                            }
                        }
                    }

                    current_row_in_zone += 1;

                    if current_row_in_zone >= zone_size || cursor >= end_offset {
                        local_zones.push(Zone {
                            start_row: 0, // Patched during merge
                            end_row: current_row_in_zone,
                            start_offset: zone_start_offset,
                            end_offset: cursor as u64,
                            column_stats: col_stats.clone(),
                        });
                        zone_start_offset = cursor as u64;
                        current_row_in_zone = 0;
                        for stats in &mut col_stats { stats.min = None; stats.max = None; }
                    }
                }
                local_zones
            }).collect()
        });

        // --- MERGE AND PATCH ROW COUNTS ---
        let mut total_rows: u64 = 0;
        for local_list in results {
            for mut zone in local_list {
                let rows_in_zone = zone.end_row;
                zone.start_row = total_rows;
                total_rows += rows_in_zone;
                zone.end_row = total_rows;
                zones.push(zone);
            }
        }
        eprintln!("\rINTEL: Parallel Mapping Zones... 100.0% Done.");

        Ok(ZoneMap { zones, zone_size, num_cols })
    }

    pub fn save_zone_map(&self, zm: &ZoneMap) -> Result<()> {
        let path = self.path.with_extension("csv.zones");
        let f = File::create(path)?;
        serde_json::to_writer(f, zm)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(())
    }

    /// Returns the number of data columns detected in this engine's file.
    fn detect_num_cols(&self) -> usize {
        let data = &self.mmap;
        let mut cursor = 0;
        // Skip BOM
        if data.len() >= 3 && &data[0..3] == b"\xEF\xBB\xBF" { cursor = 3; }
        // Skip preamble rows
        for _ in 0..self.skip_rows {
            while cursor < data.len() && data[cursor] != b'\n' { cursor += 1; }
            if cursor < data.len() { cursor += 1; }
        }
        // Skip header row (we want the first data row for column counting)
        if self.has_header {
            while cursor < data.len() && data[cursor] != b'\n' { cursor += 1; }
            if cursor < data.len() { cursor += 1; }
        }
        let row_start = cursor;
        while cursor < data.len() && data[cursor] != b'\n' { cursor += 1; }
        if row_start >= cursor { return 0; }
        let first_row = std::str::from_utf8(&data[row_start..cursor]).unwrap_or("");
        first_row.split(self.delimiter as char).count()
    }

    /// Load a ZoneMap from disk and validate it is compatible with this engine's schema.
    pub fn load_zone_map(&self) -> Result<ZoneMap> {
        let path = self.path.with_extension("csv.zones");
        let f = File::open(&path)?;
        let zm: ZoneMap = serde_json::from_reader(f)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Validate column count; legacy files have num_cols == 0 (skip validation)
        if zm.num_cols > 0 {
            let actual_cols = self.detect_num_cols();
            if actual_cols != zm.num_cols {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "ZoneMap schema mismatch: '{}' has {} columns but the ZoneMap was built \
                         with {} columns. Please rebuild with `build-zones`.",
                        path.display(), actual_cols, zm.num_cols
                    ),
                ));
            }
        }

        Ok(zm)
    }
}
