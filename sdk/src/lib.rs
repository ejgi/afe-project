//! # Zen Engine SDK (Unified Edition)
//!
//! `zen-engine` is an ultra-high performance, Data-Oriented parsing and analytical engine
//! designed for massive digital forensics datasets (CSV, EVTX, PCAP) without RAM bloating.
//!
//! ## Core Design Philosophy
//! - **Anti-OOP / DOD**: Employs strictly Data-Oriented Design and flat memory layouts to maximize CPU cache hits.
//! - **Zero-Footprint**: Analyzes 100GB+ files with O(1) memory complexity using `mmap2` and Hugepages.
//! - **SIMD Acceleration**: Custom AVX2 math kernels processing 4 `f64` per cycle for statistical moments.
//! - **Lock-Free Concurrency**: Uses `AtomicU64` and Compare-And-Swap (CAS) loops for extreme parallel aggregation.
//!
//! ## Entry Points
//! - [`VirtualDataset`]: The main orchestrator for multi-file/directory analysis.
//! - [`BigDataEngine`]: The underlying atomic engine handling raw memory-mapped files.

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
