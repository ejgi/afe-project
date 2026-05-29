use std::time::Instant;
use zen_engine::dataset::VirtualDataset;
use zen_engine::types::{AnalysisOptions, AnalysisLevel};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let path = PathBuf::from("/home/archtech/Descargas/prueba/massive_system_logs.csv");
    println!("🧪 Zen Engine Real-Data Verification");
    println!("------------------------------------");
    println!("Target: {:?}", path);
    
    if !path.exists() {
        println!("❌ Error: File not found at {:?}", path);
        return Ok(());
    }

    let metadata = std::fs::metadata(&path)?;
    println!("Size: {:.2} GB", metadata.len() as f64 / 1024.0 / 1024.0 / 1024.0);

    let mut options = AnalysisOptions::default();
    options.no_limit = true; // Nitro Mode
    options.level = AnalysisLevel::Basic; // Basic uses SIMD Batching
    options.delimiter = Some(b',');
    options.has_header = true;

    println!("\nInitializing Engine...");
    let t_init = Instant::now();
    let dataset = VirtualDataset::new(&path, &options)?;
    println!("Initialization took: {:.2?}", t_init.elapsed());

    println!("\n🚀 Running Full Analysis (SIMD Batching)...");
    let t_analysis = Instant::now();
    let result = dataset.analyze(
        &options, None, None, None, None, None, None, None, None, None, None, None, 1
    )?;
    let d_analysis = t_analysis.elapsed();

    // Read Memory RSS and Anon
    let mut memory_rss_mb = 0.0;
    let mut memory_anon_mb = 0.0;
    let mut memory_file_mb = 0.0;
    
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<f64>() {
                        memory_rss_mb = kb / 1024.0;
                    }
                }
            } else if line.starts_with("RssAnon:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<f64>() {
                        memory_anon_mb = kb / 1024.0;
                    }
                }
            } else if line.starts_with("RssFile:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<f64>() {
                        memory_file_mb = kb / 1024.0;
                    }
                }
            }
        }
    }

    println!("------------------------------------");
    println!("Rows Processed: {}", result.row_count);
    println!("Analysis Time:  {:.2?}", d_analysis);
    
    let throughput = (metadata.len() as f64 / 1024.0 / 1024.0) / d_analysis.as_secs_f64();
    println!("Throughput:    {:.2} MB/s", throughput);
    println!("Memory (Total VmRSS): {:.2} MB", memory_rss_mb);
    println!("Memory (True RAM Used by App / RssAnon): {:.2} MB", memory_anon_mb);
    println!("Memory (Zero-Copy Mmap Cache / RssFile): {:.2} MB", memory_file_mb);
    
    if !result.column_stats.is_empty() {
        println!("\nSample Stats (First Column):");
        let stat = &result.column_stats[0];
        println!("  - Name: {}", stat.name);
        println!("  - Mean: {:.4}", stat.mean);
        println!("  - Min:  {:.4}", stat.min);
        println!("  - Max:  {:.4}", stat.max);
    }

    Ok(())
}
