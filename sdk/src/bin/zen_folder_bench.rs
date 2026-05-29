use std::time::Instant;
use zen_engine::dataset::VirtualDataset;
use zen_engine::types::{AnalysisOptions, AnalysisLevel};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let dir_path = PathBuf::from("/home/ejgi/Descargas/archivos/prueba/");
    println!("📂 Zen Engine Folder-Wide Analysis");
    println!("------------------------------------");
    println!("Target Directory: {:?}", dir_path);
    
    if !dir_path.exists() || !dir_path.is_dir() {
        println!("❌ Error: Directory not found or not a directory: {:?}", dir_path);
        return Ok(());
    }

    let mut options = AnalysisOptions::default();
    options.no_limit = true; // Nitro Mode
    options.level = AnalysisLevel::Basic; // Basic uses SIMD Batching
    options.delimiter = Some(b',');
    options.has_header = true;
    options.no_index = true; // Analyze raw to benchmark direct IO + SIMD

    println!("\nInitializing Engine on entire folder...");
    let t_init = Instant::now();
    let dataset = VirtualDataset::new(&dir_path, &options)?;
    println!("Initialization (File discovery) took: {:.2?}", t_init.elapsed());
    println!("Found {} engines (files to process).", dataset.engines.len());

    println!("\n🚀 Running Aggregated Folder Analysis (SIMD Batching)...");
    let t_analysis = Instant::now();
    let result = dataset.analyze(
        &options, None, None, None, None, None, None, None, None, None, None, None, 1
    )?;
    let d_analysis = t_analysis.elapsed();

    println!("------------------------------------");
    println!("Total Rows Processed: {}", result.row_count);
    println!("Total Analysis Time:  {:.2?}", d_analysis);
    
    let total_bytes: u64 = dataset.engines.iter().map(|e| e.mmap.len() as u64).sum();
    let total_gb = total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
    println!("Total Data Volume:    {:.2} GB", total_gb);
    
    let throughput = (total_bytes as f64 / 1024.0 / 1024.0) / d_analysis.as_secs_f64();
    println!("Global Throughput:    {:.2} MB/s", throughput);
    
    if !result.column_stats.is_empty() {
        println!("\nAggregated Stats (First Column):");
        let stat = &result.column_stats[0];
        println!("  - Name: {}", stat.name);
        println!("  - Mean: {:.4}", stat.mean);
        println!("  - Count: {}", stat.count);
    }

    Ok(())
}
