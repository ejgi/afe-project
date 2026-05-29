use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use crate::types::{FileMetadata, ColumnStats};

/// A single recorded scan result.
#[derive(Serialize, Deserialize, Clone)]
pub struct ScanRecord {
    /// Absolute path of the file analyzed
    pub file_path: String,
    /// File size in bytes at scan time
    pub file_size_bytes: u64,
    /// BLAKE3 hash (if computed during scan)
    pub file_hash: Option<String>,
    /// ISO-8601 timestamp of the scan
    pub scanned_at: String,
    /// Optional user tag for easy lookup (e.g. "baseline", "post-migration")
    pub tag: Option<String>,
    /// Short note left by the user
    pub note: Option<String>,
    /// The actual profiling result
    pub metadata: FileMetadata,
}

/// Top-level history database stored at ~/.zen-engine-history.json
#[derive(Serialize, Deserialize, Default)]
pub struct ScanHistory {
    /// Key: unique record ID (auto-generated as "<filename>-<timestamp>")
    pub records: Vec<ScanRecord>,
}

pub struct HistoryManager {
    path: PathBuf,
}

impl HistoryManager {
    pub fn new() -> Self {
        #[allow(deprecated)]
        let mut path = std::env::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".zen-engine-history.json");
        Self { path }
    }

    pub fn load(&self) -> ScanHistory {
        if let Ok(content) = fs::read_to_string(&self.path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            ScanHistory::default()
        }
    }

    pub fn save_db(&self, db: &ScanHistory) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(db)?;
        fs::write(&self.path, content)?;
        Ok(())
    }

    /// Record a new scan result.
    pub fn record(
        &self,
        file_path: &str,
        metadata: &FileMetadata,
        hash: Option<String>,
        tag: Option<String>,
        note: Option<String>,
    ) -> anyhow::Result<String> {
        let mut db = self.load();
        let timestamp = chrono_now();
        let id = format!("{}-{}", sanitize(file_path), &timestamp[..19].replace(':', "-"));
        let record = ScanRecord {
            file_path: file_path.to_string(),
            file_size_bytes: metadata.file_size_bytes,
            file_hash: hash,
            scanned_at: timestamp,
            tag,
            note,
            metadata: metadata.clone(),
        };
        db.records.push(record);
        self.save_db(&db)?;
        Ok(id)
    }

    /// Return all records, optionally filtering by file path.
    pub fn list(&self, path_filter: Option<&str>) -> Vec<ScanRecord> {
        let db = self.load();
        if let Some(pf) = path_filter {
            db.records.into_iter().filter(|r| r.file_path.contains(pf)).collect()
        } else {
            db.records
        }
    }

    /// Find the most recent scan of a given file path.
    pub fn latest_for(&self, file_path: &str) -> Option<ScanRecord> {
        let db = self.load();
        db.records.into_iter().rev().find(|r| r.file_path == file_path)
    }

    /// Delete all records for a specific file.
    pub fn delete_for(&self, file_path: &str) -> anyhow::Result<usize> {
        let mut db = self.load();
        let before = db.records.len();
        db.records.retain(|r| r.file_path != file_path);
        let removed = before - db.records.len();
        self.save_db(&db)?;
        Ok(removed)
    }

    /// Compare two ScanRecords by return a column-level diff.
    pub fn compare(a: &ScanRecord, b: &ScanRecord) -> Vec<ColumnDiff> {
        let a_map: HashMap<String, &ColumnStats> =
            a.metadata.column_stats.iter().map(|s| (s.name.clone(), s)).collect();
        let b_map: HashMap<String, &ColumnStats> =
            b.metadata.column_stats.iter().map(|s| (s.name.clone(), s)).collect();

        let mut diffs = Vec::new();
        let all_cols: std::collections::HashSet<String> =
            a_map.keys().chain(b_map.keys()).cloned().collect();

        for col in all_cols {
            let a_stat = a_map.get(&col);
            let b_stat = b_map.get(&col);
            match (a_stat, b_stat) {
                (Some(a), Some(b)) => diffs.push(ColumnDiff {
                    column: col,
                    status: DiffStatus::Changed,
                    a_count: Some(a.count),
                    b_count: Some(b.count),
                    a_null_pct: Some(null_pct(a)),
                    b_null_pct: Some(null_pct(b)),
                    a_mean: Some(a.mean),
                    b_mean: Some(b.mean),
                    a_distinct: Some(a.distinct_count),
                    b_distinct: Some(b.distinct_count),
                }),
                (Some(a), None) => diffs.push(ColumnDiff {
                    column: col, status: DiffStatus::OnlyInA,
                    a_count: Some(a.count), b_count: None,
                    a_null_pct: Some(null_pct(a)), b_null_pct: None,
                    a_mean: Some(a.mean), b_mean: None,
                    a_distinct: Some(a.distinct_count), b_distinct: None,
                }),
                (None, Some(b)) => diffs.push(ColumnDiff {
                    column: col, status: DiffStatus::OnlyInB,
                    a_count: None, b_count: Some(b.count),
                    a_null_pct: None, b_null_pct: Some(null_pct(b)),
                    a_mean: None, b_mean: Some(b.mean),
                    a_distinct: None, b_distinct: Some(b.distinct_count),
                }),
                (None, None) => {}
            }
        }
        diffs.sort_by(|a, b| a.column.cmp(&b.column));
        diffs
    }
}

#[derive(Debug)]
pub enum DiffStatus { Changed, OnlyInA, OnlyInB }

#[derive(Debug)]
pub struct ColumnDiff {
    pub column: String,
    pub status: DiffStatus,
    pub a_count: Option<u64>,
    pub b_count: Option<u64>,
    pub a_null_pct: Option<f64>,
    pub b_null_pct: Option<f64>,
    pub a_mean: Option<f64>,
    pub b_mean: Option<f64>,
    pub a_distinct: Option<u64>,
    pub b_distinct: Option<u64>,
}

fn null_pct(s: &ColumnStats) -> f64 {
    let total = s.count + s.null_count;
    if total == 0 { 0.0 } else { s.null_count as f64 / total as f64 * 100.0 }
}

fn sanitize(s: &str) -> String {
    std::path::Path::new(s)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Format as YYYY-MM-DDTHH:MM:SSZ (no-std, lightweight)
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    // Days since 1970-01-01
    let (year, month, day) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, h, m, s)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let dy = if is_leap(year) { 366 } else { 365 };
        if days < dy { break; }
        days -= dy;
        year += 1;
    }
    let months = if is_leap(year) {
        [31,29,31,30,31,30,31,31,30,31,30,31]
    } else {
        [31,28,31,30,31,30,31,31,30,31,30,31]
    };
    let mut month = 1u64;
    for dm in months {
        if days < dm { break; }
        days -= dm;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(y: u64) -> bool { (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 }
