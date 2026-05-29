/// WGPU GPU Transfer Backend (Cross-Platform)
/// Uses the wgpu API (Vulkan/Metal/DX12) to transfer data from host RAM
/// to GPU VRAM using a standard write_buffer call.
/// Less efficient than the Vulkan backend (one extra copy), but works
/// on ANY platform supported by wgpu.

use anyhow::Result;
use crate::compute::dma::provider::{GpuTransferProvider, PinnedBuffer};

pub struct WgpuTransferBackend;

impl GpuTransferProvider for WgpuTransferBackend {
    fn name(&self) -> &'static str {
        "wgpu-standard"
    }

    fn is_available(&self) -> bool {
        // wgpu is always compiled in. We assume a GPU is present.
        // A real probe would call wgpu::Instance::enumerate_adapters().
        true
    }

    fn upload(
        &self,
        pinned_buf: &mut dyn PinnedBuffer,
        gpu_buffer: &wgpu::Buffer,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        let data = pinned_buf.as_slice();
        queue.write_buffer(gpu_buffer, 0, data);
        log::debug!(
            "[ZenDMA/GPU] wgpu-standard: uploaded {} bytes to GPU via write_buffer.",
            data.len()
        );
        Ok(())
    }
}
