use crate::engine::BigDataEngine;
use crate::types::*;
use crate::utils::parse_numeric_fast;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::io::Result;
use std::collections::HashMap;

/// Pool de Rayon para modo HDD: 3 hilos fijos, creado una sola vez.
/// Evita el overhead de ~180ms de creación de threads en cada escaneo.
static HDD_SCAN_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

#[inline]
fn hdd_pool() -> &'static rayon::ThreadPool {
    HDD_SCAN_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(3)
            .build()
            .expect("[ZEN] No se pudo crear el pool HDD de 3 hilos")
    })
}

pub(crate) fn infer_schema_impl(
    engine: &BigDataEngine,
    _chunk_size: usize,
    _enable_network: bool,
    skip_rows: usize,
) -> Result<Vec<ColumnSchema>> {
    let data: &[u8] = &engine.mmap;
    let mut data_start = 0;
    for _ in 0..skip_rows {
        while data_start < data.len() && data[data_start] != b'\n' { data_start += 1; }
        if data_start < data.len() { data_start += 1; }
    }
    let mut header_end = data_start;
    if engine.has_header {
        let mut i = data_start;
        while i < data.len() && data[i] != b'\n' { i += 1; }
        header_end = i;
    }
    let headers: Vec<String> = if engine.has_header {
        let s = std::str::from_utf8(&data[data_start..header_end]).unwrap_or("");
        s.split(engine.delimiter as char).map(|s| s.trim().to_string()).collect()
    } else {
        vec!["col_0".to_string()]
    };
    
    let schema = headers.into_iter().map(|name| ColumnSchema {
        name,
        data_type: DataType::Numeric,
        format: None,
        currency_symbol: None,
    }).collect();
    Ok(schema)
}

pub(crate) fn analyze_csv_impl(
    engine: &BigDataEngine,
    options: AnalysisOptions,
    _filter_col: Option<usize>,
    _filter_min: Option<f64>,
    _filter_max: Option<f64>,
    _zone_map: Option<&ZoneMap>,
    _filter_text: Option<&str>,
    _filter_text_col: Option<usize>,
    _filter_ast: Option<&crate::filter::Expr>,
    _date_col: Option<usize>,
    _date_from: Option<u32>,
    _date_to: Option<u32>,
    _progress_tx: Option<&std::sync::mpsc::Sender<FileMetadata>>,
) -> Result<FileMetadata> {
    let start = std::time::Instant::now();
    let data = &engine.mmap;
    
    // Dispatch logic: If PCAP, use the specialized forensic parser
    if engine.path.extension().and_then(|s| s.to_str()) == Some("pcap") {
        log::info!("[Nitro] PCAP detected, switching to forensic parallel engine.");
        let summary = crate::parsers::forensics::pcap::analyze_pcap(data);
        return Ok(FileMetadata {
            file_name: engine.path.display().to_string(),
            file_size_bytes: data.len() as u64,
            row_count: summary.total_packets,
            duration_ms: start.elapsed().as_millis() as u64,
            columns: vec![
                ColumnSchema { name: "Packets".into(), data_type: DataType::Numeric, format: None, currency_symbol: None },
                ColumnSchema { name: "Flows".into(), data_type: DataType::Numeric, format: None, currency_symbol: None },
                ColumnSchema { name: "Suspicious".into(), data_type: DataType::Numeric, format: None, currency_symbol: None },
            ],
            column_stats: Vec::new(),
            segmented_stats: std::collections::HashMap::new(),
            correlations: Vec::new(),
            block_hashes: Vec::new(),
            schema_type: "PCAP".to_string(),
        });
    }

    // Standard CSV/Log analysis (Nitro-Scalar)
    let chunk_size = 100_000;
    let processed_rows = AtomicUsize::new(0);
    
    engine.offsets.par_chunks(chunk_size).for_each(|_chunk| {
        processed_rows.fetch_add(_chunk.len(), Ordering::SeqCst);
    });

    Ok(FileMetadata {
        file_name: engine.path.display().to_string(),
        file_size_bytes: data.len() as u64,
        row_count: processed_rows.load(Ordering::SeqCst) as u64,
        duration_ms: start.elapsed().as_millis() as u64,
        columns: options.blueprint.map(|bp| bp.schemas).unwrap_or_default(),
        column_stats: Vec::new(),
        segmented_stats: std::collections::HashMap::new(),
        correlations: Vec::new(),
        block_hashes: Vec::new(),
        schema_type: "CSV".to_string(),
    })
}

