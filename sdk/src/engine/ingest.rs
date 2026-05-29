use std::sync::mpsc::{Sender, Receiver, channel};
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use std::path::PathBuf;
use std::fs::{File, OpenOptions};
use std::io::{Write, BufWriter, Result as IoResult};
use anyhow::Result;
use rayon::prelude::*;
use log;
use memmap2::Mmap;

use crate::engine::BigDataEngine;
use crate::engine::OffsetIndex;

use crate::compute::dma::get_dma;
use crate::compute::dma::provider::PinnedBuffer;

/// A command sent to the high-performance Ingestion Orchestrator.
pub enum IngestCmd {
    /// Append a batch of rows to the current ingestion buffer.
    AppendBatch(Vec<String>),
    /// Force a flush of the current buffer to disk and GPU.
    Commit,
    /// Stop the orchestrator and wait for final flush.
    Shutdown,
}

/// Statistics for the current ingestion session.
#[derive(Default)]
pub struct IngestStats {
    pub rows_ingested: AtomicU64,
    pub bytes_written: AtomicU64,
    pub batches_processed: AtomicU64,
}

/// High-Performance Actor-based Ingestion Orchestrator.
/// Coordinates the concurrent flow of data from raw strings to
/// Disk (Storage Layer) and VRAM (GPU Layer) using ZenDMA.
pub struct IngestOrchestrator {
    cmd_tx: Sender<IngestCmd>,
    stats: Arc<IngestStats>,
    _join_handle: Option<thread::JoinHandle<()>>,
}

impl IngestOrchestrator {
    /// Starts the ingestion orchestrator for a given target path.
    pub fn start(target_path: PathBuf) -> Result<Self> {
        // Force DMA initialization on main thread to catch crashes
        let _ = get_dma();
        
        let (tx, rx) = channel();
        let stats = Arc::new(IngestStats::default());
        let stats_clone = Arc::clone(&stats);

        let handle = thread::spawn(move || {
            if let Err(e) = Self::orchestrator_loop(rx, target_path, stats_clone) {
                log::error!("[Ingest] Orchestrator loop crashed: {:?}", e);
            }
        });

        Ok(Self {
            cmd_tx: tx,
            stats,
            _join_handle: Some(handle),
        })
    }

    /// Appends a single row to the ingestion pipeline.
    pub fn append(&self, row: String) -> Result<()> {
        self.cmd_tx.send(IngestCmd::AppendBatch(vec![row]))?;
        Ok(())
    }

    /// Appends multiple rows in a single command for better efficiency.
    pub fn append_batch(&self, rows: Vec<String>) -> Result<()> {
        self.cmd_tx.send(IngestCmd::AppendBatch(rows))?;
        Ok(())
    }

    /// Triggers a manual commit/flush.
    pub fn commit(&self) -> Result<()> {
        self.cmd_tx.send(IngestCmd::Commit)?;
        Ok(())
    }

    /// Gets current throughput/progress stats.
    pub fn get_progress(&self) -> (u64, u64) {
        (
            self.stats.rows_ingested.load(Ordering::Relaxed),
            self.stats.bytes_written.load(Ordering::Relaxed),
        )
    }

    /// Main loop for the ingestion actor.
    fn orchestrator_loop(
        rx: Receiver<IngestCmd>,
        target_path: PathBuf,
        stats: Arc<IngestStats>,
    ) -> Result<()> {
        log::info!("[Ingest] Starting Nitro Ingestion Orchestrator for {:?}", target_path);

        // ZenDMA pipeline configuration
        let dma = get_dma();
        let mut pinned_buffer = dma.memory.allocate_pinned(16 * 1024 * 1024)?; // 16MB Pinned Staging Buffer
        let mut string_buffer: Vec<String> = Vec::with_capacity(50_000);
        
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&target_path)?;
        let mut writer = BufWriter::new(file);

        while let Ok(cmd) = rx.recv() {
            match cmd {
                IngestCmd::AppendBatch(rows) => {
                    string_buffer.extend(rows);
                    
                    if string_buffer.len() >= 50_000 {
                        Self::process_and_flush(
                            &mut string_buffer,
                            &mut pinned_buffer,
                            &mut writer,
                            &stats
                        )?;
                    }
                }
                IngestCmd::Commit => {
                    Self::process_and_flush(
                        &mut string_buffer,
                        &mut pinned_buffer,
                        &mut writer,
                        &stats
                    )?;
                }
                IngestCmd::Shutdown => break,
            }
        }

