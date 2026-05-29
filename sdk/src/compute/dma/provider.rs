/// The universal contract for ZenDMA.
/// Any storage backend, memory allocator, or GPU transfer layer
/// must implement its corresponding sub-trait here.
/// This ensures the Zen Engine core never depends on a specific vendor.

use std::path::Path;
use anyhow::Result;

// ---------------------------------------------------------------------------
// Tier 1: Storage Provider
// Responsible for reading raw bytes from an NVMe/disk into a host buffer.
// ---------------------------------------------------------------------------

/// Describes the source of data for a DMA read.
pub enum StorageSource<'a> {
    /// A standard file path (used by io_uring and native backends).
    File(&'a Path),
    /// A raw NVMe device path (used by the SPDK backend, e.g. "/dev/nvme0n1").
    RawDevice(&'a Path, u64 /* LBA offset */),
}

/// The trait all storage backends must implement.
pub trait StorageProvider: Send + Sync {
    /// Returns a human-readable name for this backend.
    fn name(&self) -> &'static str;

    /// Returns true if the backend is available on this machine.
    fn is_available(&self) -> bool;

    /// Reads `size` bytes from `source` at `byte_offset` into the given buffer.
    /// Implementations MUST be zero-copy where possible.
    fn read_into(&self, source: &StorageSource, byte_offset: u64, buf: &mut [u8]) -> Result<usize>;
}

// ---------------------------------------------------------------------------
// Tier 2: Memory Provider
// Responsible for allocating and pinning host memory for DMA transfers.
// ---------------------------------------------------------------------------

/// A region of pinned (locked) host memory, ready for PCIe DMA.
/// When dropped, the memory MUST be unpinned automatically.
pub trait PinnedBuffer: Send + Sync {
    /// Returns a raw mutable pointer to the start of the pinned region.
    /// Safety: The pointer is valid only while this PinnedBuffer is alive.
    fn as_mut_ptr(&mut self) -> *mut u8;

    /// Returns the data as an immutable byte slice (safe wrapper).
    fn as_slice(&self) -> &[u8];

    /// Returns the size of this buffer in bytes.
    fn len(&self) -> usize;
}

/// The trait all memory backends must implement.
pub trait MemoryProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn is_available(&self) -> bool;

    /// Allocates `size` bytes of pinned (DMA-safe) memory.
    fn allocate_pinned(&self, size: usize) -> Result<Box<dyn PinnedBuffer>>;
}

// ---------------------------------------------------------------------------
// Tier 3: GPU Transfer Provider
// Responsible for moving data from a pinned host buffer to GPU VRAM.
// ---------------------------------------------------------------------------

/// The trait all GPU transfer backends must implement.
pub trait GpuTransferProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn is_available(&self) -> bool;

    /// Uploads data from a pinned host buffer to the GPU for processing.
    fn upload(
        &self,
        pinned_buf: &mut dyn PinnedBuffer,
        gpu_buffer: &wgpu::Buffer,
        queue: &wgpu::Queue,
    ) -> Result<()>;
}
