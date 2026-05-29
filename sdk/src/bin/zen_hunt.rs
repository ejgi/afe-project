use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use zen_engine::{VirtualDataset, AnalysisOptions, HardwareMode};
use zen_engine::analytics::hybrid::HybridEngine;
use colored::*;

/// Zen-Hunt: Specialized Forensic Search Tool
#[derive(Parser)]
#[command(author, version, about = "Zen-Hunt - High-Performance Forensic Scanner", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Output results in JSON format for machine integration
    #[arg(long, default_value_t = false)]
    json: bool,
    /// Silent mode: only output results, no headers or progress
    #[arg(short, long, default_value_t = false)]
    quiet: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan for a list of IoCs (Atomic Indicators of Compromise)
    Scan {
        #[arg(short, long)]
        path: PathBuf,
        #[arg(short, long)]
        ioc_file: PathBuf,
        #[arg(short, long, default_value_t = 100)]
        limit: usize,
        #[arg(long, default_value = "hdd")]
        hardware: String,
    },
    /// Hunt for a specific forensic substring
    Hunt {
        #[arg(short, long)]
        path: PathBuf,
        #[arg(short, long)]
        query: String,
        #[arg(short, long, default_value_t = 100)]
        limit: usize,
        #[arg(long, default_value = "hdd")]
        hardware: String,
        #[arg(short, long, default_value_t = false)]
        ignore_case: bool,
        #[arg(long, default_value_t = false)]
        indices_only: bool,
    },
    /// Benchmark the engine throughput on your hardware
    Bench {
        /// Path to a large directory or file (e.g. 1GB+)
        #[arg(short, long)]
        path: PathBuf,
        /// Hardware mode: hdd (stable) or ssd (maximum throughput)
        #[arg(long, default_value = "ssd")]
        hardware: String,
    },
    /// [EXPERIMENTAL] Double-Buffer Prefetch scan using the Hybrid I/O Engine
    ///
    /// Uses async kernel read-ahead (MADV_WILLNEED) to overlap I/O with SIMD scanning.
    /// Best results on NVMe SSD hardware. Requires a single file path.
    Hybrid {
        /// Path to a single file to scan
        #[arg(short, long)]
        path: PathBuf,
        /// Pattern to search for (case-insensitive)
        #[arg(short, long)]
        query: String,
        /// Maximum number of matches to return
        #[arg(short, long, default_value_t = 100)]
        limit: usize,
    },
    /// Export a raw binary index (.zendx) for the VS Code Extension
    ///
    /// Generates a file containing 12-bytes per line: [8-byte Offset (f64), 4-byte Length (f32)]
    Index {
        /// Path to the CSV/Log file to index
        #[arg(short, long)]
        path: PathBuf,
        /// Output path for the .zendx file (defaults to appending .zendx to path)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Get system hardware and OS metrics for performance proofs
    SysInfo,
    /// Infer the schema and header names of a file
    Metadata {
        /// Path to the file
        #[arg(short, long)]
        path: PathBuf,
        /// Number of rows to skip (0 = auto)
        #[arg(long, default_value_t = 0)]
        skip_rows: usize,
        /// Treat as RFC-4180 CSV (quoted values)
        #[arg(long, default_value_t = true)]
        rfc_4180: bool,
    },
    /// Fetch a range of rows from a dataset or file
    GetRows {
        #[arg(short, long)]
        path: PathBuf,
        #[arg(short, long)]
        start: usize,
        #[arg(short, long)]
        limit: usize,
    },
    /// Zen-Find: Ultra-fast file and folder finder by name.
    ///
    /// SSD mode: massively parallel (all cores). HDD mode: sequential single-threaded to avoid head-thrashing.
    Find {
        /// Base directory to start searching from
        #[arg(short, long)]
        path: PathBuf,
        /// Name pattern to match (substring, case-insensitive by default)
        #[arg(short, long)]
        query: String,
        /// Maximum results to return
        #[arg(short, long, default_value_t = 100)]
        limit: usize,
        /// Case-sensitive matching
        #[arg(long, default_value_t = false)]
        case_sensitive: bool,
        /// Only show directories
        #[arg(long, default_value_t = false)]
        dirs_only: bool,
        /// Only show files
        #[arg(long, default_value_t = false)]
        files_only: bool,
        /// Hardware mode override: "auto", "ssd", "hdd"
        #[arg(long, default_value = "auto")]
        hardware: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { path, ioc_file, limit, hardware } => {
            let hw_mode = if hardware.to_lowercase() == "ssd" { HardwareMode::SSD } else { HardwareMode::HDD };

            let iocs_str = std::fs::read_to_string(ioc_file)?;
            let iocs: Vec<String> = iocs_str.lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && !s.starts_with('#'))
                .collect();

            let options = AnalysisOptions { hardware_mode: hw_mode, ..Default::default() };

            if !cli.quiet && !cli.json {
                println!("\n{} {}", "🕵️‍♂️".bold(), "ZEN-HUNT: Forensic IoC Scanner".bold().underline());
                println!("{} Searching for {} indicators in: {}", "Target:".dimmed(), iocs.len(), path.display().to_string().yellow());
                println!("{} Mode: {}", "Safety:".dimmed(), if hw_mode == HardwareMode::HDD { "STREAM-SAFE (HDD)".green().bold() } else { "NITRO-PARALLEL (SSD)".red() });
                println!();
            }

            let dataset = VirtualDataset::new(&path, &options)?;
            let start = std::time::Instant::now();
            let results = dataset.search_iocs(&iocs, limit)?;
            let duration = start.elapsed();

            if cli.json {
                let indices: Vec<u64> = results.iter().map(|r| r.row_index as u64).collect();
                println!("{}", serde_json::to_string(&indices)?);
            } else {
                for res in &results {
                    println!("[FOUND] {}:{} -> {}", res.file_name.cyan(), res.row_index.to_string().yellow(), res.content);
                }
                if !cli.quiet {
                    println!("\n{} Scan complete in {:.2?}. Found {} matches.", "RESULT:".bold(), duration, results.len());
                }
            }
            let early_stop = results.len() >= limit;
            let log_size_bytes = if early_stop { 0 } else { (dataset.get_total_size_mb() * 1024.0 * 1024.0) as u64 };
            zen_engine::utils::log_telemetry(&path, "scan", duration, log_size_bytes);
        }

        Commands::Hunt { path, query, limit, hardware, ignore_case, indices_only } => {
            let hw_mode = if hardware.to_lowercase() == "ssd" { HardwareMode::SSD } else { HardwareMode::HDD };
            let mut options = AnalysisOptions { hardware_mode: hw_mode, no_index: true, ..Default::default() };
            options.no_index = true; // FORCE forensic mode to ignore stale/misaligned sidecar indices

            if !cli.quiet && !cli.json {
                println!("{} Initializing Dataset...", "Status:".dimmed());
            }

            let dataset = VirtualDataset::new(&path, &options)?;
            let start = std::time::Instant::now();
            let results = dataset.search(&query, None, limit, true, false, ignore_case, indices_only)?;
            let duration = start.elapsed();

            if cli.json {
                let indices: Vec<u64> = results.iter().map(|r| r.row_index as u64).collect();
                println!("{}", serde_json::to_string(&indices)?);
            } else {
                for res in &results {
                    println!("[HIT] {}:{} -> {}", res.file_name.cyan(), res.row_index.to_string().yellow(), res.content);
                }
                if !cli.quiet {
                    println!("\n{} Hunt complete in {:.2?}. Found {} matches.", "RESULT:".bold(), duration, results.len());
                }
            }
            let early_stop = results.len() >= limit;
            let log_size_bytes = if early_stop { 0 } else { (dataset.get_total_size_mb() * 1024.0 * 1024.0) as u64 };
            zen_engine::utils::log_telemetry(&path, "hunt", duration, log_size_bytes);
        }

        Commands::Bench { path, hardware } => {
            let hw_mode = if hardware.to_lowercase() == "ssd" { HardwareMode::SSD } else { HardwareMode::HDD };
            let options = AnalysisOptions { hardware_mode: hw_mode, ..Default::default() };

            if !cli.quiet && !cli.json {
                println!("\n{} {}", "🚀".bold(), "ZEN-HUNT: Hardware Stress Test".bold().underline());
                println!("{} Testing bit-stream throughput in {} mode", "Mode:".dimmed(),
                    if hw_mode == HardwareMode::HDD { "STREAM-SAFE".green() } else { "NITRO-PARALLEL".red() });
            }

            let dataset = VirtualDataset::new(&path, &options)?;
            let size_mb = dataset.get_total_size_mb();

            if !cli.quiet && !cli.json {
                println!("{} File Size: {:.2} MB", "Info:".dimmed(), size_mb);
                println!("{} Initializing Nitro-Engine...", "Status:".dimmed());
            }

            let start = std::time::Instant::now();
            let _ = dataset.search("ZEN_HUNT_BENCHMARK_DUMMY_STRING_12345", None, 1, true, false, false, true)?;
            let duration = start.elapsed();
            let throughput = (size_mb / 1024.0) / duration.as_secs_f64();

            // Gather SysInfo for easy HN reporting
            use sysinfo::System;
            let mut s = System::new_all();
            s.refresh_all();
            let cpu_brand = s.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_else(|| "Unknown CPU".to_string());
            
            if cli.json {
                let bench_res = serde_json::json!({
                    "throughput_gbs": throughput,
                    "duration_ms": duration.as_millis(),
                    "size_mb": size_mb,
                    "cpu": cpu_brand
                });
                println!("{}", serde_json::to_string(&bench_res)?);
            } else if !cli.quiet {
                println!("\n{} Benchmark Finished!", "DONE:".bold().green());
                println!("{} {:.2?}", "Elapsed:".dimmed(), duration);
                println!("{} {:.2} GB/s", "Throughput:".bold(), throughput);
                println!("\n{} {}", "📊".bold(), "HACKER NEWS REPORT (Copy-Paste)".bold().underline());
                println!("- THROUGHPUT: {:.2} GB/s", throughput);
                println!("- TIME: {:.2?}", duration);
                println!("- CPU: {}", cpu_brand);
                println!("- DRIVE: [Pending manual entry]");
                
                if throughput > 4.0 {
                    println!("\n{} NVMe Saturation! You've broken the barrier. 🚀", "Verdict:".bold().yellow());
                } else if throughput > 1.0 {
                    println!("\n{} Solid performance. Nitro-Parallel is flying. 🛫", "Verdict:".bold().cyan());
                } else {
                    println!("\n{} Stable scan. Check if your file is on an HDD. 🛡️", "Verdict:".bold().green());
                }
            }
            zen_engine::utils::log_telemetry(&path, "bench", duration, (dataset.get_total_size_mb() * 1024.0 * 1024.0) as u64);
        }

        Commands::Hybrid { path, query, limit } => {
            println!("\n{} {}", "🧪".bold(), "ZEN-HUNT: Hybrid Double-Buffer Engine [EXPERIMENTAL]".bold().underline());
            println!("{} Double-Buffer Prefetch + SIMD on: {}", "Mode:".dimmed(), path.display().to_string().yellow());
            println!("{} Pattern: '{}'", "Query:".dimmed(), query.yellow().bold());
            println!("{} Kernel pre-fetches next chunk while SIMD scans the current one.", "Strategy:".dimmed().italic());
            println!();

            if !path.is_file() {
                eprintln!("{} The hybrid engine requires a single file, not a directory.", "ERROR:".red().bold());
                eprintln!("{} Usage: zen_hunt hybrid --path /large/file.pcap --query 192.168.1.1", "Hint:".dimmed());
                std::process::exit(1);
            }

            let engine = HybridEngine::open(&path)?;
            let file_mb = engine.file_size() as f64 / (1024.0 * 1024.0);
            println!("{} File Size: {:.2} MB", "Info:".dimmed(), file_mb);
            println!("{} Launching Double-Buffer scan...", "Status:".dimmed());

            let result = engine.scan(&query, limit)?;

            for (pos, context) in &result.matches {
                println!("[HIT] offset:{} -> {}", pos.to_string().yellow(), context);
            }

            println!("\n{} Hybrid Scan Finished!", "DONE:".bold().green());
            println!("{} Time: {} ms", "Elapsed:".dimmed(), result.elapsed_ms);
            println!("{} Result: {:.2} GB/s", "Throughput:".bold(), result.throughput_gbs);
            println!("{} Matches: {}", "Found:".dimmed(), result.matches.len());

            if result.throughput_gbs > 5.0 {
                println!("\n{} NVMe saturation achieved! Double-Buffer at full capacity. 🚀", "Verdict:".bold().yellow());
            } else if result.throughput_gbs > 1.0 {
                println!("\n{} Solid performance. Hybrid engine active. 🛫", "Verdict:".bold().cyan());
            } else {
                println!("\n{} HDD or cached data detected. Stable scan complete. 🛡️", "Verdict:".bold().green());
            }
            zen_engine::utils::log_telemetry(&path, "hybrid", std::time::Duration::from_millis(result.elapsed_ms as u64), engine.file_size() as u64);
        }

        Commands::Index { path, output } => {
            let out_path = output.unwrap_or_else(|| {
                let p = path.clone();
                let mut s = p.into_os_string();
                s.push(".zendx");
                PathBuf::from(s)
            });

            println!("\n{} {}", "📂".bold(), "ZEN-HUNT: Sidecar Indexer (VS Code)".bold().underline());
            println!("{} Input: {}", "Target:".dimmed(), path.display());
            println!("{} Output: {}", "Index:".dimmed(), out_path.display());

            let _options = AnalysisOptions { hardware_mode: HardwareMode::SSD, ..Default::default() };
            let start = std::time::Instant::now();
            
            // We use BigDataEngine directly to bypass VirtualDataset directory aggregation
            let mut engine = zen_engine::engine::BigDataEngine::new(&path, HardwareMode::SSD)?;
            engine.build_index()?;
            
            let row_count = engine.offsets.len();
            let mmap_len = engine.mmap.len();
            
            // Export format: [8-byte Offset (f64), 4-byte Length (f32)] = 12 bytes per row
            use std::io::{Write, BufWriter};
            let file = std::fs::File::create(&out_path)?;
            let mut writer = BufWriter::new(file);
            
            for i in 0..row_count {
                let offset = engine.offsets[i];
                
                // Calculate length by looking at the next offset or the end of the file
                let end = if i + 1 < row_count {
                    engine.offsets[i + 1]
                } else {
                    mmap_len as u64
                };
                
                // Subtract 1 to exclude the newline byte (matching the extension's behavior)
                let length = if end > offset {
                    // We check if it ends with \r\n and subtract accordingly if needed, 
                    // but for compatibility with the extension's `absolutePos - lineStart`
                    // we just subtract 1 (assuming at least one newline char exists).
                    (end - offset).saturating_sub(1) as f32
                } else {
                    0.0
                };
                
                writer.write_all(&(offset as f64).to_le_bytes())?;
                writer.write_all(&length.to_le_bytes())?;
            }
            writer.flush()?;

            let duration = start.elapsed();
            println!("\n{} Indexing complete in {:.2?}.", "RESULT:".bold().green(), duration);
            println!("{} Rows indexed: {}", "Stats:".dimmed(), row_count);
            println!("{} Throughput: {:.2} MB/s", "Speed:".dimmed(), (mmap_len as f64 / 1024.0 / 1024.0) / duration.as_secs_f64());
            
            zen_engine::utils::log_telemetry(&path, "index", duration, mmap_len as u64);
        }
        Commands::SysInfo => {
            use sysinfo::System;
            let mut s = System::new_all();
            s.refresh_all();

            let os_name = System::name().unwrap_or_else(|| "Unknown OS".to_string());
            let os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());
            let cpu_brand = s.cpus().first().map(|c| c.brand().to_string()).unwrap_or_else(|| "Unknown CPU".to_string());
            let total_ram = s.total_memory() / 1024 / 1024; // MB

            let sys_info = serde_json::json!({
                "os": format!("{} {}", os_name, os_version),
                "cpu": cpu_brand,
                "cores": s.cpus().len(),
                "ram_mb": total_ram,
                "arch": std::env::consts::ARCH,
            });

            println!("{}", serde_json::to_string_pretty(&sys_info)?);
        }
        Commands::Metadata { path, skip_rows, rfc_4180 } => {
            let options = AnalysisOptions { 
                skip_rows, 
                rfc_4180, 
                hardware_mode: HardwareMode::SSD, // SSD mode for faster inference
                ..Default::default() 
            };
            
            if path.is_dir() {
                // Folder Metadata
                let dataset = VirtualDataset::new(&path, &options)?;
                let total_files = dataset.get_engine_count();
                let total_size_mb = dataset.get_total_size_mb();
                
                // Get schema from the first file found (heuristic)
                if let Some(engine) = dataset.engines.first() {
                    let schema = engine.infer_schema(100, true, engine.skip_rows)?;
                    let result = serde_json::json!({
                        "path": path.display().to_string(),
                        "is_dataset": true,
                        "total_files": total_files,
                        "total_size_mb": total_size_mb,
                        "has_header": engine.has_header,
                        "skip_rows": engine.skip_rows,
                        "delimiter": engine.delimiter as char,
                        "columns": schema,
                    });
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    anyhow::bail!("No supported files found in dataset: {}", path.display());
                }
            } else {
                // Single File Metadata
                let mut engine = zen_engine::engine::BigDataEngine::new(&path, HardwareMode::SSD)?;
                engine.rfc_4180 = rfc_4180;
                engine.skip_rows = if skip_rows == 0 { engine.auto_detect_preamble() } else { skip_rows };
                
                engine.build_index()?;
                let schema = engine.infer_schema(100, true, engine.skip_rows)?;
                
                let result = serde_json::json!({
                    "path": path.display().to_string(),
                    "is_dataset": false,
                    "has_header": engine.has_header,
                    "skip_rows": engine.skip_rows,
                    "delimiter": engine.delimiter as char,
                    "columns": schema,
                });
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        }
        Commands::GetRows { path, start, limit } => {
            let options = AnalysisOptions { hardware_mode: HardwareMode::SSD, ..Default::default() };
            let dataset = VirtualDataset::new(&path, &options)?;
            let total_rows = dataset.get_total_rows();
            
            let end = (start + limit).min(total_rows);
            let rows = dataset.get_rows_with_meta(start, end)?;
            
            let json_rows: Vec<serde_json::Value> = rows.iter().map(|(content, file)| {
                serde_json::json!({
                    "content": content,
                    "file": file
                })
            }).collect();
            
            println!("{}", serde_json::to_string(&json_rows)?);
        }

        Commands::Find { path, query, limit, case_sensitive, dirs_only, files_only, hardware } => {
            let hw_mode = match hardware.to_lowercase().as_str() {
                "ssd" => HardwareMode::SSD,
                "hdd" => HardwareMode::HDD,
                _ => HardwareMode::Auto,
            };

            let mode_label = if hw_mode == HardwareMode::HDD || (hw_mode == HardwareMode::Auto && zen_engine::utils::is_rotational(&path)) {
                "HDD SAFE (Sequential)".green()
            } else {
                "SSD NITRO (Parallel-All-Cores)".red()
            };

            if !cli.quiet && !cli.json {
                println!("\n{} {}", "🔍".bold(), "ZEN-FIND: File & Folder Finder".bold().underline());
                println!("{} '{}'", "Pattern:".dimmed(), query.yellow().bold());
                println!("{} {}", "Base:".dimmed(), path.display().to_string().cyan());
                println!("{} {}", "Mode:".dimmed(), mode_label);
                println!();
            }

            let start = std::time::Instant::now();
            let results = VirtualDataset::find_files(&path, &query, case_sensitive, dirs_only, files_only, limit, hw_mode)?;
            let duration = start.elapsed();

            if cli.json {
                let json_results: Vec<serde_json::Value> = results.iter().map(|(p, size, is_dir)| {
                    serde_json::json!({
                        "path": p.display().to_string(),
                        "size_bytes": size,
                        "is_dir": is_dir,
                    })
                }).collect();
                println!("{}", serde_json::to_string(&json_results)?);
            } else {
                for (p, size, is_dir) in &results {
                    let icon = if *is_dir { "📁" } else { "📄" };
                    let size_str = if *is_dir { String::new() } else { format!(" ({:.1} KB)", *size as f64 / 1024.0) };
                    println!("{} {}{}", icon, p.display().to_string().cyan(), size_str.dimmed());
                }
                if !cli.quiet {
                    println!("\n{} Found {} items in {:.2?}", "RESULT:".bold().green(), results.len(), duration);
                }
            }
        }
    }

    Ok(())
}
