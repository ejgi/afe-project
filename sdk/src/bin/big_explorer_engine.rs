use anyhow::{Result, Context};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::io::{self, BufRead, Write};
use rayon::prelude::*;
use zen_engine::engine::BigDataEngine;

#[derive(Parser)]
#[command(author, version, about = "Big Data Explorer - Dedicated VS Code Engine (Forensic Nitro Edition)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    SysInfo,
    Daemon {
        #[arg(long)]
        path: PathBuf,
    },
}

#[derive(serde::Deserialize, Debug)]
struct RpcCommand {
    msg_id: String,
    cmd: String,
    start: Option<usize>,
    limit: Option<usize>,
    query: Option<String>,
}

#[derive(serde::Serialize)]
struct RpcResponse<T> {
    msg_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

struct EngineDaemon {
    engine: BigDataEngine,
    path: PathBuf,
}

impl EngineDaemon {
    fn new(path: &PathBuf) -> Result<Self> {
        let mut engine = BigDataEngine::new(path, zen_engine::types::HardwareMode::Auto)
            .context("Failed to open file via Zen-Engine")?;
        
        // Smart Forensic Detection
        engine.skip_rows = engine.auto_detect_preamble();
        // Check for RFC-4180 (Quotes) in the first 8KB
        engine.rfc_4180 = engine.mmap.iter().take(8192).any(|&b| b == b'"');
        engine.has_header = true; // Standard for Forensic/BigData Explorer
        
        // Build index with smart context
        engine.build_index().context("Failed to build navigation index")?;
        
        Ok(Self { 
            engine,
            path: path.clone()
        })
    }

    fn handle_metadata(&self) -> serde_json::Value {
        let total_rows = self.engine.offsets.len();
        let schema = self.engine.infer_schema(1024 * 1024, false, 0).unwrap_or_default();
        let columns: Vec<serde_json::Value> = schema.iter().map(|col| {
            serde_json::json!({
                "name": col.name,
                "data_type": format!("{:?}", col.data_type)
            })
        }).collect();

        serde_json::json!({
            "total_rows": total_rows,
            "skip_rows": self.engine.skip_rows,
            "columns": columns
        })
    }

    fn handle_get_rows(&self, start: usize, limit: usize) -> serde_json::Value {
        let end = start + limit;
        let rows = self.engine.get_rows(start, end);
        serde_json::json!({ "rows": rows })
    }

    fn handle_search(&self, query: &str, limit: usize) -> serde_json::Value {
        let data = &self.engine.mmap;
        let query_bytes = query.as_bytes();
        if query_bytes.is_empty() { return serde_json::json!({ "indices": Vec::<u64>::new() }); }

        let chunk_size = 256 * 1024 * 1024;
        let chunks: Vec<(usize, &[u8])> = data.chunks(chunk_size)
            .enumerate()
            .map(|(i, c)| (i * chunk_size, c))
            .collect();

        let found_indices: Vec<u64> = chunks.into_par_iter()
            .flat_map_iter(|(offset, chunk)| {
                let matches = self.engine.zenscan.scan_raw(chunk, query_bytes, true);
                matches.into_iter().map(move |m_pos| {
                    let absolute_pos = offset + m_pos as usize;
                    let pos_idx = self.engine.offsets.partition_point(|&x| x <= absolute_pos as u64).saturating_sub(1);
                    pos_idx as u64
                })
            })
            .collect();

        let mut seen = std::collections::HashSet::new();
        let unique_indices: Vec<u64> = found_indices.into_iter()
            .filter(|&i| seen.insert(i))
            .take(limit)
            .collect();

        serde_json::json!({ "indices": unique_indices })
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::SysInfo => {
            let hw = zen_engine::utils::get_hardware_specs();
            let json = serde_json::json!({
                "os": hw.os_name, "cpu": hw.cpu_brand, "cores": hw.cpu_cores,
                "ram_mb": hw.total_memory_gb * 1024.0, "disk": hw.storage_type
            });
            println!("{}", serde_json::to_string(&json)?);
        }
        Commands::Daemon { path } => {
            let daemon = match EngineDaemon::new(&path) {
                Ok(d) => d,
                Err(e) => {
                    println!("{}", serde_json::json!({"error": e.to_string()}));
                    return Ok(());
                }
            };
            
            let mut stdout = io::stdout();
            writeln!(stdout, "{}", serde_json::json!({"status": "ready", "total_rows": daemon.engine.offsets.len()}))?;
            stdout.flush()?;

            for line in io::stdin().lock().lines() {
                let Ok(line_str) = line else { break };
                if line_str.trim().is_empty() { continue; }
                
                let req: RpcCommand = if let Ok(r) = serde_json::from_str(&line_str) { r } else { continue };

                let res_json = match req.cmd.as_str() {
                    "metadata" => daemon.handle_metadata(),
                    "get_rows" => daemon.handle_get_rows(req.start.unwrap_or(0), req.limit.unwrap_or(20)),
                    "search" => daemon.handle_search(&req.query.clone().unwrap_or_default(), req.limit.unwrap_or(1000)),
                    "exit" => break,
                    _ => serde_json::json!({"error": "unknown command"})
                };

                writeln!(stdout, "{}", serde_json::to_string(&RpcResponse { msg_id: req.msg_id, result: Some(res_json), error: None })?)?;
                stdout.flush()?;
            }
        }
    }
    Ok(())
}
