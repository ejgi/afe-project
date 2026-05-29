use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DataType {
    Numeric, Integer, Currency, Date, Category, Percentage, Boolean, IP, MAC, ID, Email, URL, UUID, PhoneNumber, JSON,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: DataType,
    pub format: Option<String>,
    pub currency_symbol: Option<String>,
}

impl ColumnSchema {
    pub fn new(name: &str, data_type: DataType) -> Self {
        Self { name: name.to_string(), data_type, format: None, currency_symbol: None }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CsvMeta {
    pub columns: std::collections::HashMap<String, ColumnSchema>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ColumnStats {
    pub name: String,
    pub mean: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
    pub sum: f64,
    pub count: u64,
    pub null_count: u64,
    pub distinct_count: u64,
    pub variance: f64,
    pub skewness: f64,
    pub kurtosis: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub histogram: Vec<u64>,
    pub schema: Option<ColumnSchema>,
    pub has_range_violation: bool,
    pub top_categories: Vec<(String, u64)>,
    pub is_categorical: bool,
    pub estimated_memory_kb: f64,
    pub filling_ratio: f64,
    pub unique_ratio: f64,
    pub health_score: f64,
    pub integrity_warnings: Vec<String>,
    pub is_constant: bool,
    pub is_monotonic_inc: bool,
    pub is_monotonic_dec: bool,
}