        // Final cleanup
        Self::process_and_flush(&mut string_buffer, &mut pinned_buffer, &mut writer, &stats)?;
        writer.flush()?;
        log::info!("[Ingest] Orchestrator for {:?} shutdown cleanly.", target_path);
        Ok(())
    }

    /// Processes the batch of strings, serializes them into the pinned buffer,
    /// and flushes to both Disk and GPU (via ZenDMA).
    fn process_and_flush(
        rows: &mut Vec<String>,
        pinned: &mut Box<dyn PinnedBuffer>,
        writer: &mut BufWriter<std::fs::File>,
        stats: &Arc<IngestStats>,
    ) -> Result<()> {
        if rows.is_empty() { return Ok(()); }
        
        let start = Instant::now();
        let row_count = rows.len() as u64;

        // 1. Parallel Serialization (Rayon)
        // Convert rows to a single large byte buffer
        let serialized_data: Vec<u8> = rows
            .par_iter()
            .map(|s| {
                let mut b = s.as_bytes().to_vec();
                b.push(b'\n'); // Standard line delimiter
                b
            })
            .flatten()
            .collect();

        let data_len = serialized_data.len();

        // 2. Storage Write (Native I/O via writer)
        writer.write_all(&serialized_data)?;
        
        // 3. GPU/DMA Transfer (Optional/Ready-for-Shader)
        // In a real Nitro scenario, we'd use the DMA orchestrator to ship
        // to a wgpu::Buffer for immediate live visualization or filtering.
        // We copy to pinned memory first for zero-copy upload.
        let pinned_slice = unsafe { 
            std::slice::from_raw_parts_mut(pinned.as_mut_ptr(), pinned.len()) 
        };
        
        if data_len <= pinned.len() {
            pinned_slice[..data_len].copy_from_slice(&serialized_data);
            // dma.gpu.upload(pinned.as_mut(), ...) would be called here if a buffer was tied.
        }

        // Update stats
        stats.rows_ingested.fetch_add(row_count, Ordering::SeqCst);
        stats.bytes_written.fetch_add(data_len as u64, Ordering::SeqCst);
        stats.batches_processed.fetch_add(1, Ordering::SeqCst);

        rows.clear();
        log::debug!(
            "[Ingest] Batch flush: {} rows, {} bytes in {:?}", 
            row_count, data_len, start.elapsed()
        );

        Ok(())
    }
}

