/// Native Storage Backend (CPU Fallback)
/// Uses std::fs for reading. Always available, zero extra dependencies.
/// Serves as the safety net when SPDK/io_uring are unavailable.

use anyhow::Result;
use crate::compute::dma::provider::{StorageProvider, StorageSource};

pub struct NativeStorageBackend;

impl StorageProvider for NativeStorageBackend {
    fn name(&self) -> &'static str {
        "native-fs"
    }

    fn is_available(&self) -> bool {
        true // Always available
    }

    fn read_into(&self, source: &StorageSource, byte_offset: u64, buf: &mut [u8]) -> Result<usize> {
        use std::io::{Read, Seek, SeekFrom};

        let path = match source {
            StorageSource::File(p) => p,
            StorageSource::RawDevice(p, _) => p,
        };

        let mut file = std::fs::File::open(path)?;
        file.seek(SeekFrom::Start(byte_offset))?;
        let bytes_read = file.read(buf)?;
        Ok(bytes_read)
    }
}
