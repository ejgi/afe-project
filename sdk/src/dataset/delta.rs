use std::path::PathBuf;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write, BufReader, BufWriter};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use anyhow::Result;

/// Direct command for mutability
#[derive(Debug, PartialEq, Eq)]
pub enum DeltaOp {
    /// Row index, new row content (serialized)
    Update(u64, String),
    /// Row index to delete
    Delete(u64),        
}

/// Magic signature for `.zen_delta` binary files
const ZEN_DELTA_MAGIC: &[u8; 4] = b"ZEND";
/// Current version of the delta format
const ZEN_DELTA_VERSION: u16 = 1;

/// The Delta Manager handles sidecar files that store modifications to immutable shards.
/// By storing only the changes (deltas), we can update or delete rows in a 1 TB file
/// in milliseconds, without having to rewrite the entire 1 TB file.
pub struct DeltaManager {
    /// Path to the original base dataset (e.g. data.csv)
    pub base_path: PathBuf,
    /// Path to the sidecar delta file (e.g. data.zen_delta)
    pub delta_path: PathBuf,
    /// Bitmap for deleted rows. A bit set to 1 means row is deleted.
    pub tombstone_mask: Vec<u8>,
    /// In-memory map for modified rows (Row Index -> New Serialized Data)
    pub updates: HashMap<u64, String>,
}

impl DeltaManager {
    /// Initializes a DeltaManager for a given dataset file.
    /// If a `.zen_delta` file already exists, it is automatically loaded.
    pub fn new(base_path: PathBuf) -> Result<Self> {
        let mut delta_path = base_path.clone();
        delta_path.set_extension("zen_delta");
        
        let mut manager = Self {
            base_path,
            delta_path,
            tombstone_mask: Vec::new(),
            updates: HashMap::new(),
        };

        if manager.delta_path.exists() {
            manager.load()?;
        }

        Ok(manager)
    }

    /// Mark a row as deleted.
    pub fn delete_row(&mut self, row_idx: u64) {
        let byte_idx = (row_idx / 8) as usize;
        let bit_idx = (row_idx % 8) as u8;
        
        if byte_idx >= self.tombstone_mask.len() {
            self.tombstone_mask.resize(byte_idx + 1, 0);
        }
        self.tombstone_mask[byte_idx] |= 1 << bit_idx;
        
        // If it was previously updated, remove the update as it's now deleted.
        self.updates.remove(&row_idx);
    }

    /// Check if a row is deleted.
    pub fn is_deleted(&self, row_idx: u64) -> bool {
        let byte_idx = (row_idx / 8) as usize;
        let bit_idx = (row_idx % 8) as u8;
        
        if byte_idx < self.tombstone_mask.len() {
            (self.tombstone_mask[byte_idx] & (1 << bit_idx)) != 0
        } else {
            false
        }
    }

    /// Stage an update for a row. Automatically un-marks deletion if it was deleted.
    pub fn update_row(&mut self, row_idx: u64, content: String) {
        // Un-delete if it was previously marked as deleted
        let byte_idx = (row_idx / 8) as usize;
        let bit_idx = (row_idx % 8) as u8;
        if byte_idx < self.tombstone_mask.len() {
            self.tombstone_mask[byte_idx] &= !(1 << bit_idx);
        }

        self.updates.insert(row_idx, content);
    }

    /// Retrieve an updated row, if it exists.
    pub fn get_update(&self, row_idx: u64) -> Option<&String> {
        self.updates.get(&row_idx)
    }

    /// Persist the delta file to disk in a compact binary format.
    /// Format:
    ///   [Magic Bytes: 4][Version: 2]
    ///   [Tombstone Mask Len: 8][Mask Bytes...]
    ///   [Updates Count: 8]
    ///     [Row Idx: 8][Str Len: 4][Str Bytes...] * Updates Count
    pub fn persist(&self) -> Result<()> {
        log::info!("Persisting delta changes to {:?}", self.delta_path);
        
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.delta_path)?;
            
