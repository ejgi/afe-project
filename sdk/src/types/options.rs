use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
pub enum AnalysisLevel {
    Basic,      // Row count, min/max/sum/mean (no histograms, no categories, no correlations)
    Discovery,  // Basic + categories + histograms (no correlations)
    Full,       // Discovery + correlations (N^2 operations)
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum, Default)]
pub enum HardwareMode {
    #[default]
    Auto,  // Detect HDD/SSD automatically
    HDD,   // Force HDD safety mode (2 threads)
    SSD,   // Force SSD mode (standard parallelism)
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum, Default)]
pub enum BusinessTemplate {
    #[default]
    General,
    Finance,
    Network,
    Cybersecurity,
    Sales,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum, Default)]
pub enum IpScanMode {
    V4,
    V6,
    #[default]
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisOptions {
    pub level: AnalysisLevel,
    pub target_columns: Option<Vec<usize>>,
    pub blueprint: Option<super::blueprint::Blueprint>,
    pub delimiter: Option<u8>,
    pub regex_pattern: Option<String>,
    pub rfc_4180: bool,
    pub skip_rows: usize,
    pub has_header: bool,
    pub enable_network: bool,
    pub chunk_size_mb: Option<usize>,
    pub no_index: bool,
    pub hardware_mode: HardwareMode,
    pub gpu: bool,
    pub forced_format: Option<String>,
    pub threads: Option<usize>,
    pub no_limit: bool,
    pub filter_ast: Option<crate::filter::Expr>,
    pub strip_quotes: bool,
    pub business_template: BusinessTemplate,
    pub full_scan: bool,
    pub ip_scan_mode: IpScanMode,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self {
            level: AnalysisLevel::Basic,
            target_columns: None,
            blueprint: None,
            delimiter: None,
            regex_pattern: None,
            rfc_4180: false,
            skip_rows: 0,
            has_header: true,
            enable_network: false,
            chunk_size_mb: None,
            no_index: false,
            hardware_mode: HardwareMode::Auto,
            gpu: false,
            forced_format: None,
            threads: None,
            no_limit: false,
            filter_ast: None,
            strip_quotes: false,
            business_template: BusinessTemplate::General,
            full_scan: false,
            ip_scan_mode: IpScanMode::Both,
        }
    }
}
