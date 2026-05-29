use crate::types::*;
use crate::engine::BigDataEngine;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use anyhow::Result;
use walkdir::WalkDir;
use crate::dataset::VirtualDataset;

pub(crate) fn new_impl<P: AsRef<Path>>(path: P, options: &AnalysisOptions) -> Result<VirtualDataset> {
    let path_ref = path.as_ref();
    
    let is_hdd = match options.hardware_mode {
        HardwareMode::Auto => crate::utils::is_rotational(path_ref),
        HardwareMode::HDD => true,
        HardwareMode::SSD => false,
    };

    let mut engines = Vec::new();
    let full_scan = options.full_scan;

    if path_ref.is_dir() {
        // Optimization: Use filter_entry to prune noisy directories (metadata only)
        let walk = WalkDir::new(path_ref)
            .into_iter()
            .filter_entry(move |e| {
                if full_scan { return true; }
                if !e.file_type().is_dir() { return true; }
                let name = e.file_name().to_string_lossy();
                // Prune common noisy folders that slow down root scans
                !name.starts_with('.') && 
                name != "node_modules" && 
                name != "target" && 
                name != "dist" && 
                name != "build" &&
                name != "vendor"
            })
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file());

        if is_hdd {
            #[cfg(unix)]
            use std::os::unix::fs::MetadataExt;

            let mut entries: Vec<_> = walk.map(|e| {
                #[cfg(unix)]
                let ino = e.metadata().map(|m| m.ino()).unwrap_or(0);
                #[cfg(not(unix))]
                let ino = 0;
                (e.into_path(), ino)
            }).collect();
            
            entries.sort_by_key(|e| e.1);

            for (p, _) in entries {
                if let Ok(Some(engine)) = build_engine_if_supported(&p, options) {
                    engines.push(engine);
                }
            }
        } else {
            let entries: Vec<_> = walk.map(|e| e.into_path()).collect();
            
            let results: Vec<BigDataEngine> = entries.into_par_iter().filter_map(|p| {
                match build_engine_if_supported(&p, options) {
                    Ok(Some(engine)) => Some(engine),
                    _ => None,
                }
            }).collect();
            
            engines = results;
        }
    } else {
        if let Some(engine) = build_engine_if_supported(path_ref, options)? {
            engines.push(engine);
        }
    }

    Ok(VirtualDataset {
        engines,
        base_path: path_ref.to_path_buf(),
    })
}

pub(crate) fn build_engine_if_supported(p: &Path, options: &AnalysisOptions) -> Result<Option<BigDataEngine>> {
    let filename = p.file_name().map(|f| f.to_string_lossy().to_lowercase()).unwrap_or_default();
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    
    let is_supported = match options.forced_format.as_deref() {
        Some(_) => true,
        None => {
            let s = ext == "csv" || ext == "json" || ext == "ndjson" || ext == "log" || 
                    ext == "pcap" || ext == "evtx" || ext == "bin" || ext == "dat" || ext == "img" ||
                    filename.contains("log") || filename.starts_with("snort.log");
            s || !p.is_dir()
        }
    };

    if is_supported {
        let mut engine = BigDataEngine::new(p, options.hardware_mode)?;
        engine.delimiter = options.delimiter.unwrap_or(b',');
        engine.strip_quotes = options.strip_quotes;
        if !engine.is_compressed {
            engine.rfc_4180 = options.rfc_4180 || engine.mmap.iter().take(8192).any(|&b| b == b'"');
        } else {
            engine.rfc_4180 = options.rfc_4180;
        }
        engine.skip_rows = if options.skip_rows == 0 { engine.auto_detect_preamble() } else { options.skip_rows };
        engine.has_header = options.has_header;
        engine.forced_format = options.forced_format.clone();
        if !options.no_index {
            engine.build_index()?;
        }
        Ok(Some(engine))
    } else {
        Ok(None)
    }
}

