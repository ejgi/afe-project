use anyhow::Result;
use colored::Colorize;
use crate::config::{ConfigManager, AnalysisProfile};
use crate::types::Blueprint;

pub fn handle_config(action: crate::cli::ConfigAction) -> Result<()> {
    let cm = ConfigManager::new();
    match action {
        crate::cli::ConfigAction::Create {
            name, description, blueprint, delimiter, format, hardware,
            skip, no_header, rfc_4180, network, gpu, regex, chunk_size, level, threads, no_limit, strip_quotes,
        } => {
            let bp = if let Some(bp_path) = blueprint {
                let c = std::fs::read_to_string(&bp_path)?;
                Some(serde_json::from_str::<Blueprint>(&c)?)
            } else { None };

            let profile = AnalysisProfile {
                description,
                blueprint: bp,
                delimiter: delimiter.as_ref().and_then(|s| s.chars().next()),
                format,
                hardware_mode: hardware,
                skip_rows: skip,
                has_header: if no_header { Some(false) } else { None },
                rfc_4180: if rfc_4180 { Some(true) } else { None },
                enable_network: if network { Some(true) } else { None },
                gpu: if gpu { Some(true) } else { None },
                regex_pattern: regex,
                chunk_size_mb: chunk_size,
                level,
                threads,
                no_limit: if no_limit { Some(true) } else { None },
                strip_quotes: if strip_quotes { Some(true) } else { None },
            };
            cm.save_profile(name.clone(), profile)?;
            println!("{} Profile '{}' saved successfully.", "SUCCESS:".green().bold(), name.yellow().bold());
        },
        crate::cli::ConfigAction::List => {
            println!("\n{} {}", "📂".bold(), "SAVED PROFILES (Internal DB)".bold().underline());
            let list = cm.list_profiles();
            if list.is_empty() {
                println!("  No profiles saved yet.");
                println!("  Tip: use 'config create <name> [options]' to create one.\n");
            } else {
                for (name, desc) in list {
                    match desc {
                        Some(d) => println!("  {} — {}", name.green().bold(), d.dimmed()),
                        None    => println!("  {}", name.green().bold()),
                    }
                }
                println!();
            }
        },
        crate::cli::ConfigAction::Delete { name } => {
            cm.delete_profile(&name)?;
            println!("{} Profile '{}' deleted.", "INFO:".blue(), name);
        },
        crate::cli::ConfigAction::Show { name } => {
            if let Some(profile) = cm.get_profile(&name) {
                println!("\n{} Profile: {}", "🔍".bold(), name.yellow().bold());
                if let Some(ref d) = profile.description {
                    println!("  Description : {}", d.italic());
                }
                println!("  Delimiter   : {}", profile.delimiter.map(|c| c.to_string()).unwrap_or("auto".into()).cyan());
                println!("  Format      : {}", profile.format.as_deref().unwrap_or("auto").cyan());
                println!("  Hardware    : {}", profile.hardware_mode.as_deref().unwrap_or("auto").cyan());
                // Add more details if necessary
            } else {
                println!("{} Profile '{}' not found.", "WARN:".yellow(), name);
            }
        },
    }
    Ok(())
}
