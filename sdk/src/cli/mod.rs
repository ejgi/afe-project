pub mod handlers;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use anyhow::Result;
use crate::types::AnalysisLevel;

#[derive(Parser)]
#[command(name = "zen-engine")]
#[command(about = "High-performance forensic analytical engine for massive datasets", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Index a file for fast row access (Offsets Map)
    Index {
        /// Path to the CSV file
        path: PathBuf,
        /// Force re-indexing even if .idx exists
        #[arg(short, long)]
        force: bool,
    },
    /// Build a ZoneMap (Min/Max values per block) for streaming optimization
    BuildZones {
        /// Path to the CSV file
        path: PathBuf,
        /// Size of each zone (rows), default 65536
        #[arg(short, long, default_value_t = 65536)]
        size: u64,
        /// Column delimiter (default: ,)
        #[arg(short, long)]
        delimiter: Option<String>,
        /// Enable RFC 4180 strict CSV parsing
        #[arg(long)]
        rfc_4180: bool,
        /// Force hardware profile: auto, hdd, or ssd
        #[arg(long)]
        hardware: Option<String>,
    },
    /// Deep statistical analysis of a dataset
    Analyze {
        /// Path to the file or directory
        path: PathBuf,
        /// Optional path to a blueprint JSON file
        #[arg(short, long)]
        blueprint: Option<PathBuf>,
        /// Build ZoneMap during analysis if it doesn't exist
        #[arg(long)]
        index_zones: bool,
        /// Use existing ZoneMap to skip irrelevant data blocks
        #[arg(long)]
        use_zones: bool,
        /// Column index to filter by
        #[arg(long)]
        filter_col: Option<usize>,
        #[arg(long)]
        filter_min: Option<f64>,
        #[arg(long)]
        filter_max: Option<f64>,
        #[arg(long)]
        filter_text_col: Option<usize>,
        #[arg(long)]
        filter_text: Option<String>,
        /// Boolean expression filter (e.g. "col1 > 10 AND col2 == 'foo'")
        #[arg(short, long)]
        filter: Option<String>,
        #[arg(long)]
        date_col: Option<usize>,
        #[arg(long)]
        date_from: Option<String>,
        #[arg(long)]
        date_to: Option<String>,
        /// Analysis level: basic, discovery, full
        #[arg(short, long, default_value = "basic")]
        level: AnalysisLevel,
        #[arg(short, long)]
        delimiter: Option<String>,
        #[arg(long)]
        no_header: bool,
        #[arg(long)]
        regex: Option<String>,
        #[arg(long)]
        rfc_4180: bool,
        #[arg(long, default_value_t = 0)]
        skip: usize,
        /// Calculate integrity hash (BLAKE3)
        #[arg(long)]
        hash: bool,
        /// Hardware mode: auto, hdd, or ssd
        #[arg(long)]
        hardware: Option<String>,
        /// Enable specialized network/IP analytics
        #[arg(short, long)]
        network: bool,
        /// Run analysis without building/loading index (streaming only)
        #[arg(long)]
        no_index: bool,
        #[arg(long)]
        chunk_size: Option<usize>,
        #[arg(long, default_value_t = 1)]
        loop_count: usize,
        /// Enable GPU acceleration (Nitro-GPU)
        #[arg(long)]
        gpu: bool,
        #[arg(long)]
        format: Option<String>,
        /// Use a saved profile from DB
        #[arg(long)]
        profile: Option<String>,
        /// Number of threads to use (default: auto)
        #[arg(long)]
        threads: Option<usize>,
        /// Bypass all safety limits (Max performance)
        #[arg(long)]
        no_limit: bool,
        /// Strip quotes from CSV fields
        #[arg(long)]
        strip_quotes: bool,
        /// Output template (Report format)
        #[arg(long)]
        template: Option<String>,
        /// Perform a full scan (bypass default directory exclusions like .git or node_modules)
        #[arg(long)]
        full_scan: bool,
    },
    /// Fast row preview
    View {
        path: PathBuf,
        #[arg(short, long, default_value_t = 0)]
        start: usize,
        #[arg(short, long, default_value_t = 20)]
        count: usize,
        #[arg(long)]
        hardware: Option<String>,
    },
    /// Fast text search (Indexed or Parallel Streaming)
    Search {
        path: PathBuf,
        /// Search query (Exact match or Regex)
        query: String,
        /// Column index to search (Optional)
        #[arg(short, long)]
        col: Option<usize>,
        #[arg(short, long, default_value_t = 50)]
        limit: usize,
        #[arg(short, long)]
        delimiter: Option<String>,
        #[arg(long)]
        no_header: bool,
        #[arg(long)]
        rfc_4180: bool,
        #[arg(long, default_value_t = 0)]
        skip: usize,
        /// Search in streaming mode without using index
        #[arg(long)]
        no_index: bool,
        #[arg(long)]
        gpu: bool,
        #[arg(long)]
        hardware: Option<String>,
        #[arg(long)]
        format: Option<String>,
        #[arg(long)]
        strip_quotes: bool,
        /// Output results in JSON for external integration
        #[arg(long)]
        json: bool,
    },
    /// Export filtered rows to another file
    Export {
        path: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        filter_col: Option<usize>,
        #[arg(long)]
        filter_min: Option<f64>,
        #[arg(long)]
        filter_max: Option<f64>,
        #[arg(long)]
        filter_text_col: Option<usize>,
        #[arg(long)]
        filter_text: Option<String>,
        #[arg(short, long)]
        filter: Option<String>,
        #[arg(long)]
        date_col: Option<usize>,
        #[arg(long)]
        date_from: Option<String>,
        #[arg(long)]
        date_to: Option<String>,
        #[arg(long)]
        use_zones: bool,
        #[arg(short, long, default_value = "csv")]
        output_format: String,
        #[arg(short, long)]
        delimiter: Option<String>,
        #[arg(long)]
        no_header: bool,
        #[arg(long)]
        regex: Option<String>,
        #[arg(long)]
        rfc_4180: bool,
        #[arg(long, default_value_t = 0)]
        skip: usize,
        #[arg(long)]
        hardware: Option<String>,
    },
    /// Grouping and fast aggregation (CPU/GPU)
    Group {
        path: PathBuf,
        #[arg(long)]
        by: usize,
        #[arg(long)]
        agg: usize,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        gpu: bool,
        #[arg(long)]
        hardware: Option<String>,
    },
    /// Get the Top-N elements by column
    Top {
        path: PathBuf,
        #[arg(long)]
        col: usize,
        #[arg(short, long, default_value_t = 10)]
        n: usize,
        #[arg(long)]
        desc: bool,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        gpu: bool,
        #[arg(long)]
        hardware: Option<String>,
    },
    /// Scan and report hardware capabilities
    ScanHardware,
    /// Run a standardized performance benchmark on a dataset
    Bench {
        path: PathBuf,
        #[arg(long)]
        gpu: bool,
    },
    /// Forensic IoC Scan using Aho-Corasick DFA (Multi-Pattern)
    IocSearch {
        #[arg(short, long)]
        path: PathBuf,
        #[arg(short, long)]
        ioc_file: PathBuf,
        #[arg(short, long, default_value_t = 100)]
        limit: usize,
        #[arg(long)]
        hardware: Option<String>,
    },
    /// Manage persistent engine configurations and blueprints
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Manage and query the persistent scan history database
    History {
        #[command(subcommand)]
        action: HistoryAction,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    Create {
        name: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        blueprint: Option<PathBuf>,
        #[arg(long)]
        delimiter: Option<String>,
        #[arg(long)]
        format: Option<String>,
        #[arg(long)]
        hardware: Option<String>,
        #[arg(long)]
        skip: Option<usize>,
        #[arg(long)]
        no_header: bool,
        #[arg(long)]
        rfc_4180: bool,
        #[arg(long)]
        network: bool,
        #[arg(long)]
        gpu: bool,
        #[arg(long)]
        regex: Option<String>,
        #[arg(long)]
        chunk_size: Option<usize>,
        #[arg(long)]
        level: Option<String>,
        #[arg(long)]
        threads: Option<usize>,
        #[arg(long)]
        no_limit: bool,
        #[arg(long)]
        strip_quotes: bool,
    },
    List,
    Delete { name: String },
    Show { name: String },
}

