use zen_engine::{dataset::VirtualDataset, AnalysisOptions};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let log_dir = args.get(1).map(|s| s.as_str()).unwrap_or("/var/log");
    
    println!("🔍 ZEN-IOC FORENSICS | Buscando 192.168.1.0/24 en: {}", log_dir);
    
    let options = AnalysisOptions {
        no_index: true,
        ..Default::default()
    };
    
    let dataset = VirtualDataset::new(std::path::Path::new(log_dir), &options)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    
    println!("📈 Escaneando archivos (incluyendo logs binarios de Journald)...");
    let results = dataset.extract_ips(zen_engine::types::IpScanMode::Both)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    
    // Filtrar por el rango 192.168.1.x
    let target_range = "192.168.1.";
    let filtered: Vec<_> = results.iter()
        .filter(|r| r.ip.starts_with(target_range))
        .collect();
        
    println!("--------------------------------------------------");
    if filtered.is_empty() {
        println!("⚠️ No se han encontrado IPs de la red 192.168.1.x en los logs.");
        println!("Nota: Es posible que no haya tráfico local registrado en los logs de este sistema.");
    } else {
        println!("✨ RESULTADOS ENCONTRADOS:");
        for r in filtered {
            println!("IP: {:15} | Hits Totales: {:<5}", r.ip, r.count);
            for f in &r.top_files {
                println!("  └─ File: {} ({} hits)", f.path, f.count);
            }
        }
    }
    println!("--------------------------------------------------");
    
    Ok(())
}
