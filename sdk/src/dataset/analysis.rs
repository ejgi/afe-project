use crate::types::*;
use anyhow::{Result, anyhow};
use rayon::prelude::*;
use crate::dataset::VirtualDataset;

pub(crate) fn analyze_impl(
    dataset: &VirtualDataset,
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
    if dataset.engines.is_empty() {
        return Ok(FileMetadata::default());
    }

    let is_hdd = match options.hardware_mode {
        HardwareMode::Auto => crate::utils::is_rotational(&dataset.base_path),
        HardwareMode::HDD => true,
        HardwareMode::SSD => false,
    };
    
    let (tx, rx) = std::sync::mpsc::channel();
    
    if is_hdd {
        for _ in 0..loop_count {
            dataset.engines.iter().for_each(|engine| {
                let res = engine.analyze_csv(
                    options.clone(), filter_col, filter_min, filter_max, zm,
                    filter_text, filter_text_col, filter_ast,
                    date_col, date_from, date_to,
                    progress_tx
                );
                let _ = tx.send(res);
            });
        }
    } else {
        for _ in 0..loop_count {
            dataset.engines.par_iter().for_each_with(tx.clone(), |tx, engine| {
                let res = engine.analyze_csv(
                    options.clone(), filter_col, filter_min, filter_max, zm,
                    filter_text, filter_text_col, filter_ast,
                    date_col, date_from, date_to,
                    progress_tx
                );
                let _ = tx.send(res);
            });
        }
    }

    drop(tx);

    let mut total_meta = FileMetadata {
        file_name: dataset.base_path.display().to_string(),
        file_size_bytes: 0,
        row_count: 0,
        duration_ms: 0,
        columns: Vec::new(),
        column_stats: Vec::new(),
        segmented_stats: std::collections::HashMap::new(),
        correlations: Vec::new(),
        block_hashes: Vec::new(),
        schema_type: "VirtualDataset".to_string(),
    };
    let mut best_corr_row_count: u64 = 0;
    let mut column_name_to_idx: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for res in rx {
        let meta = res?;
        total_meta.file_size_bytes += meta.file_size_bytes;
        total_meta.row_count += meta.row_count;
        total_meta.duration_ms += meta.duration_ms;
        total_meta.block_hashes.extend(meta.block_hashes);

        for (incoming_idx, col_schema) in meta.columns.iter().enumerate() {
            let name = col_schema.name.trim().to_lowercase();
            let target_idx = *column_name_to_idx.entry(name.clone()).or_insert_with(|| {
                let new_idx = total_meta.columns.len();
                total_meta.columns.push(col_schema.clone());
                total_meta.column_stats.push(ColumnStats {
                    name: col_schema.name.clone(),
                    min: f64::MAX, max: f64::MIN, ..Default::default()
                });
                new_idx
            });
            if let Some(incoming_stats) = meta.column_stats.get(incoming_idx) {
                merge_column_stats(&mut total_meta.column_stats[target_idx], incoming_stats);
            }
        }

        for (key, incoming_segs) in meta.segmented_stats {
            let segment_entries = total_meta.segmented_stats.entry(key)
                .or_insert_with(|| vec![ColumnStats::default(); total_meta.columns.len()]);
            if segment_entries.len() < total_meta.columns.len() {
                segment_entries.resize(total_meta.columns.len(), ColumnStats::default());
            }
            for (incoming_idx, col_schema) in meta.columns.iter().enumerate() {
                let name = col_schema.name.trim().to_lowercase();
                if let Some(&target_idx) = column_name_to_idx.get(&name) {
                    if let Some(inc_stat) = incoming_segs.get(incoming_idx) {
                        merge_column_stats(&mut segment_entries[target_idx], inc_stat);
                    }
                }
            }
        }
        if meta.row_count > best_corr_row_count {
            total_meta.correlations = meta.correlations;
            best_corr_row_count = meta.row_count;
        }
    }
    Ok(total_meta)
}

