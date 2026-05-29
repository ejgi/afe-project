use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneStats {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    pub start_row: u64,
    pub end_row: u64,
    pub start_offset: u64,
    pub end_offset: u64,
    pub column_stats: Vec<ZoneStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneMap {
    pub zones: Vec<Zone>,
    pub zone_size: u64,
    #[serde(default)]
    pub num_cols: usize,
}
