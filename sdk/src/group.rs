use colored::*;
use crate::dataset::VirtualDataset;
use crate::filter::Expr;
use anyhow::Result;

pub struct GroupAnalysis;

impl GroupAnalysis {
    pub fn render_group(
        dataset: &mut VirtualDataset,
        by: usize,
        agg: usize,
        filter: Option<&Expr>,
        use_gpu: bool,
    ) -> Result<()> {
        let t_start = std::time::Instant::now();
        let results = dataset.group_by(by, agg, filter, use_gpu)?;

        println!("\n{}", "📊 GROUP BY RESULTS".bold().underline());
        println!("Files:     {}", dataset.engines.len());
        println!("Time:      {} ms\n", t_start.elapsed().as_millis().to_string().cyan());
        
        println!("{:<30} {:>12} {:>15} {:>12} {:>12} {:>12}", 
            "CATEGORY".bold().blue(), 
            "COUNT".bold(), 
            "SUM".bold(), 
            "MEAN".bold(), 
            "MIN".bold(),
            "MAX".bold()
        );
        println!("{}", "─".repeat(95).dimmed());

        let mut entries = results;
        entries.sort_by(|a, b| b.sum.partial_cmp(&a.sum).unwrap_or(std::cmp::Ordering::Equal));

        for g in entries.iter().take(50) {
            println!("{:<30} {:>12} {:>15.2} {:>12.2} {:>12.2} {:>12.2}",
                if g.category.len() > 28 { format!("{:.25}...", g.category) } else { g.category.clone() }.dimmed(),
                g.count.to_string().yellow(),
                g.sum,
                g.mean,
                g.min,
                g.max
            );
        }
        Ok(())
    }

    pub fn render_top(
        dataset: &mut VirtualDataset,
        col: usize,
        limit: usize,
        descending: bool,
        filter: Option<&Expr>,
        use_gpu: bool,
    ) -> Result<()> {
        let t_start = std::time::Instant::now();
        let results = dataset.top_n(col, limit, descending, filter, use_gpu)?;

        println!("\n{}", "🏆 TOP-N RANKING".bold().underline());
        println!("Column:    {} ({})", col, if descending { "DESC" } else { "ASC" });
        println!("Time:      {} ms\n", t_start.elapsed().as_millis().to_string().cyan());

        for (i, row) in results.into_iter().enumerate() {
            println!("| {:>4} | {}",
                (i as u32 + 1).to_string().yellow(),
                row.dimmed()
            );
        }
        Ok(())
    }
}
