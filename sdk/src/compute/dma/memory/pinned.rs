/// Pinned Memory Backend (Linux mlock)
/// Allocates memory and locks it in RAM using mlock(2),
/// preventing the OS from swapping it to disk.
/// Required for safe PCIe DMA when using VK_EXT_external_memory_host.

use anyhow::{Result, anyhow};
use crate::compute::dma::provider::{MemoryProvider, PinnedBuffer};

/// A memory-locked (pinned) buffer. Automatically unpinned on drop.
pub struct PinnedMlockBuffer {
    layout: std::alloc::Layout,
    ptr: *mut u8,
    size: usize,
}

// Safety: The buffer owns the allocated memory and has exclusive access.
unsafe impl Send for PinnedMlockBuffer {}
unsafe impl Sync for PinnedMlockBuffer {}

impl PinnedBuffer for PinnedMlockBuffer {
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr
    }
    fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr as *const u8, self.size) }
    }
    fn len(&self) -> usize {
        self.size
    }
}

impl Drop for PinnedMlockBuffer {
    fn drop(&mut self) {
        unsafe {
            // Unpin the memory before freeing it.
            #[cfg(target_os = "linux")]
            libc::munlock(self.ptr as *const libc::c_void, self.size);
            std::alloc::dealloc(self.ptr, self.layout);
        }
    }
}

pub struct PinnedMemoryBackend;

impl MemoryProvider for PinnedMemoryBackend {
    fn name(&self) -> &'static str {
        "pinned-mlock"
    }

    fn is_available(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            // Test if we can mlock a single page (4KB)
            unsafe {
                let layout = std::alloc::Layout::from_size_align(4096, 4096).unwrap();
                let ptr = std::alloc::alloc(layout);
                if ptr.is_null() { return false; }
                let result = libc::mlock(ptr as *const libc::c_void, 4096);
                if result == 0 {
                    libc::munlock(ptr as *const libc::c_void, 4096);
                }
                std::alloc::dealloc(ptr, layout);
                result == 0
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    fn allocate_pinned(&self, _size: usize) -> Result<Box<dyn PinnedBuffer>> {
        #[cfg(not(target_os = "linux"))]
        return Err(anyhow!("PinnedMemory (mlock) is only supported on Linux."));

        #[cfg(target_os = "linux")]
        unsafe {
            let layout = std::alloc::Layout::from_size_align(_size, 4096)
                .map_err(|e| anyhow!("Layout error: {}", e))?;
            let ptr = std::alloc::alloc_zeroed(layout);
            if ptr.is_null() {
                return Err(anyhow!("Memory allocation failed for {} bytes", _size));
            }
            // Lock memory in RAM — prevents swapping.
            if libc::mlock(ptr as *const libc::c_void, _size) != 0 {
                log::warn!(
                    "[ZenDMA/Memory] mlock failed (limit reached?). Falling back to unpinned memory. \
                     Performance may be degraded. Try 'ulimit -l unlimited'."
                );
            }
            Ok(Box::new(PinnedMlockBuffer { layout, ptr, size: _size }))
        }
    }
}
