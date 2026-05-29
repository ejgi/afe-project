use zen_engine::{VirtualDataset, AnalysisOptions, HardwareMode};
use colored::*;

fn main() -> anyhow::Result<()> {
    let path = std::path::PathBuf::from("/home");
    
    // REPLICA EXACTA DE LAS OPCIONES EN zen-search/src-tauri/src/lib.rs
    let mut options = AnalysisOptions {
        hardware_mode: HardwareMode::HDD, // FORZADO A HDD
        no_index: true,
        ..Default::default()
    };
    options.hardware_mode = HardwareMode::HDD;

    println!("\n{} {}", "🔍".bold(), "REPLICA ZEN-SEARCH: Test de Extracción en /home (Modo HDD)".bold().underline());
    println!("{} Inicializando dataset en: {}", "Target:".dimmed(), path.display());
    
    let start_init = std::time::Instant::now();
    let dataset = VirtualDataset::new(&path, &options)?;
    let init_duration = start_init.elapsed();
    
    let file_count = dataset.get_engine_count();
    println!("{} {} archivos detectados en {:.2?}", "Motor:".dimmed(), file_count, init_duration);

    if file_count == 0 {
        println!("\n{} El motor no ha detectado ningún archivo en /home.", "⚠️ ALERTA:".yellow().bold());
        return Ok(());
    }

    println!("{} Iniciando modo NITRO (Silencioso - Máxima Velocidad)...", "Status:".dimmed());
    
    use indicatif::{ProgressBar, ProgressStyle};
    let pb = ProgressBar::new(file_count as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
        .progress_chars("#>-"));

    let start_ext = std::time::Instant::now();
    let mut total_bytes = 0u64;
    let mut global_map = std::collections::HashMap::new();
    let mut files_scanned = 0;
    
    // --- BUCLE DE MISIÓN: 10 MINUTOS (SILENCIOSO) ---
    for (idx, engine) in dataset.engines.iter().enumerate() {
        // AUTO-STOP AL MINUTO 10 (600 seg)
        if start_ext.elapsed().as_secs() >= 600 {
            pb.finish_with_message("TEST FINALIZADO (Límite 10 min)");
            break;
        }

        let file_size = engine.mmap.len();
        total_bytes += file_size as u64;
        files_scanned += 1;

        // Actualizar mensaje solo cada 500 archivos para no frenar al motor
        if idx % 500 == 0 {
            pb.set_message(format!("{:.2} GB", total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)));
        }

        if let Ok(local_map) = engine.extract_ips(None, zen_engine::types::IpScanMode::Both) {
            for (ip, meta) in local_map {
                let entry = global_map.entry(ip).or_insert(0);
                *entry += meta.hits;
            }
        }
        pb.inc(1);
    }
    
    if !pb.is_finished() {
        pb.finish_with_message("Dataset completado (Modo Nitro).");
    }

    let ext_duration = start_ext.elapsed();
    let total_gb = total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    let avg_throughput = total_gb / ext_duration.as_secs_f64();

    println!("\n{} {}", "📊".bold(), "REPORTE DE IMPACTO (MARATÓN 10 MIN)".bold().underline());
    println!("{} {:.2?} / 10:00", "Tiempo Total:".dimmed(), ext_duration);
    println!("{} {} de {}", "Progreso:".dimmed(), files_scanned, file_count);
    println!("{} {:.2} GB", "Volumen:".bold(), total_gb);
    println!("{} {:.2} MB/s", "Throughput:".bold(), avg_throughput * 1024.0);
    println!("{} {} IPs únicas", "Cosecha:".green().bold(), global_map.len());
    
    if global_map.len() > 0 {
        println!("\nTop 5 IPs encontradas:");
        let mut sorted_res: Vec<_> = global_map.into_iter().collect();
        sorted_res.sort_by(|a, b| b.1.cmp(&a.1));
        for (ip, count) in sorted_res.iter().take(5) {
            println!("[HIT] {:?} (Count: {})", ip, count);
        }
    }

    Ok(())
}
