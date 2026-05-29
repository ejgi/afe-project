use std::time::Instant;
use std::fs::File;
use std::io::{Write, BufWriter};
use zen_engine::{dataset::VirtualDataset, AnalysisOptions};

fn main() -> anyhow::Result<()> {
    let test_path = "stress_test_ips.log";
    println!("🚀 Generando archivo de prueba de 1,000,000 de IPs...");
    
    {
        let file = File::create(test_path)?;
        let mut writer = BufWriter::new(file);
        for i in 0..1_000_000 {
            // Generar IPs variadas para probar el agrupamiento
            let ip = format!("{}.{}.{}.{}\n", 192, 168, (i % 50), (i % 255));
            writer.write_all(ip.as_bytes())?;
            if i % 100_000 == 0 {
                writer.write_all(b"Some random text between logs to test robustness\n");
            }
        }
        writer.flush()?;
    }

    println!("✅ Archivo generado. Iniciando extracción Industrial (v2)...");
    
    let options = AnalysisOptions {
        no_index: true,
        ..Default::default()
    };
    
    let dataset = VirtualDataset::new(std::path::Path::new(test_path), &options)?;
    
    let start = Instant::now();
    let results = dataset.extract_ips(zen_engine::types::IpScanMode::Both).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let elapsed = start.elapsed();
    
    println!("--------------------------------------------------");
    println!("⏱️ TIEMPO DE EXTRACCIÓN: {:?}", elapsed);
    println!("📈 IPs ÚNICAS ENCONTRADAS: {}", results.len());
    println!("📊 TOP 5 RESULTADOS:");
    
    for (i, r) in results.iter().take(5).enumerate() {
        println!("  {}. IP: {} | Hits: {} | Origins: {}", i+1, r.ip, r.count, r.top_files.len());
    }
    
    println!("--------------------------------------------------");
    println!("✨ PRUEBA COMPLETADA CON ÉXITO.");
    
    // Cleanup
    let _ = std::fs::remove_file(test_path);
    Ok(())
}
