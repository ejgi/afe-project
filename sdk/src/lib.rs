pub mod parsers;
pub mod types;
pub mod domain;
pub mod utils;
pub mod accumulator;
pub mod zone_map;
pub mod engine;
pub mod analytics;
pub mod export;
pub mod filter;
pub mod dataset;
pub mod report;
pub mod group;
pub mod compute;
pub mod gds {
    // removed gds
}
pub mod ffi;
pub mod config;
pub mod history;
pub mod compression;
pub mod streaming;
pub mod cli;

// Re-export public types for backward compatibility and ease of use
pub use types::*;
pub use utils::parse_date_fast;
pub use engine::BigDataEngine;
pub use dataset::VirtualDataset;
pub use report::Report;
pub use group::GroupAnalysis;
pub use compute::GpuContext;
