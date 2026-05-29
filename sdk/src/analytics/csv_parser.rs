use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use fxhash::FxHashMap;
use crate::accumulator::ColumnAccumulator;
use crate::types::{DataType, Blueprint};
use crate::utils::parse_numeric_fast;
use memchr::memchr;

pub fn process_csv_chunk_basic(
    data: &[u8],
    work_chunk: &crate::analytics::ParChunk,
    delta: Option<&crate::dataset::delta::DeltaManager>,
    start_row: usize,
    _offsets: &[u64],
    num_cols: usize,
    limit_cols: usize,
    is_numeric_col: &[bool],
    accs: &[Arc<ColumnAccumulator>],
    tx: &std::sync::mpsc::Sender<crate::analytics::ChunkResult>,
) {
    let mut iter_idx = 0;
    let mut byte_cursor = match *work_chunk { crate::analytics::ParChunk::Bytes(s, _) => s, _ => 0 };
    let mut chunk_row_count = 0;
    
    // Fast thread-local batch buffers for SIMD
    let mut batch_buffers = vec![Vec::with_capacity(512); num_cols];

    loop {
        let current_row_idx: Option<u64> = match *work_chunk {
            crate::analytics::ParChunk::Offsets(_, _) => Some((start_row + iter_idx) as u64),
            _ => None,
        };

        // Delta check
        let mut delta_line: Option<&[u8]> = None;
        if let (Some(ridx), Some(ref d)) = (current_row_idx, delta) {
            if d.is_deleted(ridx) { iter_idx += 1; continue; }
            if let Some(updated) = d.get_update(ridx) { delta_line = Some(updated.as_bytes()); }
        }

        let line = if let Some(dl) = delta_line {
            iter_idx += 1; dl
        } else {
            match *work_chunk {
                crate::analytics::ParChunk::Offsets(chunk_offs, _) => {
                    if iter_idx >= chunk_offs.len() { break; }
                    let start_idx = chunk_offs[iter_idx] as usize;
                    let end_idx = if iter_idx + 1 < chunk_offs.len() {
                        chunk_offs[iter_idx + 1] as usize - 1
                    } else if start_idx < data.len() {
                        memchr(b'\n', &data[start_idx..]).map(|pos| start_idx + pos).unwrap_or(data.len())
                    } else { start_idx };
                    iter_idx += 1;
                    &data[start_idx..end_idx]
                }
                crate::analytics::ParChunk::Bytes(_, end_idx) => {
                    if byte_cursor >= end_idx { break; }
                    let s_idx = byte_cursor;
                    let e_idx = memchr(b'\n', &data[s_idx..end_idx]).map(|pos| s_idx + pos).unwrap_or(end_idx);
                    byte_cursor = e_idx + 1;
                    &data[s_idx..e_idx]
                }
            }
        };
        chunk_row_count += 1;

        let mut field_idx = 0;
        let mut field_start = 0;
        
        for (j, &byte) in line.iter().enumerate() {
            if byte == b',' {
                if field_idx < limit_cols {
                    if is_numeric_col[field_idx] {
                        if let Some(x) = parse_numeric_fast(&line[field_start..j]) {
                            batch_buffers[field_idx].push(x);
                            if batch_buffers[field_idx].len() >= 512 {
                                let buf = std::mem::replace(&mut batch_buffers[field_idx], Vec::with_capacity(512));
                                accs[field_idx].batch_update_basic(&buf);
                            }
                        }
                    } else {
                         if line[field_start..j].is_empty() || &line[field_start..j] == b"\"\"" {
                             accs[field_idx].update_null();
                         } else {
                             accs[field_idx].count.fetch_add(1, Ordering::Relaxed);
                         }
                    }
                }
                field_idx += 1;
                field_start = j + 1;
            }
        }
        
        // Final column
        if field_idx < limit_cols {
            if is_numeric_col[field_idx] {
                if let Some(x) = parse_numeric_fast(&line[field_start..]) {
                    batch_buffers[field_idx].push(x);
                    if batch_buffers[field_idx].len() >= 512 {
                        let buf = std::mem::replace(&mut batch_buffers[field_idx], Vec::with_capacity(512));
                        accs[field_idx].batch_update_basic(&buf);
                    }
                }
            } else {
                 if line[field_start..].is_empty() || &line[field_start..] == b"\"\"" {
                     accs[field_idx].update_null();
                 } else {
                     accs[field_idx].count.fetch_add(1, Ordering::Relaxed);
                 }
            }
        }
    }

    // Flush remaining SIMD batches
    for (i, buf) in batch_buffers.iter_mut().enumerate() {
        if !buf.is_empty() {
            accs[i].batch_update_basic(buf);
            buf.clear();
        }
    }

    let _ = tx.send(crate::analytics::ChunkResult { row_count: chunk_row_count });
}

