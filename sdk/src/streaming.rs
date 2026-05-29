use std::io::{BufRead, BufReader};
use crate::engine::BigDataEngine;
use crate::types::{FileMetadata, AnalysisOptions};
use crate::accumulator::ColumnAccumulator;
use crate::compression::get_decompressor;
use blake3::Hasher;
use anyhow::Result;

pub struct StreamingAnalyzer<'a> {
    engine: &'a BigDataEngine,
    options: AnalysisOptions,
}

impl<'a> StreamingAnalyzer<'a> {
    pub fn new(engine: &'a BigDataEngine, options: AnalysisOptions) -> Self {
        Self { engine, options }
    }

    pub fn run(&mut self) -> Result<FileMetadata> {
        let start_time = std::time::Instant::now();
        let decompressor = get_decompressor(&self.engine.path)?;
        let mut reader = BufReader::with_capacity(256 * 1024, decompressor);
        
        // 1. Infer/Get Headers
        let headers = self.engine.infer_schema(100, self.options.enable_network, self.options.skip_rows)?;
        let num_cols = headers.len();
        let mut accumulators: Vec<ColumnAccumulator> = headers.iter()
            .map(|h| ColumnAccumulator::new(h.name.clone(), h.data_type.clone()))
            .collect();

        // JIT-Lite: Pre-bind the filter function (Phase K)
        let filter_ast = self.options.filter_ast.clone();
        
        // This closure is our "JIT" version - it avoids recursive matching on every row
        let mut filter_fn: Box<dyn FnMut(&[&[u8]]) -> bool> = if let Some(expr) = filter_ast {
            match expr {
                crate::filter::Expr::Compare(col, op, val) => {
                    let target_val = val.clone();
                    Box::new(move |fields| {
                        if let Some(field_bytes) = fields.get(col) {
                            let mut s = std::str::from_utf8(field_bytes).unwrap_or("").trim();
                            if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                                s = &s[1..s.len()-1];
                            }
                            match &target_val {
                                crate::filter::Value::Number(n) => {
                                    if let Ok(f_n) = s.parse::<f64>() {
                                        match op {
                                            crate::filter::Op::Eq => (f_n - n).abs() < 1e-9,
                                            crate::filter::Op::Gt => f_n > *n,
                                            crate::filter::Op::Lt => f_n < *n,
                                            _ => false,
                                        }
                                    } else { false }
                                }
                                crate::filter::Value::String(st) => {
                                    match op {
                                        crate::filter::Op::Eq => s.eq_ignore_ascii_case(st),
                                        _ => false,
                                    }
                                }
                            }
                        } else { false }
                    })
                }
                _ => Box::new(|_| true), // Fallback for complex expressions
            }
        } else {
            Box::new(|_| true)
        };
        let mut block_hashes = Vec::new();
        let block_size = 64 * 1024 * 1024; // 64MB blocks
        let mut current_block_bytes = 0;
        let mut current_block_hasher = Hasher::new();

        // 3. Optimized Loop (Phase J & K & L)
        let mut line = Vec::with_capacity(1024);
        let mut total_rows = 0;
        let mut row_idx = 0;
        
        let skip_total = self.options.skip_rows + if self.engine.has_header { 1 } else { 0 };

        while reader.read_until(b'\n', &mut line)? > 0 {
            // Block Hashing (L)
            current_block_hasher.update(&line);
            current_block_bytes += line.len();
            if current_block_bytes >= block_size {
                block_hashes.push((current_block_hasher.finalize().into(), current_block_bytes));
                current_block_hasher = Hasher::new();
                current_block_bytes = 0;
            }

            if row_idx < skip_total {
                row_idx += 1;
                line.clear();
                continue;
            }

            // 3. Extract fields (Zero-Copy)
            let mut fields = Vec::with_capacity(num_cols);
            let mut col_idx = 0;
            let mut field_start = 0;
            for (i, &b) in line.iter().enumerate() {
                if b == self.engine.delimiter || b == b'\n' || b == b'\r' {
                    fields.push(&line[field_start..i]);
                    col_idx += 1;
                    field_start = i + 1;
                }
            }
            if col_idx < num_cols && field_start < line.len() {
                fields.push(&line[field_start..]);
            }

            // 4. JIT-Lite Filter Apply (K)
            if filter_fn(&fields) {
                for (i, field) in fields.iter().enumerate() {
                    if i < num_cols {
                        accumulators[i].add(field);
                    }
                }
                total_rows += 1;
            }
            
            row_idx += 1;
            line.clear();
        }

        // Finalize last block hash
        if current_block_bytes > 0 {
            block_hashes.push((current_block_hasher.finalize().into(), current_block_bytes));
        }

        // Convert to FileMetadata
        let mut col_stats = Vec::new();
        for acc in &mut accumulators {
            col_stats.push(acc.finalize(None));
        }

        Ok(FileMetadata {
            file_name: self.engine.path.file_name().unwrap_or_default().to_string_lossy().into(),
            file_size_bytes: block_hashes.iter().map(|(_, s)| *s as u64).sum(),
            row_count: total_rows,
            duration_ms: start_time.elapsed().as_millis() as u64,
            columns: headers,
            column_stats: col_stats,
            segmented_stats: std::collections::HashMap::new(),
            correlations: Vec::new(),
            block_hashes,
            schema_type: "Streaming".to_string(),
        })
    }
}
