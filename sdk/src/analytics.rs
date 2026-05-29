pub mod join;
pub mod simd;
pub mod zenscan;
pub mod csv_parser;
pub mod hybrid;
pub mod ioc;

use std::sync::atomic::{AtomicU64, Ordering};
use crate::accumulator::ColumnAccumulator;
use fxhash::FxHashMap;

pub enum ParChunk<'a> {
    Offsets(&'a [u64], usize),
    Bytes(usize, usize)
}

pub struct ChunkResult {
    pub row_count: usize,
}

pub fn flush_categorical_cache_to_accs(cache: &mut FxHashMap<&[u8], u64>, acc: &ColumnAccumulator) {
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
