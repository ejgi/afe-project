/// GPU Layer Dispatcher
/// Auto-selects the best available GPU transfer strategy at runtime.

pub mod cpu;
pub mod wgpu_upload;
pub mod vulkan;

use crate::compute::dma::provider::GpuTransferProvider;

/// Auto-select the best available GPU transfer backend.
/// Priority: Vulkan (zero-copy) > WGPU Standard (1-copy) > CPU-Only
pub fn best_available() -> Box<dyn GpuTransferProvider> {
    let vk = vulkan::VulkanTransferBackend;
    if vk.is_available() {
        log::info!("[ZenDMA/GPU] Backend: vulkan-external-memory-host 🚀 (zero-copy)");
        return Box::new(vk);
    }

    let wgpu = wgpu_upload::WgpuTransferBackend;
    if wgpu.is_available() {
        log::info!("[ZenDMA/GPU] Backend: wgpu-standard ⚡ (1-copy portable)");
        return Box::new(wgpu);
    }

    log::warn!("[ZenDMA/GPU] Backend: cpu-only 🐢 (no GPU)");
    Box::new(cpu::CpuTransferBackend)
}