        let mut writer = BufWriter::new(file);

        // Header
        writer.write_all(ZEN_DELTA_MAGIC)?;
        writer.write_u16::<LittleEndian>(ZEN_DELTA_VERSION)?;

        // Tombstones
        writer.write_u64::<LittleEndian>(self.tombstone_mask.len() as u64)?;
        writer.write_all(&self.tombstone_mask)?;

        // Updates
        writer.write_u64::<LittleEndian>(self.updates.len() as u64)?;
        for (idx, content) in &self.updates {
            writer.write_u64::<LittleEndian>(*idx)?;
            let bytes = content.as_bytes();
            writer.write_u32::<LittleEndian>(bytes.len() as u32)?;
            writer.write_all(bytes)?;
        }

        writer.flush()?;
        Ok(())
    }

    /// Load existing delta changes from disk.
    fn load(&mut self) -> Result<()> {
        log::info!("Loading existing deltas from {:?}", self.delta_path);
        
        let file = File::open(&self.delta_path)?;
        let mut reader = BufReader::new(file);

        // Header check
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != ZEN_DELTA_MAGIC {
            return Err(anyhow::anyhow!("Invalid delta file magic signature"));
        }

        let version = reader.read_u16::<LittleEndian>()?;
        if version != ZEN_DELTA_VERSION {
            return Err(anyhow::anyhow!("Unsupported delta format version: {}", version));
        }

        // Read Tombstones
        let mask_len = reader.read_u64::<LittleEndian>()? as usize;
        self.tombstone_mask = vec![0u8; mask_len];
        if mask_len > 0 {
            reader.read_exact(&mut self.tombstone_mask)?;
        }

        // Read Updates
        let updates_count = reader.read_u64::<LittleEndian>()?;
        self.updates.clear();
        for _ in 0..updates_count {
            let row_idx = reader.read_u64::<LittleEndian>()?;
            let str_len = reader.read_u32::<LittleEndian>()? as usize;
            
            let mut str_bytes = vec![0u8; str_len];
            reader.read_exact(&mut str_bytes)?;
            
            let content = String::from_utf8(str_bytes)?;
            self.updates.insert(row_idx, content);
        }

        Ok(())
    }
    
    /// Compaction Phase: Applies all deltas to the original file to create
    /// a new, clean immutable base file, then deletes the delta file.
    /// This should be run as a background maintenance task when deltas get too large.
    pub fn compact(&self, _output_path: &std::path::Path) -> Result<()> {
        // TODO: Read base file line by line
        // Skip lines that are in tombstone_mask
        // Replace lines that exist in updates map
        // Write to output_path
        unimplemented!("Compaction to be implemented as a background Job");
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn test_delta_lifecycle() -> Result<()> {
        let mut base_path = env::temp_dir();
        base_path.push("test_dataset.csv");
        
        // 1. Create and modify
        let mut delta = DeltaManager::new(base_path.clone())?;
        
        delta.delete_row(5);
        delta.delete_row(100);
        delta.update_row(42, "updated_content_42".to_string());
        
        assert!(delta.is_deleted(5));
        assert!(delta.is_deleted(100));
        assert!(!delta.is_deleted(42));
        assert_eq!(delta.get_update(42).unwrap(), "updated_content_42");
        
        // Persist to disk
        delta.persist()?;
        assert!(delta.delta_path.exists());

        // 2. Load from disk into new manager
        let loaded_delta = DeltaManager::new(base_path.clone())?;
        
        assert!(loaded_delta.is_deleted(5));
        assert!(loaded_delta.is_deleted(100));
        assert!(!loaded_delta.is_deleted(42));
        assert_eq!(loaded_delta.get_update(42).unwrap(), "updated_content_42");
        assert!(loaded_delta.get_update(5).is_none());

        // Cleanup
        let _ = fs::remove_file(delta.delta_path);
        
        Ok(())
    }
}
