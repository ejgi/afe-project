use std::path::PathBuf;
use anyhow::{Result, Context};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use crate::dataset::VirtualDataset;
use crate::types::{AnalysisLevel, AnalysisOptions, HardwareMode, Blueprint};
use crate::config::ConfigManager;
use crate::history::HistoryManager;
use crate::report::Report;

#[allow(clippy::too_many_arguments)]
pub fn handle_analyze(
    path: PathBuf, 
    blueprint: Option<PathBuf>, 
    index_zones: bool, 
    use_zones: bool, 
    filter_col: Option<usize>, 
    filter_min: Option<f64>, 
    filter_max: Option<f64>, 
    filter_text_col: Option<usize>, 
    filter_text: Option<String>, 
    filter: Option<String>, 
    date_col: Option<usize>, 
    date_from: Option<String>, 
    date_to: Option<String>, 
    level: AnalysisLevel, 
    mut delimiter: Option<String>, 
    mut no_header: bool, 
    mut regex: Option<String>, 
    mut rfc_4180: bool, 
    mut skip: usize, 
    hash: bool, 
    mut hardware: Option<String>, 
    mut network: bool, 
    no_index: bool, 
    mut chunk_size: Option<usize>, 
    loop_count: usize, 
    mut gpu: bool, 
    mut format: Option<String>, 
    profile: Option<String>, 
    mut threads: Option<usize>, 
    mut no_limit: bool, 
    mut strip_quotes: bool, 
    template: Option<String>,
    full_scan: bool
) -> Result<()> {
    println!("{} Analyzing {}...", "INTEL:".cyan(), path.display().to_string().bold());
    
    if let Some(t) = threads {
        println!("{} Thread Throttling Active: {} cores limit", "INFO:".cyan(), t);
    }
    
    if no_limit {
        println!("{} NITRO MODE ACTIVE: Bypassing all safety limits (100% CPU)", "WARN:".red().bold());
    }

    if full_scan {
        println!("{} FULL SCAN ENABLED: No directory exclusions.", "INFO:".cyan());
    }

    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")?);
    pb.set_message(format!("Scanning {}...", path.display().to_string().cyan()));

    let mut blueprint_obj = if let Some(bp_path) = blueprint {
        let bp_content = std::fs::read_to_string(bp_path)?;
        Some(serde_json::from_str::<Blueprint>(&bp_content)?)
    } else {
        None
    };

    if let Some(profile_name) = profile {
        let cm = ConfigManager::new();
        if let Some(prof) = cm.get_profile(&profile_name) {
            println!("{} Loaded profile: {}", "INTEL:".cyan(), profile_name.green().bold());
            if blueprint_obj.is_none() { blueprint_obj = prof.blueprint; }
            if delimiter.is_none() {
                if let Some(d) = prof.delimiter { delimiter = Some(d.to_string()); }
            }
            if format.is_none() { format = prof.format; }
            if hardware.is_none() { hardware = prof.hardware_mode; }
            if skip == 0 { skip = prof.skip_rows.unwrap_or(0); }
            if !no_header { no_header = !prof.has_header.unwrap_or(true); }
            if !rfc_4180 { rfc_4180 = prof.rfc_4180.unwrap_or(false); }
            if !network { network = prof.enable_network.unwrap_or(false); }
            if !gpu { gpu = prof.gpu.unwrap_or(false); }
            if regex.is_none() { regex = prof.regex_pattern; }
            if chunk_size.is_none() { chunk_size = prof.chunk_size_mb; }
            if threads.is_none() { threads = prof.threads; }
            if !no_limit { no_limit = prof.no_limit.unwrap_or(false); }
            if !strip_quotes { strip_quotes = prof.strip_quotes.unwrap_or(false); }
        } else {
            println!("{} Profile '{}' not found in DB.", "WARN:".yellow(), profile_name);
        }
    }

    let ast = if let Some(expr_str) = filter {
        Some(crate::filter::parse_filter(&expr_str)?)
    } else { None };

    let date_from_u32 = date_from.as_deref().and_then(|s| crate::utils::parse_date_fast(s.as_bytes()));
    let date_to_u32 = date_to.as_deref().and_then(|s| crate::utils::parse_date_fast(s.as_bytes()));

    let hw_mode = match hardware.as_deref().unwrap_or("auto").to_lowercase().as_str() {
        "hdd" => HardwareMode::HDD,
        "ssd" => HardwareMode::SSD,
        _ => HardwareMode::Auto,
    };

    let options = AnalysisOptions {
        level,
        blueprint: blueprint_obj,
        delimiter: delimiter.as_ref().map(|s| s.as_bytes()[0]).or(Some(b',')),
        regex_pattern: regex,
        rfc_4180,
        skip_rows: skip,
        has_header: !no_header,
        enable_network: network,
        chunk_size_mb: chunk_size,
        no_index,
        hardware_mode: hw_mode,
        gpu,
        forced_format: format,
        threads,
        no_limit,
        strip_quotes,
        full_scan,
        ..Default::default()
    };

    let dataset = VirtualDataset::new(&path, &options).context("Failed to load dataset")?;

    if dataset.engines.is_empty() {
        pb.finish_and_clear();
        println!("{} No se detectaron archivos de datos compatibles en {}", "WARN:".yellow().bold(), path.display());
        println!("{} El motor 'Smart Scan' está configurado para omitir carpetas de sistema (.git, node_modules, etc.).", "INFO:".cyan());
        println!("{} Si desea forzar el escaneo de todo el contenido, use {} o elija una subcarpeta específica.", "TIP:".green(), "--full-scan".bold());
        return Ok(());
    }

    let mut computed_hash = None;
    if let Some(first) = dataset.engines.first() {
        let h = crate::utils::compute_hash_fast(&first.mmap);
        computed_hash = Some(h.clone());
        if hash {
            println!("{} Integrity (File 1, BLAKE3): {}", "FORENSIC:".magenta(), h.yellow());
        }
    }

    if let Some(profile_hw) = hardware {
        println!("{} Forcing hardware profile: {}", "INTEL:".cyan(), profile_hw.bold());
    }

    let (progress_tx, progress_rx) = std::sync::mpsc::channel::<crate::types::FileMetadata>();
    
    let render_handle = std::thread::spawn(move || {
        let mut last_update = std::time::Instant::now();
        while let Ok(partial) = progress_rx.recv() {
            if last_update.elapsed().as_millis() > 500 {
                use std::io::Write;
                eprint!("\r{} Processed {} rows...", "LIVE:".green(), partial.row_count.to_string().yellow());
                std::io::stderr().flush().ok();
                last_update = std::time::Instant::now();
            }
        }
        eprintln!("\r{} Finalizing analysis...                ", "LIVE:".green());
    });

    let mut existing_zm = None;
    if use_zones {
        if let Some(first) = dataset.engines.first() {
            match first.load_zone_map() {
                Ok(zm) => existing_zm = Some(zm),
                Err(e) => {
                    eprintln!("{} ZoneMap load failed: {}. Falling back to full scan.", "WARN:".yellow(), e);
                }
            }
        }
    }

    let metadata = dataset.analyze(
        &options, filter_col, filter_min, filter_max, existing_zm.as_ref(),
        filter_text.as_deref(), filter_text_col, ast.as_ref(),
        date_col, date_from_u32, date_to_u32, Some(&progress_tx), loop_count,
    )?;
    
    if index_zones {
        if let Some(first) = dataset.engines.first() {
            if let Ok(zm) = first.build_zone_map(65536, hw_mode) {
                let _ = first.save_zone_map(&zm);
                println!("{} ZoneMap built and saved to .csv.zones", "SUCCESS:".green());
            }
        }
    }
    
    drop(progress_tx);
    let _ = render_handle.join();
    pb.finish_and_clear();

    let report_template = match template.as_deref().unwrap_or("general").to_lowercase().as_str() {
        "finance" => crate::types::BusinessTemplate::Finance,
        "network" => crate::types::BusinessTemplate::Network,
        "cybersecurity" => crate::types::BusinessTemplate::Cybersecurity,
        "sales" => crate::types::BusinessTemplate::Sales,
        _ => crate::types::BusinessTemplate::General,
    };

    let report = Report::new(&metadata).with_template(report_template);
    report.render();

    let hm = HistoryManager::new();
    match hm.record(&path.to_string_lossy(), &metadata, computed_hash, None, None) {
        Ok(_) => println!("{} Scan saved to history DB.", "HIST:".blue().bold()),
        Err(e) => eprintln!("{} Could not save to history: {}", "WARN:".yellow(), e),
    }

    Ok(())
}
