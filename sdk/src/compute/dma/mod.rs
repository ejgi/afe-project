/// ZenDMA: The Modular Universal DMA Orchestrator
///
/// Combines the 3 independent layers into a single, coherent Transfer Pipeline:
///   [Storage Backend] → [Memory Backend] → [GPU Transfer Backend]
///
/// At startup, it auto-selects the best available backend for each layer
/// based on the hardware and privileges available on the current machine.
///
/// Architecture:
///   storage/ - Reads from disk (SPDK → io_uring → native)
///   memory/  - Allocates pinned host memory (HugePages → mlock → standard)
///   gpu/     - Uploads to GPU VRAM (Vulkan zero-copy → WGPU → CPU-only)

pub mod provider;
pub mod storage;
pub mod memory;
pub mod gpu;

use std::path::Path;
use std::sync::OnceLock;
use anyhow::Result;

use provider::{StorageProvider, MemoryProvider, GpuTransferProvider, StorageSource};

/// The ZenDMA Engine — a complete, configured 3-layer DMA pipeline.
pub struct ZenDmaEngine {
    pub storage:    Box<dyn StorageProvider>,
    pub memory:     Box<dyn MemoryProvider>,
    pub gpu:        Box<dyn GpuTransferProvider>,
    /// Chunk size in bytes for ring-buffer streaming (default: 256 MB).
    pub chunk_size: usize,
}

impl ZenDmaEngine {
    /// Build the best possible DMA engine for this machine.
    pub fn auto_detect() -> Self {
        let storage = storage::best_available();
        let memory  = memory::best_available();
        let gpu     = gpu::best_available();

        log::info!(
            "[ZenDMA] Pipeline: [{}] → [{}] → [{}] | Chunk: 256 MB",
            storage.name(),
            memory.name(),
            gpu.name(),
        );

        Self {
            storage,
            memory,
            gpu,
            chunk_size: 256 * 1024 * 1024, // 256 MB default chunk
        }
    }

    /// Stream a file through the entire DMA pipeline, calling `on_chunk` for
    /// each uploaded GPU buffer chunk. This is the main entry point for Nitro mode.
    pub fn stream_file(
        &self,
        path: &Path,
        gpu_buffer: &wgpu::Buffer,
        queue: &wgpu::Queue,
        mut on_chunk: impl FnMut(&[u8], u64, usize) -> Result<()>,
    ) -> Result<()> {
        let file_size = std::fs::metadata(path)?.len();
        let mut offset = 0u64;

        while offset < file_size {
            let remaining = (file_size - offset) as usize;
            let current_chunk = remaining.min(self.chunk_size);

            // Allocate a pinned buffer for this chunk
            let mut pinned = self.memory.allocate_pinned(current_chunk)?;

            // Read from disk into pinned memory
            let source = StorageSource::File(path);
            let bytes_read = self.storage.read_into(&source, offset, {
                // Safety: ptr is valid for `current_chunk` bytes.
                unsafe { std::slice::from_raw_parts_mut(pinned.as_mut_ptr(), current_chunk) }
            })?;

            if bytes_read == 0 { break; }

            // Upload pinned memory to GPU VRAM
            self.gpu.upload(&mut *pinned, gpu_buffer, queue)?;

            // Call the user's processing callback
            on_chunk(pinned.as_slice(), offset, bytes_read)?;

            offset += bytes_read as u64;
        }

        Ok(())
    }
}

/// Global singleton for the ZenDMA engine.
static ZEN_DMA: OnceLock<ZenDmaEngine> = OnceLock::new();

/// Get or initialize the global ZenDMA engine.
pub fn get_dma() -> &'static ZenDmaEngine {
    ZEN_DMA.get_or_init(ZenDmaEngine::auto_detect)
}
