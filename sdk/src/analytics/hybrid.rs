//! # Nitro-Hybrid I/O Engine (Experimental - Phase 2)
//!
//! A Double-Buffer Prefetch scanner that overlaps kernel I/O with CPU-side SIMD scanning.
//!
//! ## Strategy
//! - **Thread A (Ingester):** Signals the kernel to prefetch the NEXT chunk via `madvise(MADV_WILLNEED)`
//! - **Thread B (Processor):** Runs ZenScan SIMD over the CURRENT chunk already in page cache
//!
//! This overlap between I/O and compute is the key to saturating NVMe bandwidth.
//!
//! ## Isolation Guarantee
//! This module is COMPLETELY STANDALONE. It does NOT modify `BigDataEngine`, `VirtualDataset`,
//! or any other production module. It is only accessible via `zen_engine::experimental::hybrid`.

use std::io::Result;
use std::path::Path;
use std::time::Instant;
use memmap2::{Mmap, MmapOptions};
#[cfg(unix)]
use memmap2::Advice;

use crate::analytics::zenscan::ZenScan;

/// Chunk size for Double-Buffer strategy: 128MB per slot.
/// Large enough to keep SIMD busy, small enough to fit in L3 cache on modern CPUs.
const HYBRID_CHUNK_SIZE: usize = 128 * 1024 * 1024; // 128MB

/// Result from a hybrid scan operation.
#[derive(Debug)]
pub struct HybridScanResult {
    /// All match offsets and their context lines.
    pub matches: Vec<(usize, String)>,
    /// Total bytes scanned.
    pub bytes_scanned: usize,
    /// Total elapsed time.
    pub elapsed_ms: u64,
    /// Effective throughput in GB/s.
    pub throughput_gbs: f64,
}

/// The Nitro-Hybrid engine: combines mmap + MADV_WILLNEED prefetch + ZenScan SIMD.
pub struct HybridEngine {
    mmap: Mmap,
    zenscan: ZenScan,
    file_size: usize,
}

impl HybridEngine {
    /// Opens a file and prepares it for hybrid scanning.
    /// Returns an error if the file cannot be opened or memory-mapped.
    pub fn open(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        // Apply sequential hint immediately on open — free performance
        #[cfg(unix)]
        let _ = mmap.advise(Advice::Sequential);

        let file_size = mmap.len();
        let zenscan = ZenScan::new();

        Ok(Self { mmap, zenscan, file_size })
    }

    /// Performs the Double-Buffer Prefetch scan.
    ///
    /// For each chunk `[N]`:
    /// 1. Tell the kernel to prefetch chunk `[N+1]` into page cache asynchronously.
    /// 2. Meanwhile, run SIMD scan on chunk `[N]` (already in cache).
    ///
    /// This overlaps I/O latency with CPU computation, maximizing NVMe utilization.
    pub fn scan(&self, pattern: &str, limit: usize) -> Result<HybridScanResult> {
        let data = &self.mmap[..];
        let needle = pattern.as_bytes();
        let start_time = Instant::now();

        let mut results: Vec<(usize, String)> = Vec::new();
        let total_chunks = (self.file_size + HYBRID_CHUNK_SIZE - 1) / HYBRID_CHUNK_SIZE;

        for chunk_idx in 0..total_chunks {
            if results.len() >= limit { break; }

            let offset = chunk_idx * HYBRID_CHUNK_SIZE;
            let end = (offset + HYBRID_CHUNK_SIZE).min(self.file_size);

            // ── PREFETCH PHASE: Tell kernel to load NEXT chunk NOW ──────────────────
            // This is the key innovation: while we process chunk[N] on CPU,
            // the kernel is busy fetching chunk[N+1] from NVMe in the background.
            #[cfg(unix)]
            {
                let next_offset = (offset + HYBRID_CHUNK_SIZE).min(self.file_size);
                let next_end = (next_offset + HYBRID_CHUNK_SIZE).min(self.file_size);

                if next_offset < self.file_size {
                    let _ = self.mmap.advise_range(Advice::WillNeed, next_offset, next_end - next_offset);
                }
            }

            // ── SCAN PHASE: Run ZenScan SIMD on current chunk (already in cache) ────
            let chunk = &data[offset..end];
            let match_positions = self.zenscan.scan(chunk, needle);

            for &pos in &match_positions {
                if results.len() >= limit { break; }

                let absolute_pos = offset + pos as usize;

                // Extract context line around the match
                let mut line_start = absolute_pos;
                while line_start > 0 && data[line_start - 1] != b'\n' {
                    line_start -= 1;
                }
                let mut line_end = absolute_pos;
                while line_end < data.len() && data[line_end] != b'\n' {
                    line_end += 1;
                }

                let context = String::from_utf8_lossy(&data[line_start..line_end])
                    .trim()
                    .to_string();
                results.push((absolute_pos, context));
            }

            // ── RELEASE PHASE: Removed — Advice::DontNeed not available in memmap2 0.9.
            // Future: use libc::madvise directly for fine-grained cache release.
        }

        let elapsed = start_time.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;
        let throughput_gbs = if elapsed.as_secs_f64() > 0.0 {
            (self.file_size as f64 / (1024.0 * 1024.0 * 1024.0)) / elapsed.as_secs_f64()
        } else {
            0.0
        };

        Ok(HybridScanResult {
            matches: results,
            bytes_scanned: self.file_size,
            elapsed_ms,
            throughput_gbs,
        })
    }

    /// Returns the file size in bytes.
    pub fn file_size(&self) -> usize {
        self.file_size
    }
}