pub(crate) fn find_files_impl(
    base_path: &Path,
    query: &str,
    case_sensitive: bool,
    dirs_only: bool,
    files_only: bool,
    limit: usize,
    hardware_mode: HardwareMode,
) -> Result<Vec<(PathBuf, u64, bool)>> {
    let is_hdd = match hardware_mode {
        HardwareMode::Auto => crate::utils::is_rotational(base_path),
        HardwareMode::HDD => true,
        HardwareMode::SSD => false,
    };

    let mut exts_to_match: Vec<String> = Vec::new();
    let mut name_query = String::new();
    let mut name_query_lower = String::new();

    for part in query.split_whitespace() {
        let p_lower = part.to_lowercase();
        if p_lower == "type:doc" || p_lower == "type:document" {
            exts_to_match.extend(vec!["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "rtf", "odt", "csv"].into_iter().map(String::from));
        } else if p_lower == "type:vid" || p_lower == "type:video" {
            exts_to_match.extend(vec!["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm"].into_iter().map(String::from));
        } else if p_lower == "type:aud" || p_lower == "type:audio" {
            exts_to_match.extend(vec!["mp3", "wav", "flac", "ogg", "m4a"].into_iter().map(String::from));
        } else if p_lower == "type:img" || p_lower == "type:image" {
            exts_to_match.extend(vec!["jpg", "jpeg", "png", "gif", "bmp", "svg", "webp"].into_iter().map(String::from));
        } else if p_lower == "type:code" {
            exts_to_match.extend(vec!["rs", "ts", "js", "py", "c", "cpp", "h", "go", "java", "cs", "html", "css"].into_iter().map(String::from));
        } else if p_lower.starts_with('.') && p_lower.len() > 1 && !p_lower[1..].contains('.') {
            exts_to_match.push(p_lower[1..].to_string());
        } else {
            if !name_query.is_empty() { name_query.push(' '); name_query_lower.push(' '); }
            name_query.push_str(part);
            name_query_lower.push_str(&p_lower);
        }
    }

    let has_name_query = !name_query.is_empty();
    let has_ext_filter = !exts_to_match.is_empty();

    if is_hdd {
        let mut results: Vec<(PathBuf, u64, bool)> = Vec::new();
        for entry in WalkDir::new(base_path).into_iter().filter_map(|e| e.ok()) {
            if results.len() >= limit { break; }
            let meta = match entry.metadata() { Ok(m) => m, Err(_) => continue };
            let is_dir = meta.is_dir();
            if files_only && is_dir { continue; }
            if dirs_only && !is_dir { continue; }
            let mut ext_matched = true;
            if has_ext_filter {
                if is_dir { ext_matched = false; }
                else {
                    ext_matched = entry.path().extension()
                        .and_then(|s| s.to_str())
                        .map(|ext| exts_to_match.iter().any(|e| ext.eq_ignore_ascii_case(e)))
                        .unwrap_or(false);
                }
            }
            let mut name_matched = true;
            if ext_matched && has_name_query {
                let name = entry.file_name().to_string_lossy();
                name_matched = if case_sensitive { name.contains(&name_query) } 
                               else { name.to_lowercase().contains(&name_query_lower) };
            }
            if ext_matched && name_matched {
                results.push((entry.into_path(), if is_dir { 0 } else { meta.len() }, is_dir));
            }
        }
        Ok(results)
    } else {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let found_count = Arc::new(AtomicUsize::new(0));
        let found_count_clone = Arc::clone(&found_count);

        let results: Vec<(PathBuf, u64, bool)> = WalkDir::new(base_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .take_while(move |_| found_count_clone.load(Ordering::Relaxed) < limit)
            .par_bridge()
            .filter_map(|entry| {
                if found_count.load(Ordering::Relaxed) >= limit { return None; }
                let meta = entry.metadata().ok()?;
                let is_dir = meta.is_dir();
                if files_only && is_dir { return None; }
                if dirs_only && !is_dir { return None; }
                let mut ext_matched = true;
                if has_ext_filter {
                    if is_dir { ext_matched = false; }
                    else {
                        ext_matched = entry.path().extension()
                            .and_then(|s| s.to_str())
                            .map(|ext| exts_to_match.iter().any(|e| ext.eq_ignore_ascii_case(e)))
                            .unwrap_or(false);
                    }
                }
                let mut name_matched = true;
                if ext_matched && has_name_query {
                    let name = entry.file_name().to_string_lossy().to_string();
                    name_matched = if case_sensitive { name.contains(&name_query) } 
                                   else { name.to_lowercase().contains(&name_query_lower) };
                }
                if ext_matched && name_matched {
                    if found_count.fetch_add(1, Ordering::Relaxed) < limit {
                        return Some((entry.into_path(), if is_dir { 0 } else { meta.len() }, is_dir));
                    }
                }
                None
            })
            .collect();
        Ok(results)
    }
}
