use crate::types::{FileMetadata, DataType, BusinessTemplate};
use crate::domain::AliasManager;
use std::collections::HashMap;
use colored::Colorize;

pub struct Report<'a> {
    metadata: &'a FileMetadata,
    template: BusinessTemplate,
    aliases: HashMap<String, String>,
}

impl<'a> Report<'a> {
    pub fn new(metadata: &'a FileMetadata) -> Self {
        Self { 
            metadata,
            template: BusinessTemplate::General,
            aliases: HashMap::new(),
        }
    }

    pub fn with_template(mut self, template: BusinessTemplate) -> Self {
        self.template = template;
        self.aliases = AliasManager::get_aliases(template);
        self
    }

    pub fn render(&self) {
        self.render_executive();
        self.render_detailed_cards();
    }

    pub fn render_executive(&self) {
        println!("\n{}", "📊 EXECUTIVE DATA SUMMARY".bold().underline());
        println!("File: {}", self.metadata.file_name.cyan());
        
        let mut years: Vec<u32> = Vec::new();
        let mut indicators = Vec::new();
        let mut locations_count = 0;
        let mut total_nulls = 0u64;
        let mut total_values = 0u64;

        for s in &self.metadata.column_stats {
            if s.count > 0 {
                total_nulls += s.null_count;
                total_values += s.count + s.null_count;
            }
            
            if s.is_categorical {
                let name_lower = s.name.to_lowercase();
                if name_lower.contains("indicator") || name_lower.contains("indicador") || name_lower.contains("series name") {
                    for (cat, _) in &s.top_categories {
                        if !cat.is_empty() && !indicators.contains(cat) {
                            indicators.push(cat.clone());
                        }
                    }
                }
                if name_lower.contains("country") || name_lower.contains("país") || name_lower.contains("location") {
                   locations_count = locations_count.max(s.distinct_count);
                }
            } else {
                // Heuristic for years: numeric name between 1900 and 2100
                if let Ok(year) = s.name.trim_matches('"').parse::<u32>() {
                    if year > 1900 && year < 2100 {
                        years.push(year);
                    }
                }
            }
        }

        years.sort();
        
        let path_str = &self.metadata.file_name;
        let is_dir = path_str.ends_with('/') || path_str.ends_with('\\') || !path_str.contains('.');
        
        if is_dir {
            println!("Contexto: {} (Multi-Dataset)", "CARPETA".yellow().bold());
        }

        if !years.is_empty() {
            println!("Periodo: {} - {} ({} años)", 
                years.first().unwrap().to_string().green(), 
                years.last().unwrap().to_string().green(), 
                years.len()
            );
        } else if is_dir {
            println!("Periodo: {}", "Variable / Varias series".dimmed());
        }

        if !indicators.is_empty() {
            if indicators.len() > 1 {
                println!("Estudios: {} detectados (ej: {})", indicators.len(), indicators[0].cyan().bold());
            } else {
                println!("Estudio: {}", indicators[0].cyan().bold());
            }
        }
        
        if locations_count > 0 {
            println!("Entidades: {} (Países/Regiones)", locations_count.to_string().yellow());
        }

        let integrity = if total_values > 0 {
            (1.0 - (total_nulls as f64 / total_values as f64)) * 100.0
        } else { 0.0 };
        
        let avg_health: f64 = if !self.metadata.column_stats.is_empty() {
            self.metadata.column_stats.iter().map(|s| s.health_score).sum::<f64>() / self.metadata.column_stats.len() as f64
        } else { 0.0 };

        if self.metadata.column_stats.is_empty() {
            println!("Contexto: {}", "SIN DATOS COMPATIBLES".red().bold());
            println!("INFO: Use {} para incluir carpetas ocultas/ruidosas.", "--full-scan".cyan());
        }

        println!("Calidad: {}% de integridad de celdas", 
            if integrity > 80.0 { format!("{:.1}", integrity).green() } else { format!("{:.1}", integrity).yellow() }
        );
        println!("Salud Global: {:.0}/100 {}", 
            avg_health,
            if avg_health > 80.0 { "EXCELENTE".green().bold() } else if avg_health > 50.0 { "REGULAR".yellow().bold() } else if avg_health > 0.0 { "CRÍTICA".red().bold() } else { "N/A".dimmed() }
        );
        println!("{}", "─".repeat(50).dimmed());
    }

