use serde::{Serialize, Deserialize};
use super::schema::{ColumnSchema, ColumnStats};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchResult {
    pub file_name: String,
    pub row_index: usize,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHit {
    pub path: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpFrequency {
    pub ip: String,
    pub count: usize,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub is_noise: bool,
    pub top_files: Vec<FileHit>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct CorrelationPair {
    pub col_a: String,
    pub col_b: String,
    pub value: f64,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct FileMetadata {
    pub file_name: String,
    pub file_size_bytes: u64,
    pub row_count: u64,
    pub duration_ms: u64,
    pub columns: Vec<ColumnSchema>,
    pub column_stats: Vec<ColumnStats>,
    pub segmented_stats: std::collections::HashMap<String, Vec<ColumnStats>>,
    pub correlations: Vec<CorrelationPair>,
    pub block_hashes: Vec<([u8; 32], usize)>,
    pub schema_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupResult {
    pub category: String,
    pub count: u64,
    pub sum: f64,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
}
