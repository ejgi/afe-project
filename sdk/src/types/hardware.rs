use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareGpu {
    pub model: String,
    pub vendor: String,
    pub cores: Option<u32>, // Intel EU count or similar
    pub memory_mb: Option<u64>,
    pub frequency_mhz: Option<u32>,
    pub device_type: String, // "Integrated", "Discrete", "Unknown"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareSpecs {
    pub os_name: String,
    pub os_version: String,
    pub total_memory_gb: f64,
    pub free_memory_gb: f64,
    pub cpu_cores: usize,
    pub cpu_brand: String,
    pub storage_type: String, // "HDD", "SSD", "Unknown"
    pub gpu: Option<HardwareGpu>,
}