    pub fn render_detailed_cards(&self) {
        println!("\n{}", "📊 DETAILED DATA PROFILING".bold().underline());

        for s in &self.metadata.column_stats {
            self.render_universal_card(s);
        }
    }

    fn render_universal_card(&self, s: &crate::types::ColumnStats) {
        let col_name = if s.name.trim().is_empty() { "_unnamed_".to_string() } else { s.name.clone() };
        let display_name = if col_name.starts_with('"') && col_name.ends_with('"') {
            col_name.clone()
        } else {
            format!("\"{}\"", col_name)
        };

        let width: usize = 64;
        let header = format!("┌─ {} ", display_name);
        let border_len = width.saturating_sub(header.chars().count() + 1);
        println!("\n{}{}{}┐", header.bold().cyan(), "─".repeat(border_len).bold().cyan(), "─");

        // Profile Line
        let profile = if s.is_categorical {
            match s.schema.as_ref().map(|sch| sch.data_type) {
                Some(DataType::IP) => "Network: IPv4/v6",
                Some(DataType::MAC) => "Hardware: MAC",
                Some(DataType::ID) => "System ID/Key",
                Some(DataType::Boolean) => "Logic: Boolean",
                Some(DataType::Email) => "Personal: Email",
                Some(DataType::URL) => "Web: URL",
                Some(DataType::UUID) => "System: UUID",
                Some(DataType::PhoneNumber) => "Contact: Phone",
                Some(DataType::JSON) => "Data: Nested JSON",
                _ => "Categorical",
            }
        } else {
            match s.schema.as_ref().map(|sch| sch.data_type) {
                Some(DataType::Date) => "Temporal/Date",
                _ => "Numeric/Float",
            }
        };
        println!("  │  PERFIL: [{}]  |  Memory: {:.1} KB", profile.yellow(), s.estimated_memory_kb);

        // Filling Ratio (Progress Bar)
        let fill_pct = s.filling_ratio * 100.0;
        let bar_width = 20;
        let filled = (s.filling_ratio * bar_width as f64) as usize;
        let bar = format!("{}{}", "█".repeat(filled).green(), "░".repeat(bar_width - filled).dimmed());
        println!("  │  Llenado: {fill_pct:>5.1}% {bar} ({nulls} Nulls)", 
            fill_pct = fill_pct,
            bar = bar,
            nulls = s.null_count.to_string().red()
        );

        println!("  ├{}", "─".repeat(width - 3).dimmed());

        // Stats & Frequency Row
        let uniqueness = if s.distinct_count > 0 {
            let ratio = (s.distinct_count as f64 / s.count as f64) * 100.0;
            if ratio > 90.0 { "Primaria".green().to_string() }
            else if ratio > 20.0 { "Alta".yellow().to_string() }
            else { "Baja".cyan().to_string() }
        } else {
            if s.count > 0 && !s.is_categorical { "Estimada".cyan().to_string() } else { "Baja".cyan().to_string() }
        };
        let unique_info = if s.distinct_count > 0 { s.distinct_count.to_string() } else { "-".to_string() };
        
        println!("  │  {:<26} │  TOP FRECUENCIA (Moda)", AliasManager::translate("ESTADÍSTICA DE VALORES", &self.aliases));
        println!("  │  {}: {:<5} ({:<13})    │  1. {:<20}", 
            AliasManager::translate("Unique", &self.aliases),
            unique_info, 
            AliasManager::translate(&uniqueness, &self.aliases),
            if !s.top_categories.is_empty() { 
                let (k, v) = &s.top_categories[0];
                let pct = (*v as f64 / s.count.max(1) as f64) * 100.0;
                let k_trunc = if k.len() > 12 { format!("{}...", &k[..9]) } else { k.clone() };
                format!("\"{}\" ({:>.0}%)", k_trunc, pct)
            } else { "-".to_string() }
        );
        
        let detection = if s.is_constant { "Constante" } else if s.is_monotonic_inc { "Incremental (ID?)" } else if s.is_monotonic_dec { "Decremental" } else { "Variable" };
        
        let mut top_entries = s.top_categories.iter().skip(1).take(2); // Skip the first one as it's already printed
        let get_top = |entry: Option<&(String, u64)>| {
            if let Some((k, v)) = entry {
                let pct = (*v as f64 / s.count.max(1) as f64) * 100.0;
                let k_trunc = if k.len() > 12 { format!("{}...", &k[..9]) } else { k.clone() };
                format!("\"{}\" ({:>.0}%)", k_trunc, pct)
            } else {
                "".to_string()
            }
        };

        println!("  │  Detección: {:<18} │  2. {:<20}", detection.dimmed(), get_top(top_entries.next()));
        
        let outliers = if s.skewness.abs() > 3.0 { ">3σ Suspected" } else { "0 detected" };
        println!("  │  Outliers: {:<19} │  3. {:<20}", outliers, get_top(top_entries.next()));

        println!("  ├{}", "─".repeat(width - 3).dimmed());

        // Visual / Distribution
        if !s.is_categorical {
            let hist_chars = ['\u{2581}','\u{2582}','\u{2583}','\u{2584}','\u{2585}','\u{2586}','\u{2587}','\u{2588}'];
            let max_bucket = s.histogram.iter().copied().max().unwrap_or(1).max(1);
            let hist: String = s.histogram.iter().map(|&b| {
                let idx = ((b as f64 / max_bucket as f64) * 7.0) as usize;
                hist_chars[idx.min(7)]
            }).collect();

            let dist_desc = if s.skewness.abs() < 0.5 { "Simétrica" } else if s.skewness > 0.0 { "Sesgo Positivo" } else { "Sesgo Negativo" };

            println!("  │  COMPORTAMIENTO VISUAL ({})", AliasManager::translate("Distribución", &self.aliases));
            println!("  │  {}: {:<10} | {}: {:<10} | {}: {:<10}", 
                AliasManager::translate("Min", &self.aliases),
                self.format_compact(s.min), 
                AliasManager::translate("Avg", &self.aliases),
                self.format_compact(s.mean), 
                AliasManager::translate("Max", &self.aliases),
                self.format_compact(s.max)
            );
            println!("  │  Visual: {} ({})  |  Health: {:.0}%", hist.dimmed(), dist_desc.cyan(), s.health_score);
        } else {
            println!("  │  Health Score: {:.0}%", s.health_score);
            for warn in &s.integrity_warnings {
                println!("  │  {} {}", "⚠".yellow(), warn.yellow());
            }
        }
        
        println!("  └{}", "─".repeat(width - 3).bold().cyan());
    }

    fn format_compact(&self, v: f64) -> String {
        if v.is_nan() || v.is_infinite() || v == f64::MAX || v == f64::MIN {
            return "-".to_string();
        }
        if v.abs() >= 1_000_000_000.0 {
            format!("{:.1}B", v / 1_000_000_000.0)
        } else if v.abs() >= 1_000_000.0 {
            format!("{:.1}M", v / 1_000_000.0)
        } else if v.abs() >= 1_000.0 {
            format!("{:.1}K", v / 1_000.0)
        } else {
            format!("{:.2}", v)
        }
    }

    pub fn render_detailed_cards_end() {} // Dummy
}