pub(crate) fn group_by_impl(
    engine: &BigDataEngine,
    group_col: usize,
    agg_col: usize,
    filter_ast: Option<&crate::filter::Expr>,
    use_gpu: bool,
) -> Result<Vec<GroupResult>> {
    use fxhash::FxHashMap;
    let data = &engine.mmap;
    let mut selection_bitset = None;
    let mut vectorized_vals = None;

    if use_gpu {
        if let Some(ref gpu) = engine.gpu {
            let rel_offsets: Vec<u32> = engine.offsets.iter().map(|&o| o as u32).collect();
            if let Ok(vec) = gpu.run_vectorize(crate::compute::GpuDataSource::Memory(data), &rel_offsets, agg_col as u32, engine.delimiter) {
                vectorized_vals = Some(vec);
            }
            if let Some(ast) = filter_ast {
                let pattern = match ast {
                    crate::filter::Expr::Compare(_, _, crate::filter::Value::String(s)) => Some(s.as_bytes()),
                    _ => None,
                };
                if let Some(p) = pattern {
                    if let Ok(bitset) = gpu.run_filter_rows(crate::compute::GpuDataSource::Memory(data), &rel_offsets, p) {
                        selection_bitset = Some(bitset);
                    }
                }
            }
        }
    }

    let chunk_size = 500_000.min(engine.offsets.len() / rayon::current_num_threads().max(1) + 1);
    let offsets_slice: &[u64] = &engine.offsets;
    let results: Vec<FxHashMap<String, crate::accumulator::ColumnAccumulator>> = offsets_slice.par_chunks(chunk_size).enumerate().map(|(chunk_idx, offsets_chunk)| {
        let mut groups = FxHashMap::default();
        let chunk_start_row = chunk_idx * chunk_size;

        for (idx, &row_offset) in offsets_chunk.iter().enumerate() {
            let row_idx = chunk_start_row + idx;
            if let Some(ref bitset) = selection_bitset {
                if (bitset[row_idx / 32] & (1 << (row_idx % 32))) == 0 { continue; }
            }

            let s = row_offset as usize;
            let mut e = s;
            while e < data.len() && data[e] != b'\n' { e += 1; }
            let line = &data[s..e];
            
            let mut agg_val = None;
            let mut group_val = String::new();
            if let Some(ref v) = vectorized_vals { agg_val = Some(v[row_idx] as f64); }

            let mut current_col = 0;
            let mut field_start = 0;
            for (j, &byte) in line.iter().enumerate() {
                if byte == engine.delimiter {
                    if current_col == group_col { group_val = String::from_utf8_lossy(&line[field_start..j]).trim().to_string(); }
                    if agg_val.is_none() && current_col == agg_col { agg_val = parse_numeric_fast(&line[field_start..j]); }
                    current_col += 1;
                    field_start = j + 1;
                    if group_val.len() > 0 && agg_val.is_some() { break; }
                }
            }
            if current_col == group_col && group_val.is_empty() { group_val = String::from_utf8_lossy(&line[field_start..]).trim().to_string(); }
            if agg_val.is_none() && current_col == agg_col { agg_val = parse_numeric_fast(&line[field_start..]); }

            if let Some(v) = agg_val {
                if !group_val.is_empty() {
                    groups.entry(group_val).or_insert_with(|| crate::accumulator::ColumnAccumulator::new("group".to_string(), DataType::Numeric)).update(v);
                }
            }
        }
        groups
    }).collect();

    let mut final_groups: FxHashMap<String, crate::accumulator::ColumnAccumulator> = FxHashMap::default();
    for res in results {
        for (k, v) in res {
            final_groups.entry(k).or_insert_with(|| crate::accumulator::ColumnAccumulator::new("group".to_string(), DataType::Numeric)).merge(&v);
        }
    }

    let mut out: Vec<GroupResult> = final_groups.into_iter().map(|(k, v)| {
        let n = v.count.load(Ordering::Relaxed);
        let sum = f64::from_bits(v.sum.load(Ordering::Relaxed));
        GroupResult {
            category: k, count: n, sum, mean: if n > 0 { sum / n as f64 } else { 0.0 },
            min: f64::from_bits(v.min.load(Ordering::Relaxed)),
            max: f64::from_bits(v.max.load(Ordering::Relaxed)),
        }
    }).collect();
    out.sort_by(|a, b| b.count.cmp(&a.count));
    Ok(out)
}

