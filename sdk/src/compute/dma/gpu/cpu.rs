/// CPU GPU Transfer Backend (Fallback)
/// No GPU required. Processes data directly in CPU RAM.
/// Used when no GPU is available or for testing purposes.

use anyhow::Result;
use crate::compute::dma::provider::{GpuTransferProvider, PinnedBuffer};

pub struct CpuTransferBackend;

impl GpuTransferProvider for CpuTransferBackend {
    fn name(&self) -> &'static str {
        "cpu-only"
    }

    fn is_available(&self) -> bool {
        true // Always available
    }

    fn upload(
        &self,
        _pinned_buf: &mut dyn PinnedBuffer,
        _gpu_buffer: &wgpu::Buffer,
        _queue: &wgpu::Queue,
    ) -> Result<()> {
        // In CPU mode, we do not need a GPU upload.
        // The data stays in the pinned buffer and gets processed by Rayon threads.
        log::debug!("[ZenDMA/GPU] CPU-only mode: data remains in host memory for CPU processing.");
        Ok(())
    }
}
