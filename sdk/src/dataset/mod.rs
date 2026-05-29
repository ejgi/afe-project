pub mod delta;
pub mod discovery;
pub mod analysis;

use crate::types::*;
use crate::engine::BigDataEngine;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use anyhow::Result;

pub struct VirtualDataset {
    pub engines: Vec<BigDataEngine>,
    pub base_path: PathBuf,
}

impl VirtualDataset {
    pub fn new<P: AsRef<Path>>(path: P, options: &AnalysisOptions) -> Result<Self> {
        discovery::new_impl(path, options)
    }

    pub fn find_files(
        base_path: &Path,
        query: &str,
        case_sensitive: bool,
        dirs_only: bool,
        files_only: bool,
        limit: usize,
        hardware_mode: HardwareMode,
    ) -> Result<Vec<(PathBuf, u64, bool)>> {
        discovery::find_files_impl(base_path, query, case_sensitive, dirs_only, files_only, limit, hardware_mode)
    }

    pub fn analyze(
        &self, 
        options: &AnalysisOptions,
        filter_col: Option<usize>,
        filter_min: Option<f64>,
        filter_max: Option<f64>,
        zm: Option<&ZoneMap>,
        filter_text: Option<&str>,
        filter_text_col: Option<usize>,
        filter_ast: Option<&crate::filter::Expr>,
        date_col: Option<usize>,
        date_from: Option<u32>,
        date_to: Option<u32>,
        progress_tx: Option<&std::sync::mpsc::Sender<FileMetadata>>,
        loop_count: usize,
    ) -> Result<FileMetadata> {
        analysis::analyze_impl(self, options, filter_col, filter_min, filter_max, zm, filter_text, filter_text_col, filter_ast, date_col, date_from, date_to, progress_tx, loop_count)
    }

    pub fn try_enable_gpu(&mut self) -> bool {
        let mut enabled = false;
        for engine in &mut self.engines {
            if engine.try_enable_gpu() { enabled = true; }
        }
        enabled
    }

    pub fn search(
        &self,
        query: &str,
        col: Option<usize>,
        limit: usize,
        raw: bool,
        gpu: bool,
        ignore_case: bool,
        indices_only: bool,
    ) -> Result<Vec<SearchResult>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let is_hdd = crate::utils::is_rotational(&self.base_path);
        