pub(crate) fn top_n_impl(
    engine: &BigDataEngine,
    col: usize,
    n: usize,
    desc: bool,
    _filter_ast: Option<&crate::filter::Expr>,
    _use_gpu: bool,
) -> Result<Vec<String>> {
    use std::collections::BinaryHeap;
    #[derive(PartialEq)]
    struct TopItem { val: f64, offset: u64, desc: bool }
    impl Eq for TopItem {}
    impl PartialOrd for TopItem {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
    }
    impl Ord for TopItem {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            if self.desc { other.val.partial_cmp(&self.val).unwrap_or(std::cmp::Ordering::Equal) }
            else { self.val.partial_cmp(&other.val).unwrap_or(std::cmp::Ordering::Equal) }
        }
    }

    let data = &engine.mmap;
    let chunk_size = 500_000.min(engine.offsets.len() / rayon::current_num_threads().max(1) + 1);
    let offsets_slice: &[u64] = &engine.offsets;
    let final_heap = offsets_slice.par_chunks(chunk_size).map(|chunk| {
        let mut heap = BinaryHeap::with_capacity(n + 1);
        for &offset in chunk {
            let s = offset as usize;
            let mut e = s;
            while e < data.len() && data[e] != b'\n' { e += 1; }
            let line = &data[s..e];
            let mut current_col = 0;
            let mut field_start = 0;
            let mut val_opt = None;
            for (j, &byte) in line.iter().enumerate() {
                if byte == engine.delimiter {
                    if current_col == col { val_opt = parse_numeric_fast(&line[field_start..j]); break; }
                    current_col += 1;
                    field_start = j + 1;
                }
            }
            if val_opt.is_none() && current_col == col { val_opt = parse_numeric_fast(&line[field_start..]); }
            if let Some(val) = val_opt {
                heap.push(TopItem { val, offset, desc });
                if heap.len() > n { heap.pop(); }
            }
        }
        heap
    }).reduce(|| BinaryHeap::new(), |mut a: BinaryHeap<TopItem>, b| {
        for item in b { a.push(item); if a.len() > n { a.pop(); } }
        a
    });

    let sorted = final_heap.into_sorted_vec();
    let mut out = Vec::new();
    for item in sorted {
        let s = item.offset as usize;
        let mut e = s;
        while e < data.len() && data[e] != b'\n' { e += 1; }
        out.push(String::from_utf8_lossy(&data[s..e]).into_owned());
    }
    Ok(out)
}

pub(crate) fn extract_ips_impl(engine: &BigDataEngine, token: Option<&std::sync::atomic::AtomicBool>, mode: crate::types::IpScanMode) -> Result<HashMap<crate::analytics::ioc::IpValue, crate::analytics::ioc::IpMetadata>> {
    use crate::analytics::ioc::{IpScanner, IpMetadata};
    let scanner = IpScanner::new();
    let data = &engine.mmap;
    let chunk_size = 64 * 1024 * 1024;
    let mut chunk_starts = Vec::new();
    let mut cursor = 0;
    while cursor < data.len() {
        chunk_starts.push(cursor);
        cursor += chunk_size;
    }

    let is_mech = match engine.hardware_mode {
        crate::types::HardwareMode::Auto => crate::utils::is_rotational(&engine.path),
        crate::types::HardwareMode::HDD => true,
        crate::types::HardwareMode::SSD => false,
    };

    // Para escaneos completos de archivo con mmap, el paralelismo es siempre óptimo:
    // el OS gestiona la E/S física de forma secuencial independientemente del número de hilos.
    // El modo HDD-secuencial solo aplica a búsquedas con acceso aleatorio indexado.
    // En HDD: chunk_size más grande para reducir overhead de chunks y favorecer prefetch del OS.
    let chunk_size = if is_mech { 128 * 1024 * 1024 } else { 64 * 1024 * 1024 };

    let scan_fn = |chunk_starts: Vec<usize>| {
        chunk_starts.into_par_iter()
            .map(|start| {
                if let Some(t) = token {
                    if t.load(Ordering::Relaxed) { return HashMap::new(); }
                }
                let end = (start + chunk_size).min(data.len());
                let chunk = &data[start..end];
                scanner.extract(chunk, mode)
            })
            .reduce(HashMap::new, |mut a, b| {
                for (ip, meta) in b {
                    let entry = a.entry(ip).or_insert(IpMetadata { hits: 0, noise_hits: 0 });
                    entry.hits += meta.hits;
                    entry.noise_hits += meta.noise_hits;
                }
                a
            })
    };

    let total_map: HashMap<crate::analytics::ioc::IpValue, crate::analytics::ioc::IpMetadata> = if is_mech {
        // HDD: pool estático de 3 hilos — creado una sola vez, reutilizado en todos los scans
        hdd_pool().install(|| scan_fn(chunk_starts))
    } else {
        // SSD/NVMe: paralelismo total con el pool global de Rayon
        scan_fn(chunk_starts)
    };

    Ok(total_map)
}
