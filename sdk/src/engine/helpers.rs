use crate::engine::BigDataEngine;
use crate::utils::{parse_numeric_fast, parse_date_fast};

pub(crate) fn extract_field<'a>(engine: &BigDataEngine, line: &'a [u8], col: usize) -> Option<&'a [u8]> {
    if engine.rfc_4180 {
        return extract_field_rfc4180(engine, line, col);
    }
    let mut idx = 0;
    let mut start = 0;
    let delim = engine.delimiter;
    for (j, &b) in line.iter().enumerate() {
        if b == delim {
            if idx == col {
                return Some(&line[start..j]);
            }
            idx += 1;
            start = j + 1;
        }
    }
    if idx == col {
        return Some(&line[start..]);
    }
    None
}

pub(crate) fn extract_field_rfc4180<'a>(engine: &BigDataEngine, line: &'a [u8], col: usize) -> Option<&'a [u8]> {
    let mut idx = 0;
    let mut start = 0;
    let mut in_quotes = false;
    let delim = engine.delimiter;
    
    let mut i = 0;
    while i < line.len() {
        let b = line[i];
        if b == b'"' {
            in_quotes = !in_quotes;
        } else if b == delim && !in_quotes {
            if idx == col {
                let mut field = &line[start..i];
                if engine.strip_quotes && field.starts_with(b"\"") && field.ends_with(b"\"") && field.len() >= 2 {
                    field = &field[1..field.len()-1];
                }
                return Some(field);
            }
            idx += 1;
            start = i + 1;
        }
        i += 1;
    }
    
    if idx == col {
        let mut field = &line[start..];
        if engine.strip_quotes && field.starts_with(b"\"") && field.ends_with(b"\"") && field.len() >= 2 {
            field = &field[1..field.len()-1];
        }
        return Some(field);
    }
    None
}

pub(crate) fn row_matches(
    engine: &BigDataEngine,
    line: &[u8],
    filter_col: Option<usize>,
    filter_min: Option<f64>,
    filter_max: Option<f64>,
    filter_text_col: Option<usize>,
    filter_text: Option<&str>,
    filter_ast: Option<&crate::filter::Expr>,
    date_col: Option<usize>,
    date_from: Option<u32>,
    date_to: Option<u32>,
) -> bool {
    // Numeric filter
    if let Some(fc) = filter_col {
        let val = engine.extract_field(line, fc)
            .and_then(|f| parse_numeric_fast(f));
        match val {
            None => return false,
            Some(v) => {
                if let Some(min) = filter_min { if v < min { return false; } }
                if let Some(max) = filter_max { if v > max { return false; } }
            }
        }
    }
    // Date filter
    if let Some(dc) = date_col {
        let val = engine.extract_field(line, dc)
            .and_then(|f| parse_date_fast(f));
        match val {
            None => return false,
            Some(v) => {
                if let Some(min) = date_from { if v < min { return false; } }
                if let Some(max) = date_to { if v > max { return false; } }
            }
        }
    }
    // Text filter
    if let (Some(tc), Some(needle)) = (filter_text_col, filter_text) {
        let needle_bytes = needle.as_bytes();
        if let Some(field) = engine.extract_field(line, tc) {
            if engine.zenscan.scan(field, needle_bytes).is_empty() {
                return false;
            }
        } else {
            return false;
        }
    }
    // AST Filter
    if let Some(ast) = filter_ast {
        let extract_fn = |col: usize| -> Option<&str> {
            engine.extract_field(line, col).and_then(|b| std::str::from_utf8(b).ok())
        };
        if !crate::filter::evaluate_row(ast, &extract_fn) {
            return false;
        }
    }
    true
}

pub(crate) fn is_json_format(engine: &BigDataEngine) -> bool {
    match engine.forced_format.as_deref() {
        Some("json") | Some("ndjson") => true,
        Some("csv") | Some("logs") => false,
        _ => {
            let first_char = engine.mmap.iter().find(|&&b| !b.is_ascii_whitespace());
            first_char == Some(&b'{')
        }
    }
}

pub(crate) fn auto_detect_preamble(engine: &BigDataEngine) -> usize {
    if engine.mmap.len() < 10 { return 0; }
    
    let mut data_start = 0;
    if engine.mmap.len() >= 3 && &engine.mmap[0..3] == b"\xEF\xBB\xBF" {
        data_start = 3;
    }

    let sample_len = 8192.min(engine.mmap.len());
    let sample = &engine.mmap[data_start..sample_len];
    let _sample_str = String::from_utf8_lossy(sample);
    
    // Simplificado para la versión estable
    0
}
