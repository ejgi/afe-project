use std::path::PathBuf;
use anyhow::{Result, Context};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use crate::engine::BigDataEngine;
use crate::types::{HardwareMode, AnalysisOptions};

pub fn handle_index(path: PathBuf, force: bool) -> Result<()> {
    println!("{} Indexing {}", "INTEL:".cyan(), path.display());
    
    if force {
        let idx_path = path.with_extension("csv.idx");
        if idx_path.exists() {
            std::fs::remove_file(&idx_path)?;
            println!("{} Existing index purged.", "WARN:".yellow());
        }
    }

    let mut engine = BigDataEngine::new(&path, HardwareMode::Auto)
        .with_context(|| format!("Failed to open {}", path.display()))?;
    
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} [{elapsed_precise}] {msg}")?);
    pb.set_message("Building offset map...");
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    engine.build_index()?;
    
    pb.finish_with_message(format!(
        "{} Indexed {} rows successfully.", 
        "SUCCESS:".green(), 
        engine.offsets.len().to_string().bold()
    ));
    Ok(())
}

pub fn handle_build_zones(path: PathBuf, size: u64, delimiter: Option<String>, rfc_4180: bool, hardware: Option<String>) -> Result<()> {
    println!("{} Building ZoneMap for {} (Zero-RAM Streaming)...", "INTEL:".cyan(), path.display().to_string().bold());
    let hw_mode = match hardware.as_deref().unwrap_or("auto").to_lowercase().as_str() {
        "hdd" => HardwareMode::HDD,
        "ssd" => HardwareMode::SSD,
        _ => HardwareMode::Auto,
    };
    
    let options = AnalysisOptions {
        delimiter: delimiter.as_ref().map(|s| s.as_bytes()[0]).or(Some(b',')),
        rfc_4180,
        hardware_mode: hw_mode,
        gpu: false,
        ..Default::default()
    };
    let mut engine = BigDataEngine::new(&path, hw_mode)?;
    engine.delimiter = options.delimiter.unwrap();
    engine.rfc_4180 = options.rfc_4180;
    
    let zm = engine.build_zone_map(size, hw_mode)?;
    engine.save_zone_map(&zm)?;
    println!("{} ZoneMap saved successfully.", "SUCCESS:".green());
    Ok(())
}
