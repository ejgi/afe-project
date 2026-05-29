/// Memory Layer Dispatcher
/// Auto-selects the best available memory pinning strategy at runtime.

pub mod standard;
pub mod pinned;
pub mod hugepages;

use crate::compute::dma::provider::MemoryProvider;

/// Auto-select the best available memory backend.
/// Priority: HugePages > PinnedMlock > Standard
pub fn best_available() -> Box<dyn MemoryProvider> {
    let hugepages = hugepages::HugePagesMemoryBackend;
    if hugepages.is_available() {
        log::info!("[ZenDMA/Memory] Backend selected: hugepages-2mb 🚀");
        return Box::new(hugepages);
    }

    let pinned = pinned::PinnedMemoryBackend;
    if pinned.is_available() {
        log::info!("[ZenDMA/Memory] Backend selected: pinned-mlock ⚡");
        return Box::new(pinned);
    }

    log::warn!("[ZenDMA/Memory] Backend selected: standard-allocator 🐢 (no DMA guarantee)");
    Box::new(standard::StandardMemoryBackend)
}
