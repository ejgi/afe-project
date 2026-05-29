/// SPDK Storage Backend (Maximum Performance)
/// Uses Intel's Storage Performance Development Kit for user-space NVMe access.
/// Bypasses the Linux kernel VFS and block layer entirely.
/// Requires root/CAP_SYS_ADMIN to unbind the NVMe driver from the kernel.
///
/// Dependencies: spdk-rs (https://github.com/OpenEBS/spdk-rs)

use anyhow::{Result, anyhow};
use crate::compute::dma::provider::{StorageProvider, StorageSource};

pub struct SpdkStorageBackend;

impl SpdkStorageBackend {
    /// Probe for SPDK availability.
    /// SPDK requires:
    ///   1. The NVMe device to be unbound from the kernel nvme driver.
    ///   2. A valid 2MB HugePages allocation.
    ///   3. CAP_SYS_ADMIN or root privileges.
    fn probe() -> bool {
        // A real probe would call spdk_env_opts_init() and spdk_env_init().
        // We simulate this by checking for the presence of /dev/hugepages and
        // verifying we have the correct privilege level.
        #[cfg(target_os = "linux")]
        {
            let hugepages_available = std::path::Path::new("/dev/hugepages").exists();
            // Check for root (UID 0) or CAP_SYS_ADMIN
            // In practice, use the `caps` crate for a proper check.
            let has_privileges = unsafe { libc::getuid() == 0 };
            return hugepages_available && has_privileges;
        }
        #[cfg(not(target_os = "linux"))]
        {
            return false;
        }
    }
}

impl StorageProvider for SpdkStorageBackend {
    fn name(&self) -> &'static str {
        "spdk-nvme"
    }

    fn is_available(&self) -> bool {
        Self::probe()
    }

    fn read_into(&self, source: &StorageSource, _byte_offset: u64, buf: &mut [u8]) -> Result<usize> {
        let (device_path, lba_offset) = match source {
            StorageSource::RawDevice(p, lba) => (p, lba),
            StorageSource::File(p) => {
                // SPDK can't address files; it needs a raw device.
                return Err(anyhow!(
                    "[ZenDMA] SPDK requires a RawDevice source, not a file path: {:?}. \
                     Use SpdkStorageSource::RawDevice(\"/dev/nvme0n1\", lba) instead.",
                    p
                ));
            }
        };

        // TODO: Integrate spdk-rs bindings here.
        // 1. spdk_nvme_probe() → discover and attach to the NVMe controller.
        // 2. spdk_nvme_ns_cmd_read() → submit a read command.
        // 3. spdk_nvme_qpair_process_completions() → busy-poll until done.
        log::info!(
            "[ZenDMA] SPDK: Reading {} bytes from {:?} @ LBA {}",
            buf.len(), device_path, lba_offset
        );

        Err(anyhow!("[ZenDMA] SPDK backend: spdk-rs bindings not yet linked. Implement via FFI."))
    }
}
