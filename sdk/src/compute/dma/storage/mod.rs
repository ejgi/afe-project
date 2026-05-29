/// Storage Layer Dispatcher
/// Auto-selects the best available storage backend at runtime.

pub mod native;
pub mod io_uring;
pub mod spdk;

use crate::compute::dma::provider::StorageProvider;

/// Auto-select the best available storage backend.
/// Priority: SPDK > io_uring > native
pub fn best_available() -> Box<dyn StorageProvider> {
    let spdk = spdk::SpdkStorageBackend;
    if spdk.is_available() {
        log::info!("[ZenDMA/Storage] Backend selected: spdk-nvme 🚀");
        return Box::new(spdk);
    }

    let io_uring = io_uring::IoUringStorageBackend;
    if io_uring.is_available() {
        log::info!("[ZenDMA/Storage] Backend selected: io_uring ⚡");
        return Box::new(io_uring);
    }

    log::warn!("[ZenDMA/Storage] Backend selected: native-fs 🐢 (fallback)");
    Box::new(native::NativeStorageBackend)
}
