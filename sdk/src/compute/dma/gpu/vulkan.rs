/// Vulkan GPU Transfer Backend (Maximum Performance — Zero Copy)
/// Uses VK_EXT_external_memory_host to import pinned host memory directly
/// into Vulkan GPU memory, enabling a true zero-copy DMA transfer.
///
/// Supported by: NVIDIA (since 2020), AMD/RADV (since 2018), Intel/ANV (since 2018).
///
/// This eliminates the extra memcpy that the wgpu-standard backend requires.

use anyhow::{Result, anyhow};
use crate::compute::dma::provider::{GpuTransferProvider, PinnedBuffer};

pub struct VulkanTransferBackend;

impl VulkanTransferBackend {
    /// Probe for VK_EXT_external_memory_host support.
    /// Real implementation uses ash::Instance::enumerate_device_extension_properties().
    fn probe() -> bool {
        // Supported by NVIDIA (2020+), AMD RADV (2018+), Intel ANV (2018+) on Linux.
        // We conservatively return false until ash bindings are linked.
        #[cfg(target_os = "linux")] { false } // TODO: Probe via ash Vulkan bindings
        #[cfg(not(target_os = "linux"))] { false }
    }
}

impl GpuTransferProvider for VulkanTransferBackend {
    fn name(&self) -> &'static str {
        "vulkan-external-memory-host"
    }

    fn is_available(&self) -> bool {
        Self::probe()
    }

    fn upload(
        &self,
        pinned_buf: &mut dyn PinnedBuffer,
        _gpu_buffer: &wgpu::Buffer,
        _queue: &wgpu::Queue,
    ) -> Result<()> {
        log::warn!(
            "[ZenDMA/GPU] vulkan-external-memory-host: ash Vulkan bindings not yet linked. \
             {} bytes of pinned memory ready for zero-copy import.",
            pinned_buf.len(),
        );
        Err(anyhow!("Vulkan VK_EXT_external_memory_host: ash bindings TODO"))
    }
}
