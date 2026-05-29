/// ZenDMA Benchmark — Tests the full DMA pipeline on real files.
/// Measures throughput (GB/s) for each file and reports which backends
/// were auto-selected for this machine.
///
/// Usage:
///   cargo run --bin zen-dma-bench -- --dir /home/ejgi/Descargas/archivos/prueba/
///   cargo run --bin zen-dma-bench -- --dir /path/to/files/ --chunk-mb 128

use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use colored::*;
use zen_engine::compute::dma;

#[derive(Parser, Debug)]
#[command(name = "zen-dma-bench", about = "ZenDMA Modular Engine Benchmark")]
struct Args {
    /// Directory containing test files
    #[arg(long, short, default_value = "/home/ejgi/Descargas/archivos/prueba/")]
    dir: PathBuf,

    /// Chunk size in MB for streaming (default: 256)
    #[arg(long, default_value_t = 256)]
    chunk_mb: usize,

    /// Max number of files to benchmark (0 = all)
    #[arg(long, default_value_t = 0)]
    max_files: usize,
}

fn human_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.2} GB", bytes as f64 / 1e9)
    } else if bytes >= 1_000_000 {
        format!("{:.2} MB", bytes as f64 / 1e6)
    } else {
        format!("{:.2} KB", bytes as f64 / 1e3)
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::parse();

    println!("{}", "╔══════════════════════════════════════════════════╗".cyan().bold());
    println!("{}", "║          ZenDMA Universal — Benchmark Run         ║".cyan().bold());
    println!("{}", "╚══════════════════════════════════════════════════╝".cyan().bold());
    println!();

    // ── 1. Initialize the ZenDMA engine ──────────────────────────────────────
    let chunk_bytes = args.chunk_mb * 1024 * 1024;
    let mut engine = dma::ZenDmaEngine::auto_detect();
    engine.chunk_size = chunk_bytes;

    println!("  {} Storage  : {}", "●".green(), engine.storage.name().yellow().bold());
    println!("  {} Memory   : {}", "●".green(), engine.memory.name().yellow().bold());
    println!("  {} GPU      : {}", "●".green(), engine.gpu.name().yellow().bold());
    println!("  {} Chunk    : {} MB", "●".blue(), args.chunk_mb);
    println!();

    // ── 2. Discover files ─────────────────────────────────────────────────────
    let mut entries: Vec<_> = std::fs::read_dir(&args.dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .collect();

    // Sort by size descending (largest first — most interesting benchmark)
    entries.sort_by(|a, b| {
        let sa = a.metadata().map(|m| m.len()).unwrap_or(0);
        let sb = b.metadata().map(|m| m.len()).unwrap_or(0);
        sb.cmp(&sa)
    });

    if args.max_files > 0 {
        entries.truncate(args.max_files);
    }

    println!("  {} files found in {}", entries.len().to_string().yellow(), args.dir.display());
    println!("{}", "─────────────────────────────────────────────────────".dimmed());
    println!("{:<55} {:>10} {:>12} {:>12}",
        "File".bold(), "Size".bold(), "Time".bold(), "Throughput".bold());
    println!("{}", "─────────────────────────────────────────────────────".dimmed());

    // ── 3. Benchmark each file ────────────────────────────────────────────────
    let mut total_bytes = 0u64;
    let mut total_ms = 0u128;

    for entry in &entries {
        let path = entry.path();
        let file_size = entry.metadata()?.len();
        let fname = path.file_name().unwrap_or_default().to_string_lossy();
        // Truncate long names: keep first 52 chars
        let display_name = if fname.len() > 52 {
            format!("{}…", &fname[..51])
        } else {
            fname.to_string()
        };

        let t0 = Instant::now();
        let mut bytes_processed: u64 = 0;

        // Run through the storage layer (reads the file in chunks).
        // In this test we don't have a GPU buffer, so we use the storage
        // provider directly and measure pure read throughput.
        let storage = &engine.storage;
        let chunk = engine.chunk_size;
        let mut offset = 0u64;

        loop {
            let remaining = (file_size - offset) as usize;
            if remaining == 0 { break; }
            let current = remaining.min(chunk);

            // Allocate a stack-ish buffer for this run — mmap avoids heap copy.
            let mut buf = vec![0u8; current];
            let source = dma::provider::StorageSource::File(&path);
            match storage.read_into(&source, offset, &mut buf) {
                Ok(n) => {
                    bytes_processed += n as u64;
                    offset += n as u64;
                    if n == 0 { break; }
                }
                Err(e) => {
                    eprintln!("  {} Error reading {}: {}", "✗".red(), display_name, e);
                    break;
                }
            }
        }

        let elapsed_ms = t0.elapsed().as_millis().max(1);
        let throughput_gbs = (bytes_processed as f64 / 1e9) / (elapsed_ms as f64 / 1000.0);
        let throughput_str = format!("{:.2} GB/s", throughput_gbs);

        let throughput_colored = if throughput_gbs >= 1.0 {
            throughput_str.green().bold().to_string()
        } else {
            throughput_str.yellow().to_string()
        };

        println!("{:<55} {:>10} {:>10}ms {:>12}",
            display_name,
            human_bytes(file_size),
            elapsed_ms,
            throughput_colored
        );

        total_bytes += bytes_processed;
        total_ms += elapsed_ms;
    }

    // ── 4. Summary ────────────────────────────────────────────────────────────
    let total_gbs = (total_bytes as f64 / 1e9) / (total_ms as f64 / 1000.0);
    println!("{}", "─────────────────────────────────────────────────────".dimmed());
    println!("{:<55} {:>10} {:>10}ms {:>12}",
        format!("TOTAL ({} files)", entries.len()).bold(),
        human_bytes(total_bytes).bold(),
        total_ms,
        format!("{:.2} GB/s", total_gbs).green().bold()
    );
    println!();
    println!("  {} ZenDMA Benchmark complete.", "✓".green().bold());

    Ok(())
}
