use zen_engine::*;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    println!("🕵️  ZEN ENGINE — NO-CACHE / NO-INDEX BENCHMARK");
    println!("──────────────────────────────────────────────");

    let pcap_dir = "/home/archtech/Descargas/FIRST-2015_Hands-on_Network_Forensics_PCAP";

    // 1. Configurar opciones SIN indexación
    let mut options = AnalysisOptions::default();
    options.no_index = true; // Forzamos a no generar índices ni hashes

    println!("📂 Cargando dataset (Sin Indexación)...");
    let load_start = Instant::now();
    let dataset = VirtualDataset::new(std::path::Path::new(pcap_dir), &options)?;
    let load_time = load_start.elapsed();
    let total_mb = dataset.get_total_size_mb();
    println!("   Carga inicial: {:.2}s", load_time.as_secs_f64());

    // 2. Ejecutar IoC Search (Esto obligará al HDD a leer cada byte por primera vez)
    let iocs = vec![
        "192.168.".to_string(),
        "exploit".to_string(),
        "malware".to_string(),
    ];

    println!("\n🔍 Escaneando disco 'en frío' (Full Scan)...");
    let scan_start = Instant::now();
    let results = dataset.search_iocs(&iocs, 100)?;
    let scan_time = scan_start.elapsed();

    let mb_searched = total_mb;
    let throughput_mb = mb_searched / scan_time.as_secs_f64();

    println!("\n📊 RESULTADOS (Fuerza Bruta SIMD):");
    println!("   Datos procesados: {:.1} MB", mb_searched);
    println!("   Tiempo:           {:.3}s", scan_time.as_secs_f64());
    println!("   Throughput:       {:.1} MB/s", throughput_mb);
    println!("   Matches:          {}", results.len());

    println!("\n✅ VEREDICTO (Perspectiva Real de Hardware):");
    if throughput_mb > 50.0 {
        println!("   🟢 Excelente: Superas los 50 MB/s en un HDD real sin caché.");
    } else {
        println!("   🟡 Limitado por I/O: {:.1} MB/s (Tu disco es el cuello de botella).", throughput_mb);
    }

    Ok(())
}
