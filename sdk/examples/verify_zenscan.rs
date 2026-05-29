use zen_engine::BigDataEngine;
use zen_engine::types::HardwareMode;
use std::path::Path;

fn main() {
    let test_file = "test_data.csv";
    std::fs::write(test_file, "name,age,city\nAlice,30,New York\nBob,25,Los Angeles\nCharlie,35,Chicago\n").unwrap();
    
    let mut engine = BigDataEngine::new(Path::new(test_file), HardwareMode::Auto).unwrap();
    engine.build_index().unwrap();

    println!("--- Zen-Scan Verification ---");
    
    let needle = "Los Angeles";
    let matches = engine.zenscan.scan(engine.mmap.as_ref(), needle.as_bytes());
    
    println!("Pattern: '{}'", needle);
    println!("Found at offsets: {:?}", matches);
    
    if !matches.is_empty() {
        println!("✅ Zen-Scan successfully found the pattern.");
    } else {
        println!("❌ Zen-Scan failed to find the pattern.");
    }

    let needle_case = "chicago"; // lowercase (should match due to case-insensitivity)
    let matches_case = engine.zenscan.scan(engine.mmap.as_ref(), needle_case.as_bytes());
    println!("Pattern (Case-Insensitive): '{}'", needle_case);
    println!("Found at offsets: {:?}", matches_case);

    if !matches_case.is_empty() {
        println!("✅ Zen-Scan case-insensitivity verified.");
    } else {
        println!("❌ Zen-Scan case-insensitivity FAILED.");
    }

    std::fs::remove_file(test_file).ok();
}
