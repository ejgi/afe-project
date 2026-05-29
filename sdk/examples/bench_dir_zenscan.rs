use zen_engine::VirtualDataset;
use zen_engine::types::{AnalysisOptions, HardwareMode};
use std::path::Path;
use std::time::Instant;

fn main() {
    let dir_path = "/home/archtech/Descargas/prueba/";
    if !Path::new(dir_path).exists() {
        println!("❌ Error: No se encuentra la carpeta en {}", dir_path);
        return;
    }

    println!("--- Zen-Scan Directory Stress Test ---");
    println!("Carpeta: {}", dir_path);

    let options = AnalysisOptions {
        hardware_mode: HardwareMode::Auto,
        no_index: true, // Queremos probar búsqueda RAW sobre mmap
        ..Default::default()
    };

    let start_init = Instant::now();
    let dataset = VirtualDataset::new(dir_path, &options).unwrap();
    let duration_init = start_init.elapsed();
    
    println!("Dataset inicializado con {} archivos en {:?}", dataset.engines.len(), duration_init);
    
    let total_size_mb = dataset.get_total_size_mb();
    println!("Tamaño total: {:.2} MB", total_size_mb);

    let needle = "aicwf_usb_bus_txdata";
    println!("\nBuscando patrón: '{}'...", needle);

    let start_search = Instant::now();
    // search_raw = true para forzar el uso de ZenScan sin índices
    let results = dataset.search(needle, None, 1000, true, false, false, false).unwrap();
    let duration_search = start_search.elapsed();

    println!("\nResultados:");
    println!("- Coincidencias totales (primeras 1000): {}", results.len());
    println!("- Tiempo de búsqueda total: {:?}", duration_search);
    
    let throughput = (total_size_mb / 1024.0) / duration_search.as_secs_f64();
    println!("🚀 Rendimiento global: {:.2} GB/s", throughput);

    if !results.is_empty() {
        println!("\nÚltimo resultado encontrado:");
        let last = &results[results.len() - 1];
        println!("[{}] Row {}: {}", last.file_name, last.row_index, last.content);
    }
}