// BigDataEngine Ingestion Methods
pub(crate) fn build_index_impl(engine: &mut BigDataEngine) -> IoResult<()> {
    let mut idx_path = engine.path.clone();
    if let Some(mut file_name) = idx_path.file_name().map(|n| n.to_os_string()) {
        file_name.push(".zendx");
        idx_path.set_file_name(file_name);
    } else {
        idx_path = engine.path.with_extension("zendx");
    }
    
    let data = &engine.mmap;
    let file_size = data.len() as u64;

    let total_header_offset;
    
    // Skip UTF-8 BOM if present (\xEF\xBB\xBF)
    let mut data_start = 0;
    if data.len() >= 3 && &data[0..3] == b"\xEF\xBB\xBF" {
        data_start = 3;
    }

    // Skip preambleRows
    let mut preamble_offset = data_start;
    for _ in 0..engine.skip_rows {
        while preamble_offset < data.len() && data[preamble_offset] != b'\n' {
            preamble_offset += 1;
        }
        if preamble_offset < data.len() {
            preamble_offset += 1;
        }
    }

    if engine.has_header {
        let mut header_end = preamble_offset;
        while header_end < data.len() && data[header_end] != b'\n' {
            header_end += 1;
        }
        total_header_offset = (header_end + 1).min(data.len());
    } else {
        total_header_offset = preamble_offset;
    }

    let should_cache_to_disk = file_size >= 50 * 1024 * 1024; // Only disk-cache > 50MB files

    if should_cache_to_disk && idx_path.exists() {
        if let Ok(file) = File::open(&idx_path) {
            if let Ok(m) = unsafe { Mmap::map(&file) } {
                if m.len() >= 16 {
                    let cached_size = u64::from_le_bytes(m[0..8].try_into().unwrap());
                    if cached_size == file_size {
                        if &m[8..16] == b"ZENFOREN" {
                            let off_count = u64::from_le_bytes(m[16..24].try_into().unwrap()) as usize;
                            let off_end = 24 + off_count * 8;
                            if m.len() >= off_end + 8 {
                                engine.offsets = OffsetIndex::Mmapped { 
                                    mmap: m, 
                                    start_offset: 24, 
                                    count: off_count 
                                };
                                
                                let m_ref = match &engine.offsets {
                                    OffsetIndex::Mmapped { mmap, .. } => mmap,
                                    _ => unreachable!(),
                                };
                                
                                let hash_count_pos = off_end;
                                let hash_count = u64::from_le_bytes(m_ref[hash_count_pos..hash_count_pos+8].try_into().unwrap()) as usize;
                                let mut hashes = Vec::with_capacity(hash_count);
                                let mut cursor = hash_count_pos + 8;
                                for _ in 0..hash_count {
                                    if cursor + 40 <= m_ref.len() {
                                        let mut h = [0u8; 32];
                                        h.copy_from_slice(&m_ref[cursor..cursor+32]);
                                        let size = u64::from_le_bytes(m_ref[cursor+32..cursor+40].try_into().unwrap()) as usize;
                                        hashes.push((h, size));
                                        cursor += 40;
                                    }
                                }
                                engine.block_hashes = hashes;
                                return Ok(());
                            }
                        } else if m.len() % 8 == 0 {
                            let count = (m.len() - 8) / 8;
                            engine.offsets = OffsetIndex::Mmapped { 
                                mmap: m, 
                                start_offset: 8, 
                                count 
                            };
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    let body_data = &data[total_header_offset as usize..];
    let chunk_size = 64 * 1024 * 1024;
    let mut chunk_starts = Vec::new();
    let mut cursor = 0;
    while cursor < body_data.len() {
        chunk_starts.push(cursor);
        cursor += chunk_size;
        if cursor < body_data.len() {
            while cursor < body_data.len() && body_data[cursor] != b'\n' {
                cursor += 1;
            }
            cursor += 1;
        }
    }

    let is_mech = match engine.hardware_mode {
        crate::types::HardwareMode::Auto => crate::utils::is_rotational(&engine.path),
        crate::types::HardwareMode::HDD => true,
        crate::types::HardwareMode::SSD => false,
    };
    
    let num_threads = if is_mech { 
        2 
    } else { 
        if body_data.len() > 1_000_000_000 { 
            8.min(rayon::current_num_threads()) 
        } else { 
            rayon::current_num_threads() 
        }
    };
    
    let chunk_results: Vec<(Vec<u64>, ([u8; 32], usize))> = if is_mech {
        chunk_starts.into_iter().map(|start_pos| {
            let chunk_data = if start_pos + chunk_size < body_data.len() {
                let mut end = start_pos + chunk_size;
                while end < body_data.len() && body_data[end] != b'\n' { end += 1; }
                &body_data[start_pos..end]
            } else {
                &body_data[start_pos..]
            };

            let mut hasher = blake3::Hasher::new();
            hasher.update(chunk_data);
            let block_hash: [u8; 32] = hasher.finalize().into();

            let estimated_rows = chunk_data.len() / 100;
            let mut offsets: Vec<u64> = Vec::with_capacity(estimated_rows);
            let base_offset = (total_header_offset + start_pos) as u64;
            if start_pos == 0 { offsets.push(base_offset); }
            for (i, &b) in chunk_data.iter().enumerate() {
                if b == b'\n' {
                    let next_line_start = base_offset + (i + 1) as u64;
                    if (i + 1) < chunk_data.len() {
                        offsets.push(next_line_start);
                    }
                }
            }
            (offsets, (block_hash, chunk_data.len()))
        }).collect()
    } else {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(num_threads).build().unwrap();
        pool.install(|| {
            chunk_starts.into_par_iter().map(|start_pos| {
                let chunk_data = if start_pos + chunk_size < body_data.len() {
                    let mut end = start_pos + chunk_size;
                    while end < body_data.len() && body_data[end] != b'\n' { end += 1; }
                    &body_data[start_pos..end]
                } else {
                    &body_data[start_pos..]
                };

                let mut hasher = blake3::Hasher::new();
                hasher.update(chunk_data);
                let block_hash: [u8; 32] = hasher.finalize().into();

                let estimated_rows = chunk_data.len() / 100;
                let mut offsets: Vec<u64> = Vec::with_capacity(estimated_rows);
                let base_offset = (total_header_offset + start_pos) as u64;
                if start_pos == 0 { offsets.push(base_offset); }
                for (i, &b) in chunk_data.iter().enumerate() {
                    if b == b'\n' {
                        let next_line_start = base_offset + (i + 1) as u64;
                        if (i + 1) < chunk_data.len() {
                            offsets.push(next_line_start);
                        }
                    }
                }
                (offsets, (block_hash, chunk_data.len()))
            }).collect()
        })
    };

    let mut all_offsets = Vec::new();
    let mut final_hashes = Vec::new();
    for (offsets, hash_entry) in chunk_results {
        all_offsets.extend(offsets);
        final_hashes.push(hash_entry);
    }
    engine.block_hashes = final_hashes;

    let mut out_data = Vec::with_capacity(8 + all_offsets.len() * 8 + engine.block_hashes.len() * 40);
    out_data.extend_from_slice(&file_size.to_le_bytes());
    out_data.extend_from_slice(b"ZENFOREN"); 
    out_data.extend_from_slice(&(all_offsets.len() as u64).to_le_bytes());
    
    for &off in &all_offsets {
        out_data.extend_from_slice(&off.to_le_bytes());
    }
    
    out_data.extend_from_slice(&(engine.block_hashes.len() as u64).to_le_bytes());
    for (hash, size) in &engine.block_hashes {
        out_data.extend_from_slice(hash);
        out_data.extend_from_slice(&(*size as u64).to_le_bytes());
    }

    if should_cache_to_disk {
        let _ = std::fs::write(&idx_path, out_data);
    }
    
    engine.offsets = OffsetIndex::Memory(all_offsets);
    Ok(())
}
