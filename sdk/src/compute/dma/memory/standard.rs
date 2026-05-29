/// Standard Memory Backend (CPU Fallback)
/// Uses Rust's standard allocator (Box/Vec). Never fails, but memory
/// is NOT pinned, which means the OS may swap it, causing DMA to fail.
/// Use only when HugePages and mlock are unavailable.

use anyhow::Result;
use crate::compute::dma::provider::{MemoryProvider, PinnedBuffer};

/// A standard (non-pinned) memory buffer. Safe for CPU-only processing.
pub struct StandardBuffer {
    data: Vec<u8>,
}

impl PinnedBuffer for StandardBuffer {
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr()
    }
    fn as_slice(&self) -> &[u8] {
        &self.data
    }
    fn len(&self) -> usize {
        self.data.len()
    }
}

pub struct StandardMemoryBackend;

impl MemoryProvider for StandardMemoryBackend {
    fn name(&self) -> &'static str {
        "standard-allocator"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn allocate_pinned(&self, size: usize) -> Result<Box<dyn PinnedBuffer>> {
        let data = vec![0u8; size];
        Ok(Box::new(StandardBuffer { data }))
    }
}
