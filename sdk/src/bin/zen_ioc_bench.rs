use zen_engine::*;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    println!("🕵️  ZEN ENGINE — CLI IoC BENCHMARK (sin GUI)");
    println!("──────────────────────────────────────────────");

    let pcap_dir = "/home/archtech/Descargas/FIRST-2015_Hands-on_Network_Forensics_PCAP";

    // 1. Cargar dataset
    let load_start = Instant::now();
    let options = AnalysisOptions::default();
    let dataset = VirtualDataset::new(std::path::Path::new(pcap_dir), &options)?;
    let load_time = load_start.elapsed();
    let total_mb = dataset.get_total_size_mb();
    let engine_count = dataset.get_engine_count();
    println!("📂 Dataset cargado: {:.1} MB en {:.2}s ({} archivos)", total_mb, load_time.as_secs_f64(), engine_count);

    // 2. Ejecutar IoC Search
    let iocs = vec![
        "192.168.".to_string(),
        "exploit".to_string(),
        "malware".to_string(),
    ];

    println!("\n🔍 Ejecutando búsqueda IoC ({} patrones) sobre {:.1} MB...", iocs.len(), total_mb);
    let scan_start = Instant::now();
    let results = dataset.search_iocs(&iocs, 1000)?;
    let scan_time = scan_start.elapsed();

    // Calcular throughput en base al tamaño real del dataset
    let throughput_mb = total_mb / scan_time.as_secs_f64();
    let time_ns = scan_time.as_nanos();

    println!("\n📊 RESULTADOS:");
    println!("   Bytes escaneados: {:.1} MB", total_mb);
    println!("   TIME:             {} ns", time_ns);
    println!("   TIME (segundos):  {:.3}s", scan_time.as_secs_f64());
    println!("   Throughput:       {:.1} MB/s", throughput_mb);
    println!("   Matches IoC:      {}", results.len());

    if !results.is_empty() {
        println!("\n🎯 Primeros 5 matches:");
        for m in results.iter().take(5) {
            let preview = if m.content.len() > 80 { &m.content[..80] } else { &m.content };
            println!("   [{} L{}] {}", m.file_name.split('/').last().unwrap_or("?"), m.row_index, preview);
        }
    } else {
        println!("\n   (sin matches en estos patrones)");
    }

    println!("\n✅ VEREDICTO:");
    if throughput_mb > 100.0 {
        println!("   🟢 Motor óptimo en HDD: {:.0} MB/s", throughput_mb);
    } else if throughput_mb > 50.0 {
        println!("   🟡 Motor aceptable: {:.0} MB/s (algo de contención)", throughput_mb);
    } else {
        println!("   🔴 Throughput bajo: {:.0} MB/s — posible thrashing", throughput_mb);
    }

    Ok(())
}
