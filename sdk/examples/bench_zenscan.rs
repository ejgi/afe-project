use zen_engine::BigDataEngine;
use zen_engine::types::HardwareMode;
use std::path::Path;
use std::time::Instant;

fn main() {
    let log_path = "/home/archtech/Descargas/prueba/massive_system_logs.csv";
    if !Path::new(log_path).exists() {
        println!("❌ Error: No se encuentra el archivo en {}", log_path);
        return;
    }

    println!("--- Zen-Scan Performance Benchmark ---");
    println!("Archivo: {} ({:.2} MB)", log_path, std::fs::metadata(log_path).unwrap().len() as f64 / 1024.0 / 1024.0);

    let mut engine = BigDataEngine::new(Path::new(log_path), HardwareMode::Auto).unwrap();
    // No necesitamos construir el índice completo para probar ZenScan sobre el mmap crudo
    
    let needle = "usb_err";
    let data = engine.mmap.as_ref();

    // 1. Benchmark Zen-Scan (SIMD)
    let start_simd = Instant::now();
    let matches_simd = engine.zenscan.scan(data, needle.as_bytes());
    let duration_simd = start_simd.elapsed();
    
    // 2. Benchmark Standard Fallback (Scalar/Regex-like)
    // Usamos el ScalarKernel directamente para comparar
    let start_scalar = Instant::now();
    let mut matches_scalar_count = 0;
    let needle_bytes = needle.as_bytes();
    for i in 0..=data.len().saturating_sub(needle_bytes.len()) {
        if data[i..i + needle_bytes.len()].eq_ignore_ascii_case(needle_bytes) {
            matches_scalar_count += 1;
        }
    }
    let duration_scalar = start_scalar.elapsed();

    let size_gb = data.len() as f64 / 1024.0 / 1024.0 / 1024.0;
    let throughput_simd = size_gb / duration_simd.as_secs_f64();
    let throughput_scalar = size_gb / duration_scalar.as_secs_f64();

    println!("\nResultados:");
    println!("- Matches encontrados: {}", matches_simd.len());
    println!("- Zen-Scan (SIMD):   {:?} ({:.2} GB/s)", duration_simd, throughput_simd);
    println!("- Fallback Scalar:   {:?} ({:.2} GB/s)", duration_scalar, throughput_scalar);
    println!("\n🚀 Ganancia de velocidad: {:.1}x", duration_scalar.as_secs_f64() / duration_simd.as_secs_f64());
    
    if matches_simd.len() == matches_scalar_count {
        println!("✅ Verificación de integridad: Coincidencia exacta.");
    } else {
        println!("❌ Error de integridad: SIMD ({}) vs Scalar ({})", matches_simd.len(), matches_scalar_count);
    }
}
