use crate::engine::BigDataEngine;
use std::io::Result;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(crate) fn search_rows_impl(
    engine: &BigDataEngine,
    query: &str,
    col: Option<usize>,
    limit: usize,
    ignore_case: bool,
    indices_only: bool,
) -> Result<Vec<(usize, String)>> {
    use memchr::memmem;
    let needle = query.as_bytes();
    let data = &engine.mmap;
    let mut results = Vec::new();
    let finder = memmem::Finder::new(needle);
    
    for (i, &offset) in engine.offsets.iter().enumerate() {
        let s = offset as usize;
        let mut e = s;
        while e < data.len() && data[e] != b'\n' { e += 1; }
        let line = &data[s..e];
        
        let matched = if let Some(c) = col {
            if let Some(field) = engine.extract_field(line, c) {
                if ignore_case {
                    field.eq_ignore_ascii_case(needle)
                } else {
                    finder.find(field).is_some()
                }
            } else { false }
        } else {
            if ignore_case {
                line.windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle))
            } else {
                finder.find(line).is_some()
            }
        };
        
        if matched {
            let row_str = if indices_only {
                String::new()
            } else {
                String::from_utf8_lossy(line).trim().to_string()
            };
            results.push((i + 1, row_str));
            if results.len() >= limit { break; }
        }
    }
    Ok(results)
}

pub(crate) fn search_raw_impl(
    engine: &BigDataEngine,
    query: &str,
    limit: usize,
    use_gpu: bool,
    ignore_case: bool,
    _indices_only: bool,
) -> Result<Vec<(usize, String)>> {
    let data = &engine.mmap;
    let chunk_size = 128 * 1024 * 1024;
    let mut offset = 0usize;

    if use_gpu {
        if let Some(gpu) = &engine.gpu {
            let mut results: Vec<(usize, String)> = Vec::new();
            while offset < data.len() {
                let end = (offset + chunk_size).min(data.len());
                let chunk = &data[offset..end];
                if let Ok(indices) = gpu.run_filter(crate::compute::GpuDataSource::Memory(chunk), query.as_bytes()) {
                    for idx in indices {
                        let absolute_pos = offset + idx as usize;
                        let mut line_start = absolute_pos;
                        while line_start > 0 && data[line_start - 1] != b'\n' { line_start -= 1; }
                        let mut line_end = absolute_pos;
                        while line_end < data.len() && data[line_end] != b'\n' { line_end += 1; }
                        let line = String::from_utf8_lossy(&data[line_start..line_end]).trim().to_string();
                        results.push((absolute_pos, line));
                        if results.len() >= limit { return Ok(results); }
                    }
                }
                offset += chunk_size;
            }
            return Ok(results);
        }
    }

    let is_mech = match engine.hardware_mode {
        crate::types::HardwareMode::Auto => crate::utils::is_rotational(&engine.path),
        crate::types::HardwareMode::HDD => true,
        crate::types::HardwareMode::SSD => false,
    };

    if is_mech {
        use std::io::{Seek, SeekFrom, Read};
        let mut file = std::fs::File::open(&engine.path)?;
        let mut buffer = vec![0u8; chunk_size];
        let total_len = data.len();
        let all_offsets: Vec<usize> = (0..total_len).step_by(chunk_size).collect();
        let mut results = Vec::new();
        let found_count = AtomicUsize::new(0);

        for offset in all_offsets {
            if found_count.load(Ordering::Relaxed) >= limit { break; }
            let end = (offset + chunk_size).min(total_len);
            let current_chunk_size = end - offset;
            if file.seek(SeekFrom::Start(offset as u64)).is_err() { continue; }
            if file.read_exact(&mut buffer[..current_chunk_size]).is_err() { continue; }
            let chunk = &buffer[..current_chunk_size];
            let matches = engine.zenscan.scan_raw(chunk, query.as_bytes(), ignore_case);
            for &match_pos in &matches {
                if found_count.load(Ordering::Relaxed) >= limit { break; }
                let absolute_pos = offset + match_pos as usize;
                let row_idx = if engine.offsets.len() > 0 {
                    let pos_idx = engine.offsets.partition_point(|&x| x <= absolute_pos as u64).saturating_sub(1);
                    if engine.has_header && pos_idx == 0 { continue; }
                    if engine.has_header { pos_idx - 1 } else { pos_idx }
                } else { absolute_pos };
                results.push((row_idx, String::new()));
                found_count.fetch_add(1, Ordering::Relaxed);
            }
        }
        return Ok(results);
    }

    #[cfg(unix)]
    {
        use memmap2::Advice;
        let _ = engine.mmap.advise(Advice::Sequential);
    }

    let mut parallel_results: Vec<(usize, String)> = Vec::new();
    let matches = engine.zenscan.scan_raw(&data[..16*1024*1024.min(data.len())], query.as_bytes(), ignore_case);
    for &match_pos in &matches {
        if parallel_results.len() >= limit { break; }
        let absolute_pos = match_pos as usize;
        let row_idx = if engine.offsets.len() > 0 {
            let pos_idx = engine.offsets.partition_point(|&x| x <= absolute_pos as u64).saturating_sub(1);
            if engine.has_header && pos_idx == 0 { continue; }
            if engine.has_header { pos_idx - 1 } else { pos_idx }
        } else { absolute_pos };
        parallel_results.push((row_idx, String::new()));
    }
    Ok(parallel_results)
}

pub(crate) fn search_iocs_impl(
    engine: &BigDataEngine,
    _ctx: &aho_corasick::AhoCorasick,
    limit: usize,
    iocs: &[String],
) -> Result<Vec<(usize, String)>> {
    let mut results = Vec::new();
    let data = &engine.mmap;
    
    for ioc in iocs {
        if results.len() >= limit { break; }
        let ioc_matches = engine.zenscan.scan_raw(data, ioc.as_bytes(), true);
        for &pos in &ioc_matches {
            if results.len() >= limit { break; }
            let absolute_pos = pos as usize;
            let row_idx = if engine.offsets.len() > 0 {
                let pos_idx = engine.offsets.partition_point(|&x| x <= absolute_pos as u64).saturating_sub(1);
                if engine.has_header && pos_idx == 0 { continue; }
                if engine.has_header { pos_idx - 1 } else { pos_idx }
            } else { absolute_pos };
            
            let mut line_start = absolute_pos;
            while line_start > 0 && data[line_start - 1] != b'\n' { line_start -= 1; }
            let mut line_end = absolute_pos;
            while line_end < data.len() && data[line_end] != b'\n' { line_end += 1; }
            let content = String::from_utf8_lossy(&data[line_start..line_end]).trim().to_string();
            
            results.push((row_idx, content));
        }
    }
    Ok(results)
}
