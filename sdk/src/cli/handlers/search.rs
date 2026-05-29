use std::path::PathBuf;
use anyhow::{Result, Context};
use colored::Colorize;
use crate::dataset::VirtualDataset;
use crate::types::{AnalysisOptions, HardwareMode};

#[allow(clippy::too_many_arguments)]
pub fn handle_search(
    path: PathBuf, 
    query: String, 
    col: Option<usize>, 
    limit: usize, 
    delimiter: Option<String>, 
    no_header: bool, 
    rfc_4180: bool, 
    skip: usize, 
    no_index: bool, 
    gpu: bool, 
    hardware: Option<String>, 
    format: Option<String>, 
    strip_quotes: bool, 
    json: bool
) -> Result<()> {
    let hw_mode = match hardware.as_deref().unwrap_or("auto").to_lowercase().as_str() {
        "hdd" => HardwareMode::HDD,
        "ssd" => HardwareMode::SSD,
        _ => HardwareMode::Auto,
    };

    let options = AnalysisOptions {
        delimiter: delimiter.as_ref().map(|s| {
            match s.as_str() {
                "\\t" => b'\t',
                "\\n" => b'\n',
                "\\r" => b'\r',
                _ => s.as_bytes()[0],
            }
        }).or(Some(b',')),
        rfc_4180,
        skip_rows: skip,
        has_header: !no_header,
        no_index,
        hardware_mode: hw_mode,
        gpu,
        forced_format: format,
        strip_quotes,
        ..Default::default()
    };

    let mut dataset = VirtualDataset::new(&path, &options).context("Failed to load dataset")?;
    
    if gpu {
        println!("{} Attempting to enable GPU acceleration...", "INFO:".blue());
        dataset.try_enable_gpu();
    }

    println!("{} Searching for '{}' in {}...", "INTEL:".cyan(), query.bold().yellow(), path.display());
    let start_time = std::time::Instant::now();
    
    let results = dataset.search(&query, col, limit, no_index, gpu, false, false)?;
    let duration = start_time.elapsed();

    if json {
        #[derive(serde::Serialize)]
        struct CliSearchResponse {
            results: Vec<crate::types::SearchResult>,
            duration_ns: u64,
            bytes_processed: u64,
            throughput_gb_s: f64,
        }
        
        let total_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let duration_ns = duration.as_nanos() as u64;
        let duration_secs = duration.as_secs_f64();
        let throughput_gb_s = if duration_secs > 0.000000001 {
            ((total_bytes as f64) / 1024.0 / 1024.0 / 1024.0) / duration_secs
        } else {
            0.0
        };
        
        let resp = CliSearchResponse {
            results,
            duration_ns,
            bytes_processed: total_bytes,
            throughput_gb_s,
        };
        
        println!("{}", serde_json::to_string(&resp).unwrap());
        return Ok(());
    }

    if results.is_empty() {
        println!("{} No matches found (Time: {:?})", "INFO:".blue(), duration);
    } else {
        println!("{} Found {} matches (Time: {:?})", "SUCCESS:".green(), results.len().to_string().bold(), duration);
        println!("{}", "-".repeat(100).dimmed());
        println!("| {:<30} | {:>8} | {}", "File".bold(), if no_index { "Offset".bold() } else { "Row".bold() }, "Content".bold());
        println!("{}", "-".repeat(100).dimmed());
        for res in results {
            let fname = std::path::Path::new(&res.file_name)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or(res.file_name);
            println!("| {:<30} | {:>8} | {}", fname.dimmed(), res.row_index.to_string().cyan(), res.content);
        }
        println!("{}", "-".repeat(100).dimmed());
    }
    Ok(())
}

pub fn handle_ioc_search(path: PathBuf, ioc_file: PathBuf, limit: usize, hardware: Option<String>) -> Result<()> {
    let hw_mode = match hardware.as_deref().unwrap_or("auto").to_lowercase().as_str() {
        "hdd" => HardwareMode::HDD,
        "ssd" => HardwareMode::SSD,
        _ => HardwareMode::Auto,
    };
    
    let ioc_content = std::fs::read_to_string(ioc_file)?;
    let iocs: Vec<String> = ioc_content.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    
    let options = AnalysisOptions { hardware_mode: hw_mode, ..Default::default() };
    let dataset = VirtualDataset::new(&path, &options)?;
    
    println!("{} Performing IoC scan with {} patterns...", "INTEL:".cyan(), iocs.len());
    let results = dataset.search_iocs(&iocs, limit)?;
    
    if results.is_empty() {
        println!("{} No IoC matches found.", "INFO:".blue());
    } else {
        println!("{} Found {} IoC matches.", "SUCCESS:".green(), results.len());
        for res in results {
            println!("[{}] Row {}: {}", res.file_name.dimmed(), res.row_index, res.content);
        }
    }
    Ok(())
}
