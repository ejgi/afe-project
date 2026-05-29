use anyhow::Result;
use colored::Colorize;
use crate::history::HistoryManager;

pub fn handle_history(action: crate::cli::HistoryAction) -> Result<()> {
    let hm = HistoryManager::new();
    match action {
        crate::cli::HistoryAction::List { path: filter_path } => {
            println!("\n{} {}", "📜".bold(), "SCAN HISTORY".bold().underline());
            let list = hm.list(filter_path.as_deref());
            if list.is_empty() {
                println!("  No scans recorded yet.");
            } else {
                for (i, entry) in list.iter().enumerate() {
                    println!("  [{:>3}] {} — {} ({} rows)", 
                        i.to_string().dimmed(), 
                        entry.file_path, 
                        entry.scanned_at, 
                        entry.metadata.row_count
                    );
                }
            }
            println!();
        },
        crate::cli::HistoryAction::Show { index, path: filter_path } => {
            let list = hm.list(filter_path.as_deref());
            if let Some(entry) = list.get(index) {
                println!("\n{} Full Scan Details: {}", "🔍".bold(), entry.file_path.yellow().bold());
                println!("  Timestamp  : {}", entry.scanned_at);
                println!("  Row Count  : {}", entry.metadata.row_count);
                println!("  File Hash  : {}", entry.file_hash.as_deref().unwrap_or("N/A"));
                // Implement more detailed rendering if metadata is accessible
            } else {
                println!("{} No scan found at index {}.", "WARN:".yellow(), index);
            }
        },
        crate::cli::HistoryAction::Compare { a, b, path: filter_path } => {
            let list = hm.list(filter_path.as_deref());
            if let (Some(ea), Some(eb)) = (list.get(a), list.get(b)) {
                println!("\n{} COMPARING SCANS: {} vs {}", "⚖️".bold(), a.to_string().cyan(), b.to_string().magenta());
                println!("{:<20} | {:<40} | {:<40}", "Metric".bold(), format!("Scan #{}", a), format!("Scan #{}", b));
                println!("{}", "-".repeat(100).dimmed());
                println!("{:<20} | {:<40} | {:<40}", "Rows", ea.metadata.row_count, eb.metadata.row_count);
                println!("{:<20} | {:<40} | {:<40}", "Timestamp", ea.scanned_at, eb.scanned_at);
            } else {
                println!("{} One or both scan indices are invalid.", "WARN:".yellow());
            }
        },
        crate::cli::HistoryAction::Delete { file } => {
            hm.delete_for(&file)?;
            println!("{} History for '{}' cleared.", "INFO:".blue(), file);
        },
    }
    Ok(())
}