#[derive(Subcommand)]
pub enum HistoryAction {
    List { #[arg(long)] path: Option<String> },
    Show { index: usize, #[arg(long)] path: Option<String> },
    Compare { a: usize, b: usize, #[arg(long)] path: Option<String> },
    Delete { file: String },
}

pub fn main_impl() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Index { path, force } => handlers::index::handle_index(path, force),
        Commands::BuildZones { path, size, delimiter, rfc_4180, hardware } => handlers::index::handle_build_zones(path, size, delimiter, rfc_4180, hardware),
        Commands::Analyze { 
            path, blueprint, index_zones, use_zones, filter_col, filter_min, filter_max, 
            filter_text_col, filter_text, filter, date_col, date_from, date_to, 
            level, delimiter, no_header, regex, rfc_4180, skip, hash, hardware, network, no_index, chunk_size, loop_count, gpu, format, profile, threads, no_limit, strip_quotes, template, full_scan
        } => handlers::analyze::handle_analyze(path, blueprint, index_zones, use_zones, filter_col, filter_min, filter_max, filter_text_col, filter_text, filter, date_col, date_from, date_to, level, delimiter, no_header, regex, rfc_4180, skip, hash, hardware, network, no_index, chunk_size, loop_count, gpu, format, profile, threads, no_limit, strip_quotes, template, full_scan),
        Commands::View { path, start, count, hardware } => handlers::view::handle_view(path, start, count, hardware),
        Commands::Search { path, query, col, limit, delimiter, no_header, rfc_4180, skip, no_index, gpu, hardware, format, strip_quotes, json } => handlers::search::handle_search(path, query, col, limit, delimiter, no_header, rfc_4180, skip, no_index, gpu, hardware, format, strip_quotes, json),
        Commands::Export { path, output, filter_col, filter_min, filter_max, filter_text_col, filter_text, filter, date_col, date_from, date_to, use_zones, output_format, delimiter, no_header, regex, rfc_4180, skip, hardware } => handlers::export::handle_export(path, output, filter_col, filter_min, filter_max, filter_text_col, filter_text, filter, date_col, date_from, date_to, use_zones, output_format, delimiter, no_header, regex, rfc_4180, skip, hardware),
        Commands::Group { path, by, agg, filter, gpu, hardware } => {
            let options = crate::types::AnalysisOptions { hardware_mode: match hardware.as_deref().unwrap_or("auto").to_lowercase().as_str() { "hdd" => crate::types::HardwareMode::HDD, "ssd" => crate::types::HardwareMode::SSD, _ => crate::types::HardwareMode::Auto }, ..Default::default() };
            let mut dataset = crate::dataset::VirtualDataset::new(&path, &options)?;
            if gpu { dataset.try_enable_gpu(); }
            let filter_ast = filter.as_ref().and_then(|f| crate::filter::parse_filter(f).ok());
            crate::GroupAnalysis::render_group(&mut dataset, by, agg, filter_ast.as_ref(), gpu)
        }
        Commands::Top { path, col, n, desc, filter, gpu, hardware } => {
            let options = crate::types::AnalysisOptions { hardware_mode: match hardware.as_deref().unwrap_or("auto").to_lowercase().as_str() { "hdd" => crate::types::HardwareMode::HDD, "ssd" => crate::types::HardwareMode::SSD, _ => crate::types::HardwareMode::Auto }, ..Default::default() };
            let mut dataset = crate::dataset::VirtualDataset::new(&path, &options)?;
            if gpu { dataset.try_enable_gpu(); }
            let filter_ast = filter.as_ref().and_then(|f| crate::filter::parse_filter(f).ok());
            crate::GroupAnalysis::render_top(&mut dataset, col, n, desc, filter_ast.as_ref(), gpu)
        }
        Commands::ScanHardware => handlers::utils::handle_scan_hardware(),
        Commands::Bench { path, gpu } => handlers::bench::handle_bench(path, gpu),
        Commands::IocSearch { path, ioc_file, limit, hardware } => handlers::search::handle_ioc_search(path, ioc_file, limit, hardware),
        Commands::Config { action } => handlers::config::handle_config(action),
        Commands::History { action } => handlers::history::handle_history(action),
    }
}
