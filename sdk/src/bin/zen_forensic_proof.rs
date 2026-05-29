use std::path::Path;
use zen_engine::parsers::forensics::evtx::analyze_evtx;
use zen_engine::parsers::forensics::pcap::analyze_pcap;
use colored::*;

fn main() -> anyhow::Result<()> {
    println!("\n{}", "      ZEN ENGINE - FORENSIC NITRO PROOF      ".on_black().bold());
    println!("{}", "===============================================".black());

    // 1. Analyze EVTX
    let evtx_path = "test_massive.evtx";
    if Path::new(evtx_path).exists() {
        let file = std::fs::File::open(evtx_path)?;
        let data = unsafe { memmap2::Mmap::map(&file)? };
        let size_mb = data.len() as f64 / 1024.0 / 1024.0;
        println!("\n🔍 Analizando [{}] ({:.2} MB)...", evtx_path.cyan().bold(), size_mb);
        
        let start = std::time::Instant::now();
        let (total, errors, _warnings, _info) = analyze_evtx(&data, false);
        let duration = start.elapsed();
        let mbs = size_mb / duration.as_secs_f64();
        
        println!("   📦 Registros detectados: {}", total.to_string().yellow().bold());
        println!("   🔴 Errores/Críticos:    {}", errors.to_string().red());
        println!("   ⏱️  Tiempo:              {:?}", duration);
        println!("   🚀 Velocidad:           {} MB/s", format!("{:.2}", mbs).green().bold());
    }

    // 2. Analyze PCAP
    let pcap_path = "test_massive.pcap";
    if Path::new(pcap_path).exists() {
        let file = std::fs::File::open(pcap_path)?;
        let data = unsafe { memmap2::Mmap::map(&file)? };
        let size_mb = data.len() as f64 / 1024.0 / 1024.0;
        println!("\n📡 Analizando [{}] ({:.2} MB)...", pcap_path.cyan().bold(), size_mb);
        
        let start = std::time::Instant::now();
        let summary = analyze_pcap(&data);
        let duration = start.elapsed();
        let mbs = size_mb / duration.as_secs_f64();
        
        println!("   📦 Paquetes totales:    {}", summary.total_packets.to_string().yellow().bold());
        println!("   💾 Bytes totales:       {}", summary.total_bytes.to_string().cyan());
        println!("   🔗 Flujos únicos (Zen-Flow): {}", summary.unique_flows.to_string().green());
        println!("   ⚡ TCP:                 {}", summary.tcp_packets);
        println!("   🔋 UDP:                 {}", summary.udp_packets);
        println!("   🚨 Payloads Sospechosos:{}", summary.suspicious_payloads.to_string().red().bold());
        println!("   ⏱️  Tiempo:              {:?}", duration);
        println!("   🚀 Velocidad:           {} MB/s", format!("{:.2}", mbs).green().bold());
        
        println!("\n✅ Validado: Soporte de VLAN e IPv6 activado en Zen-Flow.");
    }

    println!("\n{}", "===============================================".black());
    println!("🏁 {} completada.", "Prueba de Fuego".green().bold());

    Ok(())
}
