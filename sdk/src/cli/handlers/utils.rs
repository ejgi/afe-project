use anyhow::Result;
use colored::Colorize;
use crate::utils::get_hardware_specs;

pub fn handle_scan_hardware() -> Result<()> {
    println!("\n{}", "🕵️ ZEN HARDWARE SCANNER".bold().underline());
    let specs = get_hardware_specs();
    
    println!("{:<20} {}", "OS:".dimmed(), format!("{} {}", specs.os_name, specs.os_version).yellow());
    println!("{:<20} {}", "CPU:".dimmed(), format!("{} ({} cores)", specs.cpu_brand, specs.cpu_cores).cyan());
    println!("{:<20} {}", "RAM:".dimmed(), format!("{:.2} GB Total ({:.2} GB Free)", specs.total_memory_gb, specs.free_memory_gb).green());
    println!("{:<20} {}", "Storage:".dimmed(), specs.storage_type.bold());

    // GPU Scan
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });
    
    let adapters: Vec<_> = instance.enumerate_adapters(wgpu::Backends::all());
    
    if !adapters.is_empty() {
        println!("\n{}", "🌐 SOFTWARE GPU ADAPTERS (API Ready)".bold());
        for adapter in &adapters {
            let info = adapter.get_info();
            println!("{:<20} {} [{:?}]", "Adapter:".dimmed(), info.name.magenta(), info.backend);
            println!("{:<20} {:?}", "  Type:".dimmed(), info.device_type);
        }
    }

    if let Some(gpu) = specs.gpu {
        println!("\n{}", "🔌 PHYSICAL GPU HARDWARE (Detected via OS)".bold());
        println!("{:<20} {}", "Model:".dimmed(), gpu.model.yellow());
        println!("{:<20} {}", "Vendor:".dimmed(), gpu.vendor.cyan());
        println!("{:<20} {}", "Type:".dimmed(), gpu.device_type.bold());
        if let Some(cores) = gpu.cores {
           println!("{:<20} {}", "Cores (EU/Compute):".dimmed(), cores.to_string().green().bold());
        }
        if let Some(freq) = gpu.frequency_mhz {
           println!("{:<20} {}", "Power (Clock):".dimmed(), format!("{} MHz", freq).blue().bold());
        }
    } else if adapters.is_empty() {
        println!("{:<20} {}", "GPU:".dimmed(), "None detected".red());
    }
    
    println!("\n{}", "💡 Zen Strategy Recommended:".bold());
    if specs.storage_type == "HDD" {
        println!("- Parallelism: {} (Low, for HDD protection)", "LOCKED".red());
        println!("- Strategy: Row-by-row / Segmented scan (Bypassing seeks)");
    } else {
        println!("- Parallelism: {} (High, for Maximum Speed)", "UNLOCKED".green());
        println!("- Strategy: ZoneMaps & Memory Mapping (Instant Access)");
    }
    println!();
    Ok(())
}
