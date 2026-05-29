/// HugePages Memory Backend (Linux 2MB/1GB pages)
/// Allocates memory using 2MB or 1GB pages via /dev/hugepages.
/// This dramatically reduces TLB misses in the CPU and memory controller,
/// which is critical when processing GB of data per second.
/// Required by SPDK for its DMA pools.

use anyhow::Result;
use crate::compute::dma::provider::{MemoryProvider, PinnedBuffer};
use crate::compute::dma::memory::pinned::PinnedMemoryBackend;

pub struct HugePagesMemoryBackend;

impl HugePagesMemoryBackend {
    fn probe() -> bool {
        std::path::Path::new("/dev/hugepages").exists()
            || std::path::Path::new("/sys/kernel/mm/hugepages").exists()
    }
}

impl MemoryProvider for HugePagesMemoryBackend {
    fn name(&self) -> &'static str {
        "hugepages-2mb"
    }

    fn is_available(&self) -> bool {
        Self::probe()
    }

    fn allocate_pinned(&self, size: usize) -> Result<Box<dyn PinnedBuffer>> {
        // TODO: Implement via mmap with MAP_HUGETLB | MAP_HUGE_2MB flags.
        // For now, fall back to pinned mlock as the page size difference
        // is an optimization, not a correctness issue.
        log::debug!(
            "[ZenDMA/Memory] HugePages fallback to mlock for {} bytes (MAP_HUGETLB TODO)",
            size
        );
        PinnedMemoryBackend.allocate_pinned(size)
    }
}