fn flush_categorical_cache_to_accs(cache: &mut FxHashMap<&[u8], u64>, acc: &Arc<ColumnAccumulator>) {
    for (bytes, count) in cache.drain() {
        let cat_str = String::from_utf8_lossy(bytes).trim().to_string();
        if cat_str.is_empty() {
            acc.null_count.fetch_add(count as u64, Ordering::Relaxed);
        } else {
            acc.count.fetch_add(count as u64, Ordering::Relaxed);
            acc.categories.entry(cat_str).or_insert_with(|| AtomicU64::new(0)).fetch_add(count as u64, Ordering::Relaxed);
        }
    }
}

pub fn process_csv_chunk_full(
    data: &[u8],
    work_chunk: &crate::analytics::ParChunk,
    delta: Option<&crate::dataset::delta::DeltaManager>,
    start_row: usize,
    _offsets: &[u64],
    num_cols: usize,
    limit_cols: usize,
    is_numeric_col: &[bool],
    accs: &[Arc<ColumnAccumulator>],
    f_accs: &[Arc<ColumnAccumulator>],
    blueprint: Option<&Blueprint>,
    header: &[String],
    filter_ast: Option<&crate::filter::Expr>,
    tx: &std::sync::mpsc::Sender<crate::analytics::ChunkResult>,
) {
    let mut iter_idx = 0;
    let mut byte_cursor = match *work_chunk { crate::analytics::ParChunk::Bytes(s, _) => s, _ => 0 };
    let mut chunk_row_count = 0;
    let has_blueprint = blueprint.is_some();
    
    let mut batch_buffers = vec![Vec::with_capacity(512); num_cols];
    let mut cl_categorical_cache: Vec<FxHashMap<&[u8], u64>> = (0..num_cols).map(|_| FxHashMap::default()).collect();
    let mut vals = vec![0.0f64; num_cols];
    let mut present = vec![false; num_cols];
    let mut segmented_accs: FxHashMap<String, Vec<ColumnAccumulator>> = FxHashMap::default();

    // JIT-Lite: Pre-bind the filter function (Phase K Optimization)
    let mut filter_logic: Box<dyn FnMut(&[f64], &[bool]) -> bool> = if let Some(expr) = filter_ast {
        match expr {
            crate::filter::Expr::Compare(col_idx, op, val) => {
                let col = *col_idx;
                let target_op = *op;
                let target_val = match val {
                    crate::filter::Value::Number(n) => *n,
                    _ => 0.0,
                };
                Box::new(move |vals, present| {
                    if let Some(&p) = present.get(col) {
                        if p {
                            let v = vals[col];
                            match target_op {
                                crate::filter::Op::Eq => (v - target_val).abs() < 1e-9,
                                crate::filter::Op::Gt => v > target_val,
                                crate::filter::Op::Lt => v < target_val,
                                _ => true,
                            }
                        } else { false }
                    } else { false }
                })
            }
            _ => Box::new(|_, _| true),
        }
    } else {
        Box::new(|_, _| true)
    };


    loop {
        let current_row_idx: Option<u64> = match *work_chunk {
            crate::analytics::ParChunk::Offsets(_, _) => Some((start_row + iter_idx) as u64),
            _ => None,
        };

        let mut delta_line: Option<&[u8]> = None;
        if let (Some(ridx), Some(ref d)) = (current_row_idx, delta) {
            if d.is_deleted(ridx) { iter_idx += 1; continue; }
            if let Some(updated) = d.get_update(ridx) { delta_line = Some(updated.as_bytes()); }
        }

        let line = if let Some(dl) = delta_line {
            iter_idx += 1; dl
        } else {
            match *work_chunk {
                crate::analytics::ParChunk::Offsets(chunk_offs, _) => {
                    if iter_idx >= chunk_offs.len() { break; }
                    let start_idx = chunk_offs[iter_idx] as usize;
                    let end_idx = if iter_idx + 1 < chunk_offs.len() {
                        chunk_offs[iter_idx + 1] as usize - 1
                    } else if start_idx < data.len() {
                        memchr(b'\n', &data[start_idx..]).map(|pos| start_idx + pos).unwrap_or(data.len())
                    } else { start_idx };
                    iter_idx += 1;
                    &data[start_idx..end_idx]
                }
                crate::analytics::ParChunk::Bytes(_, end_idx) => {
                    if byte_cursor >= end_idx { break; }
                    let s_idx = byte_cursor;
                    let e_idx = memchr(b'\n', &data[s_idx..end_idx]).map(|pos| s_idx + pos).unwrap_or(end_idx);
                    byte_cursor = e_idx + 1;
                    &data[s_idx..e_idx]
                }
            }
        };
        chunk_row_count += 1;

        let mut field_idx = 0;
        let mut field_start = 0;
        for v in &mut vals { *v = 0.0; }
        for p in &mut present { *p = false; }
        let mut field_bounds = vec![(0u32, 0u32); num_cols];

        for (j, &byte) in line.iter().enumerate() {
            if byte == b',' {
                if field_idx < limit_cols {
                    field_bounds[field_idx] = (field_start as u32, j as u32);
                    let field_bytes = &line[field_start..j];
                    if is_numeric_col[field_idx] {
                        if let Some(x) = parse_numeric_fast(field_bytes) {
                            batch_buffers[field_idx].push(x);
                            if batch_buffers[field_idx].len() >= 512 {
                                let buf = std::mem::replace(&mut batch_buffers[field_idx], Vec::with_capacity(512));
                                accs[field_idx].batch_update(&buf);
                            }
                            vals[field_idx] = x;
                            present[field_idx] = true;
                        }
                    } else {
                        let mut bytes = field_bytes;
                        if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len()-1] == b'"' {
                            bytes = &bytes[1..bytes.len()-1];
                        }
                        let count = cl_categorical_cache[field_idx].entry(bytes).or_insert(0);
                        *count += 1;
                        if cl_categorical_cache[field_idx].len() > 1000 {
                            flush_categorical_cache_to_accs(&mut cl_categorical_cache[field_idx], &accs[field_idx]);
                        }
                    }
                }
                field_idx += 1;
                field_start = j + 1;
            }
        }
        
        if field_idx < limit_cols {
            field_bounds[field_idx] = (field_start as u32, line.len() as u32);
            let field_bytes = &line[field_start..];
            if is_numeric_col[field_idx] {
                if let Some(x) = parse_numeric_fast(field_bytes) {
                    batch_buffers[field_idx].push(x);
                    if batch_buffers[field_idx].len() >= 512 {
                        let buf = std::mem::replace(&mut batch_buffers[field_idx], Vec::with_capacity(512));
                        accs[field_idx].batch_update(&buf);
                    }
                    vals[field_idx] = x;
                    present[field_idx] = true;
                }
            } else {
                let mut bytes = field_bytes;
                if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len()-1] == b'"' {
                    bytes = &bytes[1..bytes.len()-1];
                }
                let count = cl_categorical_cache[field_idx].entry(bytes).or_insert(0);
                *count += 1;
                if cl_categorical_cache[field_idx].len() > 1000 {
                    flush_categorical_cache_to_accs(&mut cl_categorical_cache[field_idx], &accs[field_idx]);
                }
            }
        }
        
        // 4. JIT-Lite Filter Apply (K)
        if !filter_logic(&vals, &present) {
            continue;
        }

        if has_blueprint {
            if let Some(ref bp) = blueprint {
                if let Some(disc_idx) = bp.discriminator_col {
                    if disc_idx < num_cols && disc_idx <= field_idx {
                        let (s, e) = field_bounds[disc_idx];
                        let disc_bytes = &line[s as usize..e as usize];
                        let disc_val = String::from_utf8_lossy(disc_bytes).trim().to_string();

                        let seg_accs = segmented_accs.entry(disc_val).or_insert_with(|| {
                            header.iter().map(|h| ColumnAccumulator::new(h.clone(), DataType::Numeric)).collect::<Vec<ColumnAccumulator>>()
                        });
                        for (f_i, &p) in present.iter().enumerate().take(limit_cols) {
                            if p { seg_accs[f_i].update(vals[f_i]); }
                        }
                    }
                }
                if !bp.formulas.is_empty() {
                    for (f_i, _) in bp.formulas.iter().enumerate() {
                        if let Some(result) = bp.formulas[f_i].evaluate_fast(&vals[..limit_cols]) {
                            f_accs[f_i].update(result);
                        }
                    }
                }
            }
        }
    }

    for (f_idx, mut cache) in cl_categorical_cache.into_iter().enumerate() {
        flush_categorical_cache_to_accs(&mut cache, &accs[f_idx]);
    }

    for (i, buf) in batch_buffers.iter_mut().enumerate() {
        if !buf.is_empty() {
            accs[i].batch_update(buf);
            buf.clear();
        }
    }

    let _ = tx.send(crate::analytics::ChunkResult { row_count: chunk_row_count });
}
