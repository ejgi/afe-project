use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;
use zen_engine::{dataset::VirtualDataset, AnalysisOptions, HardwareMode, types::*};

// ─── State ───────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct AppState {
    pub search_folders: Mutex<Vec<String>>,
    pub hardware_mode: Mutex<String>,
    pub is_cancelled: std::sync::atomic::AtomicBool,
}

// ─── Payloads ─────────────────────────────────────────────────────────────────





#[derive(Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub folders: Vec<String>,
    pub hardware_mode: String,
    pub max_results: usize,
}

/// Wrapper de respuesta: resultados paginados + metadata del escaneo completo
#[derive(Serialize)]
struct IpScanResult {
    results: Vec<IpFrequency>,   // Top-N resultados para la UI
    total_unique: usize,          // Total de IPs únicas encontradas
    total_hits: u64,              // Suma de todos los hits
    truncated: bool,              // true si hay más resultados que los enviados
}

/// Save settings to app state.
#[tauri::command]
fn save_settings(settings: AppSettings, state: State<'_, AppState>) {
    *state.search_folders.lock().unwrap() = settings.folders;
    *state.hardware_mode.lock().unwrap() = settings.hardware_mode;
}

/// Load current settings.
#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> AppSettings {
    AppSettings {
        folders: state.search_folders.lock().unwrap().clone(),
        hardware_mode: state.hardware_mode.lock().unwrap().clone(),
        max_results: 200,
    }
}

/// Open a file with the system default application.
#[tauri::command]
fn open_file(path: String) {
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(&path).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", &path])
        .spawn();
}

/// Cancel the current scan.
#[tauri::command]
fn cancel_scan(state: tauri::State<'_, AppState>) {
    state.is_cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
}


/// Zen-IOC: Massively parallel IP extraction and frequency grouping.
#[tauri::command]
async fn extract_ips_cmd(mode: String, state: State<'_, AppState>) -> Result<IpScanResult, String> {
    let start_total = std::time::Instant::now();
    let folders = {
        let guard = state.search_folders.lock().unwrap();
        guard.clone()
    };
    let hw_str = {
        let guard = state.hardware_mode.lock().unwrap();
        guard.clone()
    };

    if folders.is_empty() {
        return Err("No directories configured for extraction.".to_string());
    }

    let hw_mode = if hw_str == "ssd" { HardwareMode::SSD } else { HardwareMode::HDD };
    let ip_mode = match mode.as_str() {
        "v4" => IpScanMode::V4,
        "v6" => IpScanMode::V6,
        _ => IpScanMode::Both,
    };

    state.is_cancelled.store(false, std::sync::atomic::Ordering::SeqCst);
    let mut all_results = Vec::new();

    for folder in &folders {
        if state.is_cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        let path = std::path::PathBuf::from(folder);
        let options = AnalysisOptions {
            hardware_mode: hw_mode,
            no_index: true,
            ip_scan_mode: ip_mode,
            ..Default::default()
        };

        let dataset = VirtualDataset::new(&path, &options).map_err(|e| e.to_string())?;

        match dataset.extract_ips_cancellable(Some(&state.is_cancelled), ip_mode) {
            Ok(found) => { all_results.extend(found); }
            Err(e) => { eprintln!("Extraction error in {}: {}", folder, e); }
        }
    }

    // Consolidar resultados si hay múltiples carpetas
    let final_v = if folders.len() > 1 {
        use std::collections::HashMap;
        let mut global_map: HashMap<String, IpFrequency> = HashMap::new();
        
        for item in all_results {
            let entry = global_map.entry(item.ip.clone()).or_insert(IpFrequency {
                ip: item.ip.clone(),
                count: 0,
                country_code: item.country_code.clone(),
                country_name: item.country_name.clone(),
                is_noise: true, // assume noise until proven otherwise
                top_files: Vec::new(),
            });
            entry.count += item.count;
            if !item.is_noise { entry.is_noise = false; } // if any hit is clean, the IP is clean
            entry.top_files.extend(item.top_files);
            entry.top_files.sort_by(|a, b| b.count.cmp(&a.count));
            entry.top_files.truncate(50);
        }
        
        let mut v: Vec<IpFrequency> = global_map.into_values().collect();
        v.sort_by(|a, b| b.count.cmp(&a.count));
        v
    } else {
        all_results
    };

    // Calcular métricas del dataset completo ANTES de truncar
    let total_unique = final_v.len();
    let total_hits: u64 = final_v.iter().map(|r| r.count as u64).sum();
    let truncated = total_unique > 1_000;

    let elapsed = start_total.elapsed();
    println!("🚀 [BACKEND] Cálculo completado en: {:?}. IPs Únicas: {} | Hits: {} | Truncado: {}",
        elapsed, total_unique, total_hits, truncated);

    // Truncar a top-1000 para el puente IPC — el CSV siempre exporta el dataset completo
    let results = if truncated {
        final_v.into_iter().take(1_000).collect()
    } else {
        final_v
    };

    Ok(IpScanResult { results, total_unique, total_hits, truncated })
}

/// Guardar reporte de IPs a archivo CSV
#[tauri::command]
async fn save_report_cmd(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

// ─── App Entry ────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            save_settings,
            get_settings,
            open_file,
            extract_ips_cmd,
            save_report_cmd,
            cancel_scan
        ])
        .setup(|_app| {
            // // Start hidden — only show on hotkey
            // if let Some(win) = app.get_webview_window("main") {
            //     win.hide().ok();
            // }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running zen-search");
}