        if is_hdd {
            for engine in &self.engines {
                let results = if raw { engine.search_raw(query, limit, gpu, ignore_case, indices_only) }
                              else { engine.search_rows(query, col, limit, ignore_case, indices_only) };
                if let Ok(found) = results {
                    for (idx, content) in found {
                        let _ = tx.send(SearchResult { file_name: engine.path().display().to_string(), row_index: idx, content });
                    }
                }
            }
        } else {
            self.engines.par_iter().for_each_with(tx.clone(), |tx, engine| {
                let results = if raw { engine.search_raw(query, limit, gpu, ignore_case, indices_only) }
                              else { engine.search_rows(query, col, limit, ignore_case, indices_only) };
                if let Ok(found) = results {
                    for (idx, content) in found {
                        let _ = tx.send(SearchResult { file_name: engine.path().display().to_string(), row_index: idx, content });
                    }
                }
            });
        }
        drop(tx);
        let mut all_results: Vec<SearchResult> = rx.into_iter().collect();
        all_results.sort_by_key(|r| r.file_name.clone());
        all_results.truncate(limit);
        Ok(all_results)
    }

    pub fn search_iocs(&self, iocs: &[String], limit: usize) -> Result<Vec<SearchResult>> {
        let is_hdd = crate::utils::is_rotational(&self.base_path);
        let ac = aho_corasick::AhoCorasick::builder().build(iocs).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        let mut all_results = Vec::new();
        if is_hdd {
            for engine in &self.engines {
                if all_results.len() >= limit { break; }
                if let Ok(matches) = engine.search_iocs(&ac, limit - all_results.len(), iocs) {
                    for (pos, content) in matches {
                        all_results.push(SearchResult { file_name: engine.path().to_string_lossy().to_string(), row_index: pos, content });
                    }
                }
            }
        } else {
            all_results = self.engines.par_iter().flat_map(|engine| {
                match engine.search_iocs(&ac, limit, iocs) {
                    Ok(matches) => matches.into_iter().map(|(pos, content)| SearchResult { file_name: engine.path().to_string_lossy().to_string(), row_index: pos, content }).collect(),
                    Err(_) => Vec::new(),
                }
            }).collect();
        }
        all_results.sort_by_key(|r| r.file_name.clone());
        all_results.truncate(limit);
        Ok(all_results)
    }

    pub fn extract_ips(&self, mode: crate::types::IpScanMode) -> Result<Vec<IpFrequency>> {
        analysis::extract_ips_impl(self, None, mode)
    }

    pub fn extract_ips_cancellable(&self, token: Option<&std::sync::atomic::AtomicBool>, mode: crate::types::IpScanMode) -> Result<Vec<IpFrequency>> {
        analysis::extract_ips_impl(self, token, mode)
    }

    pub fn export_raw_search(&self, query: &str, output_path: &std::path::Path, gpu: bool) -> Result<u64> {
        if output_path.exists() { std::fs::remove_file(output_path)?; }
        let mut total_exported = 0;
        for engine in &self.engines {
            if let Ok(exported) = engine.export_raw_matches(query, output_path, gpu, false) {
                total_exported += exported;
            }
        }
        Ok(total_exported)
    }

    pub fn get_engine_count(&self) -> usize { self.engines.len() }
    pub fn get_total_rows(&self) -> usize { self.engines.iter().map(|e| e.offsets.len()).sum() }

    pub fn get_rows(&self, start_idx: usize, end_idx: usize) -> Result<Vec<String>> {
        let mut results = Vec::with_capacity(end_idx - start_idx);
        let mut current_offset = 0;
        for engine in &self.engines {
            let engine_rows = engine.offsets.len();
            let engine_end = current_offset + engine_rows;
            if start_idx < engine_end && end_idx > current_offset {
                let relative_start = if start_idx > current_offset { start_idx - current_offset } else { 0 };
                let relative_end = if end_idx < engine_end { end_idx - current_offset } else { engine_rows };
                if relative_start < relative_end { results.extend(engine.get_rows(relative_start, relative_end)); }
            }
            current_offset += engine_rows;
            if current_offset >= end_idx { break; }
        }
        Ok(results)
    }

    pub fn get_rows_with_meta(&self, start_idx: usize, end_idx: usize) -> Result<Vec<(String, String)>> {
        let mut results = Vec::with_capacity(end_idx - start_idx);
        let mut current_offset = 0;
        for engine in &self.engines {
            let engine_rows = engine.offsets.len();
            let engine_end = current_offset + engine_rows;
            if start_idx < engine_end && end_idx > current_offset {
                let relative_start = if start_idx > current_offset { start_idx - current_offset } else { 0 };
                let relative_end = if end_idx < engine_end { end_idx - current_offset } else { engine_rows };
                if relative_start < relative_end {
                    let rows = engine.get_rows(relative_start, relative_end);
                    let file_name = engine.path().file_name().unwrap_or_default().to_string_lossy().into_owned();
                    for row in rows { results.push((row, file_name.clone())); }
                }
            }
            current_offset += engine_rows;
            if current_offset >= end_idx { break; }
        }
        Ok(results)
    }

    pub fn get_total_size_mb(&self) -> f64 {
        self.engines.iter().map(|e| {
            if e.is_compressed { std::fs::metadata(&e.path).map(|m| m.len()).unwrap_or(0) as f64 / 1024.0 / 1024.0 }
            else { e.mmap.len() as f64 / 1024.0 / 1024.0 }
        }).sum()
    }

    pub fn group_by(&self, group_col: usize, agg_col: usize, filter: Option<&crate::filter::Expr>, use_gpu: bool) -> Result<Vec<GroupResult>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let is_hdd = crate::utils::is_rotational(&self.base_path);
        if is_hdd {
            for engine in &self.engines { if let Ok(results) = engine.group_by(group_col, agg_col, filter, use_gpu) { let _ = tx.send(results); } }
        } else {
            self.engines.par_iter().for_each_with(tx.clone(), |tx, engine| { if let Ok(results) = engine.group_by(group_col, agg_col, filter, use_gpu) { let _ = tx.send(results); } });
        }
        let mut final_groups: std::collections::HashMap<String, GroupResult> = std::collections::HashMap::new();
        for results in rx {
            for res in results {
                let entry = final_groups.entry(res.category.clone()).or_insert_with(|| GroupResult { category: res.category.clone(), count: 0, sum: 0.0, mean: 0.0, min: f64::MAX, max: f64::MIN });
                let n1 = entry.count as f64; let n2 = res.count as f64;
                if (n1 + n2) > 0.0 { entry.mean = (entry.mean * n1 + res.mean * n2) / (n1 + n2); }
                entry.count += res.count; entry.sum += res.sum;
                if res.min < entry.min { entry.min = res.min; }
                if res.max > entry.max { entry.max = res.max; }
            }
        }
        let mut out: Vec<GroupResult> = final_groups.into_values().collect();
        out.sort_by(|a, b| b.count.cmp(&a.count));
        Ok(out)
    }

    pub fn top_n(&self, col: usize, n: usize, desc: bool, filter: Option<&crate::filter::Expr>, use_gpu: bool) -> Result<Vec<String>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let is_hdd = crate::utils::is_rotational(&self.base_path);
        if is_hdd {
            for engine in &self.engines { if let Ok(results) = engine.top_n(col, n, desc, filter, use_gpu) { let _ = tx.send(results); } }
        } else {
            self.engines.par_iter().for_each_with(tx.clone(), |tx, engine| { if let Ok(results) = engine.top_n(col, n, desc, filter, use_gpu) { let _ = tx.send(results); } });
        }
        let mut all_rows: Vec<String> = Vec::new();
        for results in rx { all_rows.extend(results); }
        all_rows.sort_by(|a, b| {
            let delim = self.engines.first().map(|e| e.delimiter).unwrap_or(b',');
            let val_a = parse_col_numeric(a, col, delim);
            let val_b = parse_col_numeric(b, col, delim);
            if desc { val_b.partial_cmp(&val_a).unwrap_or(std::cmp::Ordering::Equal) }
            else { val_a.partial_cmp(&val_b).unwrap_or(std::cmp::Ordering::Equal) }
        });
        all_rows.truncate(n);
        Ok(all_rows)
    }

    pub fn update_row(&mut self, file_name: &str, row_idx: u64, content: String) -> Result<()> {
        if let Some(engine) = self.engines.iter_mut().find(|e| e.path().to_string_lossy().contains(file_name)) {
            if engine.delta.is_none() { engine.load_delta()?; }
            if let Some(ref mut delta) = engine.delta { delta.update_row(row_idx, content); delta.persist()?; }
        }
        Ok(())
    }

    pub fn delete_row(&mut self, file_name: &str, row_idx: u64) -> Result<()> {
        if let Some(engine) = self.engines.iter_mut().find(|e| e.path().to_string_lossy().contains(file_name)) {
            if engine.delta.is_none() { engine.load_delta()?; }
            if let Some(ref mut delta) = engine.delta { delta.delete_row(row_idx); delta.persist()?; }
        }
        Ok(())
    }

    pub fn join(&self, other: &Self, left_col_name: &str, right_col_name: &str, join_type: crate::analytics::join::JoinType) -> Result<Vec<crate::analytics::join::JoinedRow>> {
        analysis::join_impl(self, other, left_col_name, right_col_name, join_type)
    }
}

fn parse_col_numeric(row: &str, col: usize, delim: u8) -> f64 {
    row.split(delim as char).nth(col).and_then(|s| crate::utils::parse_numeric_fast(s.trim().as_bytes())).unwrap_or(0.0)
}
