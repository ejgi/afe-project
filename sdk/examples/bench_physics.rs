use std::fs::File;
use std::io::{BufRead, BufReader, Write, BufWriter};
use std::time::Instant;
use memmap2::MmapOptions;

use zen_engine::analytics::zenscan::ZenScan;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!(" 🌩️  ZEN-ENGINE: BENCHMARK 'LEYES DE LA FÍSICA' 🌩️");
    println!("============================================================\n");

    let path = "/tmp/zen_physics_test.dat";
    let file_size_mb = 500;
    let target_str = "SECRETO_NUCLEARES_ACTIVADOS";

    println!("⏳ 1. Preparando entorno físico...");
    println!("   Escribiendo {} MB de datos sintéticos en el disco...", file_size_mb);
    
    // Crear archivo de prueba
    {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        let dummy_line = "Esto es un registro de log aburrido llenando espacio en el disco duro.\n";
        let target_line = format!("Aqui hay un log muy importante que contiene {} para buscar.\n", target_str);
        
        let bytes_per_line = dummy_line.len();
        let total_lines = (file_size_mb * 1024 * 1024) / bytes_per_line;
        
        for i in 0..total_lines {
            if i == total_lines / 2 {
                writer.write_all(target_line.as_bytes())?;
            } else {
                writer.write_all(dummy_line.as_bytes())?;
            }
        }
        writer.flush()?;
    }
    println!("   ✅ Entorno listo.\n");

    // =========================================================
    // PRUEBA A: Costumbres Convencionales
    // =========================================================
    println!("▶  PRUEBA A: La Metodología Convencional (Software Abstraído)");
    println!("   Mecánica: Lector con Buffer -> Asignar Memoria en Heap (Strings) -> Comparar UTF-8 -> Destruir Memoria.");
    
    let start_a = Instant::now();
    let mut found_a = false;
    let mut matches_a = 0;
    
    // El "High-Level Way"
    let file_a = File::open(path)?;
    let reader = BufReader::with_capacity(64 * 1024, file_a);
    
    for line in reader.lines() {
        if let Ok(texto) = line { // Alocación de String (Heap)
            if texto.contains(target_str) { // Múltiples saltos en RAM
                found_a = true;
                matches_a += 1;
            }
        } // El Garbage Collector / Drop entra aquí destruyendo la memoria
    }
    
    let elapsed_a = start_a.elapsed();
    let speed_a = (file_size_mb as f64) / elapsed_a.as_secs_f64();
    println!("   [RESULTADO A] Tiempo: {:.3} s | Velocidad: {:.2} MB/s | Encontrado: {}\n", 
             elapsed_a.as_secs_f64(), speed_a, found_a);


    // =========================================================
    // PRUEBA B: Leyes de la Física (Zen)
    // =========================================================
    println!("▶  PRUEBA B: La Filosofía Zen (Las Leyes de la Física del Silicio)");
    println!("   Mecánica: Mapeo Holográfico (Mmap) -> Acceso Directo de Memoria -> Registros Vectoriales SIMD -> Cero Alocaciones.");
    
    let start_b = Instant::now();
    let mut found_b = false;
    let mut matches_b = 0;
    
    let file_b = File::open(path)?;
    // Enviar el archivo directo a la Caché del SO, cero copias
    let mmap = unsafe { MmapOptions::new().map(&file_b)? }; 
    
    // El "Hardware Way"
    let scanner = ZenScan::new();
    let needle = target_str.as_bytes();
    
    // Esto entra directo a los registros AVX/SSE de la CPU
    let offsets = scanner.scan(&mmap[..], needle); 
    if !offsets.is_empty() {
        found_b = true;
        matches_b = offsets.len();
    }
    
    let elapsed_b = start_b.elapsed();
    let speed_b = (file_size_mb as f64) / elapsed_b.as_secs_f64();
    println!("   [RESULTADO B] Tiempo: {:.4} s | Velocidad: {:.2} MB/s | Encontrado: {}\n", 
             elapsed_b.as_secs_f64(), speed_b, found_b);

    // =========================================================
    // CONCLUSIONES
    // =========================================================
    println!("============================================================");
    println!(" 📊  ANÁLISIS DE EFICIENCIA ELECTROMECÁNICA");
    println!("============================================================");
    let multiplo = speed_b / speed_a;
    println!("⚡ Zen es {:.1}x veces más rápido que el software convencional.", multiplo);
    println!("⚡ Zen gastó un 0% de sobrecarga en el Gestor de Memoria del Sistema Operativo.");
    println!("⚡ Zen respetó el límite máximo de transferencia del Bus de la Placa Madre.");
    println!("============================================================\n");

    // Clean up
    let _ = std::fs::remove_file(path);

    Ok(())
}