pub(crate) fn extract_ips_impl(dataset: &VirtualDataset, token: Option<&std::sync::atomic::AtomicBool>, mode: crate::types::IpScanMode) -> Result<Vec<IpFrequency>> {
    use std::collections::HashMap;
    use crate::analytics::ioc::{IpValue, IpMetadata};
    use std::net::IpAddr;
    use std::sync::atomic::Ordering;

    let is_hdd = crate::utils::is_rotational(&dataset.base_path);
    
    let mut engine_results: Vec<(String, HashMap<IpValue, IpMetadata>)> = Vec::new();

    if is_hdd {
        for engine in &dataset.engines {
            if let Some(t) = token {
                if t.load(Ordering::SeqCst) { break; }
            }
            if let Ok(map) = engine.extract_ips(token, mode) {
                engine_results.push((engine.path().to_string_lossy().to_string(), map));
            }
        }
    } else {
        let results: Vec<_> = dataset.engines.par_iter().map(|engine| {
            if let Some(t) = token {
                if t.load(Ordering::SeqCst) { return None; }
            }
            engine.extract_ips(token, mode).ok().map(|map| (engine.path().to_string_lossy().to_string(), map))
        }).while_some().collect();
        engine_results = results;
    }

    let mut global_map: HashMap<IpValue, (usize, usize, HashMap<String, usize>)> = HashMap::new();
    for (file_path, local_map) in engine_results {
        for (ip, meta) in local_map {
            let entry = global_map.entry(ip).or_insert((0, 0, HashMap::new()));
            entry.0 += meta.hits;
            entry.1 += meta.noise_hits;
            *entry.2.entry(file_path.clone()).or_insert(0) += meta.hits;
        }
    }

    let db_bytes = include_bytes!("../../resources/GeoLite2-Country.mmdb");
    let geoip_db = maxminddb::Reader::from_source(db_bytes.as_slice()).ok();

    use rayon::prelude::*;

    let mut final_results: Vec<IpFrequency> = global_map.into_par_iter().map(|(ip_val, (total, total_noise, file_dist)): (IpValue, (usize, usize, HashMap<String, usize>))| {
        let mut top_files: Vec<FileHit> = file_dist.into_iter()
            .map(|(path, count)| FileHit { path, count })
            .collect();
        top_files.sort_by(|a, b| b.count.cmp(&a.count));
        top_files.truncate(50);

        let ip_addr = match ip_val {
            IpValue::V4(v) => IpAddr::V4(std::net::Ipv4Addr::new(((v >> 24) & 0xFF) as u8, ((v >> 16) & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, (v & 0xFF) as u8)),
            IpValue::V6(v) => IpAddr::V6(std::net::Ipv6Addr::from(v)),
        };

        // Skip GeoIP for Private/Local IPs (RFC1918)
        let is_private = match ip_addr {
            IpAddr::V4(v) => v.is_private() || v.is_loopback() || v.is_link_local(),
            IpAddr::V6(v) => v.is_loopback(), // Simplified for v6 in Nitro-GeoIP
        };

        let mut country_code = None;
        let mut country_name = None;

        if !is_private {
            if let Some(ref reader) = geoip_db {
                if let Ok(country_info) = reader.lookup::<maxminddb::geoip2::Country>(ip_addr) {
                    if let Some(country) = country_info.country {
                        if let Some(iso) = country.iso_code {
                            country_code = Some(iso.to_string());
                        }
                        if let Some(names) = country.names {
                            if let Some(name) = names.get("en") {
                                country_name = Some(name.to_string());
                            }
                        }
                    }
                }
            }
        }

        let is_noise = total > 0 && total_noise == total;

        IpFrequency { ip: ip_addr.to_string(), count: total, country_code, country_name, is_noise, top_files }
    }).collect();

    final_results.par_sort_by(|a, b| b.count.cmp(&a.count));
    Ok(final_results)
}

pub(crate) fn join_impl(
    dataset: &VirtualDataset,
    other: &VirtualDataset,
    left_col_name: &str,
    right_col_name: &str,
    join_type: crate::analytics::join::JoinType,
) -> Result<Vec<crate::analytics::join::JoinedRow>> {
    use crate::analytics::join::{HashJoin, JoinTable};
    
    let mut left_rows = Vec::new();
    let mut left_found = false;
    let left_needle = left_col_name.trim().to_lowercase();
    for engine in &dataset.engines {
        if let Ok(schema) = engine.infer_schema(10, false, engine.skip_rows) {
            if let Some(pos) = schema.iter().position(|s| s.name.trim().to_lowercase() == left_needle) {
                left_found = true;
                let start = engine.skip_rows + if engine.has_header { 1 } else { 0 };
                for row_str in engine.get_rows(start, usize::MAX) { 
                    let mut fields = Vec::new();
                    let row_bytes = row_str.as_bytes();
                    let mut col_idx = 0;
                    while let Some(field) = engine.extract_field(row_bytes, col_idx) {
                        let mut s = std::str::from_utf8(field).unwrap_or("").trim().to_string();
                        if s.starts_with('"') && s.ends_with('"') { s = s[1..s.len()-1].to_string(); }
                        fields.push(s); col_idx += 1;
                    }
                    if pos < fields.len() {
                        let key = fields.swap_remove(pos);
                        let mut normalized = vec![key]; normalized.extend(fields);
                        left_rows.push(normalized); 
                    }
                }
            }
        }
    }
    if !left_found { return Err(anyhow!("Left col '{}' not found", left_col_name)); }

    let mut right_rows = Vec::new();
    let mut right_found = false;
    let right_needle = right_col_name.trim().to_lowercase();
    for engine in &other.engines {
        if let Ok(schema) = engine.infer_schema(10, false, engine.skip_rows) {
            if let Some(pos) = schema.iter().position(|s| s.name.trim().to_lowercase() == right_needle) {
                right_found = true;
                let start = engine.skip_rows + if engine.has_header { 1 } else { 0 };
                for row_str in engine.get_rows(start, usize::MAX) { 
                    let mut fields = Vec::new();
                    let row_bytes = row_str.as_bytes();
                    let mut col_idx = 0;
                    while let Some(field) = engine.extract_field(row_bytes, col_idx) {
                        let mut s = std::str::from_utf8(field).unwrap_or("").trim().to_string();
                        if s.starts_with('"') && s.ends_with('"') { s = s[1..s.len()-1].to_string(); }
                        fields.push(s); col_idx += 1;
                    }
                    if pos < fields.len() {
                        let key = fields.swap_remove(pos);
                        let mut normalized = vec![key]; normalized.extend(fields);
                        right_rows.push(normalized); 
                    }
                }
            }
        }
    }
    if !right_found { return Err(anyhow!("Right col '{}' not found", right_col_name)); }

    let left_table = JoinTable::new(left_rows, 0);
    let right_table = JoinTable::new(right_rows, 0);
    HashJoin::new(left_col_name, right_col_name, 64).with_join_type(join_type).execute_materialized(&left_table, &right_table)
}

fn merge_column_stats(base: &mut ColumnStats, incoming: &ColumnStats) {
    if base.name.is_empty() && !incoming.name.is_empty() { base.name = incoming.name.clone(); }
    if base.is_categorical || incoming.is_categorical {
        base.count += incoming.count;
        let mut cat_map: std::collections::HashMap<String, u64> = base.top_categories.iter().cloned().collect();
        for (k, v) in &incoming.top_categories { *cat_map.entry(k.clone()).or_insert(0) += v; }
        let mut merged: Vec<_> = cat_map.into_iter().collect();
        merged.sort_by(|a, b| b.1.cmp(&a.1)); merged.truncate(10);
        base.top_categories = merged; base.is_categorical = true; return;
    }
    let n1 = base.count as f64; let n2 = incoming.count as f64;
    if n2 == 0.0 { return; }
    let total_n = n1 + n2;
    let new_mean = if total_n > 0.0 { (base.mean * n1 + incoming.mean * n2) / total_n } else { 0.0 };
    let d1 = base.mean - new_mean; let d2 = incoming.mean - new_mean;
    let var1 = base.std_dev * base.std_dev; let var2 = incoming.std_dev * incoming.std_dev;
    let new_variance = if total_n > 0.0 { (n1 * (var1 + d1 * d1) + n2 * (var2 + d2 * d2)) / total_n } else { 0.0 };
    base.mean = new_mean; base.std_dev = new_variance.sqrt();
    base.sum += incoming.sum; base.count += incoming.count;
    if incoming.min < base.min { base.min = incoming.min; }
    if incoming.max > base.max { base.max = incoming.max; }
    if base.histogram.len() == incoming.histogram.len() {
        for (b, &inc) in base.histogram.iter_mut().zip(incoming.histogram.iter()) { *b += inc; }
    }
    base.q1 = (base.q1 * n1 + incoming.q1 * n2) / total_n;
    base.median = (base.median * n1 + incoming.median * n2) / total_n;
    base.q3 = (base.q3 * n1 + incoming.q3 * n2) / total_n;
}
