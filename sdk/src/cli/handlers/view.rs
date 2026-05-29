use std::path::PathBuf;
use anyhow::Result;
use colored::Colorize;
use crate::engine::BigDataEngine;
use crate::types::HardwareMode;

pub fn handle_view(path: PathBuf, start: usize, count: usize, hardware: Option<String>) -> Result<()> {
    let hw_mode = match hardware.as_deref().unwrap_or("auto").to_lowercase().as_str() {
        "hdd" => HardwareMode::HDD,
        "ssd" => HardwareMode::SSD,
        _ => HardwareMode::Auto,
    };
    let mut engine = BigDataEngine::new(&path, hw_mode)?;
    engine.build_index()?;
    
    let rows = engine.get_rows(start, start + count);
    println!("{} Viewing rows {} to {} of {}", 
        "INTEL:".cyan(), 
        start.to_string().yellow(), 
        (start + rows.len()).to_string().yellow(),
        path.display()
    );
    println!("{}", "-".repeat(50).dimmed());
    
    for (i, row) in rows.iter().enumerate() {
        println!("| {:>8} | {}", (start + i + 1).to_string().dimmed(), row);
    }
    println!("  └{}", "─".repeat(34).bold().dimmed());
    Ok(())
}
