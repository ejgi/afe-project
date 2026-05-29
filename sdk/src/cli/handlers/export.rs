use std::path::PathBuf;
use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use crate::engine::BigDataEngine;
use crate::types::{AnalysisLevel, AnalysisOptions, HardwareMode};

#[allow(clippy::too_many_arguments)]
pub fn handle_export(
    path: PathBuf,
    output: PathBuf,
    filter_col: Option<usize>,
    filter_min: Option<f64>,
    filter_max: Option<f64>,
    filter_text_col: Option<usize>,
    filter_text: Option<String>,
    filter: Option<String>,
    date_col: Option<usize>,
    date_from: Option<String>,
    date_to: Option<String>,
    use_zones: bool,
    output_format: String,
    delimiter: Option<String>,
    no_header: bool,
    regex: Option<String>,
    rfc_4180: bool,
    skip: usize,
    hardware: Option<String>,
) -> Result<()> {
    let hw_mode = match hardware.as_deref().unwrap_or("auto").to_lowercase().as_str() {
        "hdd" => HardwareMode::HDD,
        "ssd" => HardwareMode::SSD,
        _ => HardwareMode::Auto,
    };
    
    let mut engine = BigDataEngine::new(&path, hw_mode)?;
    if let Some(d) = delimiter.as_ref() {
        if !d.is_empty() { 
            engine.delimiter = match d.as_str() {
                "\\t" => b'\t',
                "\\n" => b'\n',
                "\\r" => b'\r',
                _ => d.as_bytes()[0],
            };
        }
    }
    engine.has_header = !no_header;
    engine.rfc_4180 = rfc_4180; engine.skip_rows = skip;
    engine.build_index()?;

    let mut zm = None;
    if use_zones {
        match engine.load_zone_map() {
            Ok(loaded_zm) => zm = Some(loaded_zm),
            Err(e) => {
                eprintln!("{} ZoneMap for '{}' load failed: {}. Procedural search only.", "WARN:".yellow(), engine.path().display(), e);
            }
        }
    }

    println!("{} Exporting to {} (format: {})...", "INTEL:".cyan(), output.display(), output_format.bold());

    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.magenta} [{elapsed_precise}] {msg}")?);
    pb.set_message("Filtering and writing rows...");
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let options = AnalysisOptions {
        level: AnalysisLevel::Basic,
        delimiter: Some(engine.delimiter),
        regex_pattern: regex,
        rfc_4180: engine.rfc_4180,
        skip_rows: skip,
        has_header: !no_header,
        hardware_mode: hw_mode,
        gpu: false,
        ..Default::default()
    };

    let ast = if let Some(expr_str) = filter {
        Some(crate::filter::parse_filter(&expr_str)?)
    } else { None };

    let date_from_u32 = date_from.as_deref().and_then(|s| crate::utils::parse_date_fast(s.as_bytes()));
    let date_to_u32 = date_to.as_deref().and_then(|s| crate::utils::parse_date_fast(s.as_bytes()));

    let exported = engine.export_rows(
        &output, &output_format, options, filter_col, filter_min, filter_max,
        filter_text_col, filter_text.as_deref(), zm.as_ref(), ast.as_ref(),
        date_col, date_from_u32, date_to_u32,
    )?;

    pb.finish_with_message(format!(
        "{} Exported {} rows → {}",
        "SUCCESS:".green(),
        exported.to_string().bold(),
        output.display()
    ));
    Ok(())
}
