use zen_engine::analytics::zenscan::ZenScan;
use memmap2::Mmap;
use std::fs::File;
use std::time::Instant;
use walkdir::WalkDir;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Dynamic Path Intake (Lab-Ready)
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("\n❌ Error: No se ha proporcionado la ruta de la evidencia.");
        println!("Uso: cargo run --release --example verify_real_pcap <PATH_TO_FOLDER>");
        return Ok(());
    }
    
    let root_path = &args[1];
    
    println!("\n🚀 ZEN ENGINE: LAB-MODE NITRO HUNT (DYNAMIC PATH)");
    println!("Target Evidence: {}", root_path);
    println!("--------------------------------------------------");

    let scanner = Arc::new(ZenScan::new());
    
    // 2. Recursive Discovery (Silent)
    let files: Vec<walkdir::DirEntry> = WalkDir::new(root_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("snort.log"))
        .collect();

    if files.is_empty() {
        println!("⚠️ No se han encontrado archivos 'snort.log' en la ruta especificada.");
        return Ok(());
    }

    println!("📂 Discovery: {} evidence files identified. Running parallel search...", files.len());

    let queries = vec![
        (b"GET".as_slice(), "HTTP_GET", true),
        (b"facebook".as_slice(), "SOCIAL_DOMAIN", true),
        (b"DNS".as_slice(), "DNS_PATTERN", false),
    ];

    for (pattern, label, case_insensitive) in queries {
        let total_matches = AtomicUsize::new(0);
        let total_bytes = AtomicUsize::new(0);
        let start = Instant::now();
        let scanner_ref = Arc::clone(&scanner);

        files.par_iter().for_each(|entry| {
            if let Ok(file) = File::open(entry.path()) {
                if let Ok(mmap) = unsafe { Mmap::map(&file) } {
                    let matches = scanner_ref.scan_raw(&mmap, pattern, case_insensitive);
                    total_matches.fetch_add(matches.len(), Ordering::Relaxed);
                    total_bytes.fetch_add(mmap.len(), Ordering::Relaxed);
                }
            }
        });

        let duration = start.elapsed();
        let bytes = total_bytes.load(Ordering::Relaxed);
        let speed = (bytes as f64 / 1_073_741_824.0) / duration.as_secs_f64();
        
        println!("\n[HUNT: {}]", label);
        println!("  - Matches: {}", total_matches.load(Ordering::Relaxed));
        println!("  - Volume:  {:.2} GB", bytes as f64 / 1_073_741_824.0);
        println!("  - Speed:   {:.2} GB/s", speed);
    }

    println!("\n--------------------------------------------------");
    println!("Zen Engine: Dynamic Lab Validation COMPLETE.");
    
    Ok(())
}
