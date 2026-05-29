use std::sync::Arc;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IpValue {
    V4(u32),
    V6(u128),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IpMetadata {
    pub hits: usize,
    pub noise_hits: usize,
}

pub struct IpScanner {
    inner: Arc<dyn IpScannerKernel + Send + Sync>,
}

pub trait IpScannerKernel: Send + Sync {
    /// Escanea un bloque de memoria completo buscando Direcciones IPv4/IPv6
    /// Devuelve un mapa de (Valor IP -> Metadatos de Frecuencia/Ruido)
    fn extract(&self, data: &[u8], mode: crate::types::IpScanMode) -> HashMap<IpValue, IpMetadata>;
}

impl IpScanner {
    pub fn new() -> Self {
        let kernel: Arc<dyn IpScannerKernel + Send + Sync> = if is_x86_feature_detected!("avx2") {
            #[cfg(target_arch = "x86_64")]
            { Arc::new(Avx2IpKernel) }
            #[cfg(not(target_arch = "x86_64"))]
            { Arc::new(ScalarIpKernel) }
        } else {
            Arc::new(ScalarIpKernel)
        };

        Self { inner: kernel }
    }

    pub fn extract(&self, data: &[u8], mode: crate::types::IpScanMode) -> HashMap<IpValue, IpMetadata> {
        self.inner.extract(data, mode)
    }
}

/// Fallback Escalar para ARM o procesadores antiguos
struct ScalarIpKernel;
impl IpScannerKernel for ScalarIpKernel {
    fn extract(&self, data: &[u8], mode: crate::types::IpScanMode) -> HashMap<IpValue, IpMetadata> {
        use crate::types::IpScanMode;
        let mut results: HashMap<IpValue, IpMetadata> = HashMap::with_capacity(data.len() / 256 + 1);
        let mut i = 0;
        let mut last_ip_end = 0;
        
        match mode {
            IpScanMode::V4 => {
                while i < data.len() {
                    if data[i] == b'.' {
                        let actual_start = find_ip_start_v4(data, i);
                        if actual_start >= last_ip_end && actual_start < i {
                            if let Some((ip, len)) = parse_ipv4(data, actual_start) {
                                let noise = is_noise_context(data, actual_start, len);
                                let entry = results.entry(IpValue::V4(ip)).or_insert(IpMetadata { hits: 0, noise_hits: 0 });
                                entry.hits += 1;
                                if noise { entry.noise_hits += 1; }
                                last_ip_end = actual_start + len;
                                i = last_ip_end;
                                continue;
                            }
                        }
                    }
                    i += 1;
                }
            }
            IpScanMode::V6 => {
                while i < data.len() {
                    if data[i] == b':' {
                        let actual_start = find_ip_start_v6(data, i);
                        if actual_start >= last_ip_end && actual_start < i {
                            if let Some((ip, len)) = parse_ipv6_nitro(data, actual_start) {
                                let noise = is_noise_context(data, actual_start, len);
                                let entry = results.entry(IpValue::V6(ip)).or_insert(IpMetadata { hits: 0, noise_hits: 0 });
                                entry.hits += 1;
                                if noise { entry.noise_hits += 1; }
                                last_ip_end = actual_start + len;
                                i = last_ip_end;
                                continue;
                            }
                        }
                    }
                    i += 1;
                }
            }
            IpScanMode::Both => {
                while i < data.len() {
                    let b = data[i];
                    if b == b'.' {
                        let actual_start = find_ip_start_v4(data, i);
                        if actual_start >= last_ip_end && actual_start < i {
                            if let Some((ip, len)) = parse_ipv4(data, actual_start) {
                                let noise = is_noise_context(data, actual_start, len);
                                let entry = results.entry(IpValue::V4(ip)).or_insert(IpMetadata { hits: 0, noise_hits: 0 });
                                entry.hits += 1;
                                if noise { entry.noise_hits += 1; }
                                last_ip_end = actual_start + len;
                                i = last_ip_end;
                                continue;
                            }
                        }
                    } else if b == b':' {
                        let actual_start = find_ip_start_v6(data, i);
                        if actual_start >= last_ip_end && actual_start < i {
                            if let Some((ip, len)) = parse_ipv6_nitro(data, actual_start) {
                                let noise = is_noise_context(data, actual_start, len);
                                let entry = results.entry(IpValue::V6(ip)).or_insert(IpMetadata { hits: 0, noise_hits: 0 });
                                entry.hits += 1;
                                if noise { entry.noise_hits += 1; }
                                last_ip_end = actual_start + len;
                                i = last_ip_end;
                                continue;
                            }
                        }
                    }
                    i += 1;
                }
            }
        }
        results
    }
}

#[cfg(target_arch = "x86_64")]
struct Avx2IpKernel;

#[cfg(target_arch = "x86_64")]
impl Avx2IpKernel {
    #[target_feature(enable = "avx2")]
    unsafe fn extract_impl(&self, data: &[u8], mode: crate::types::IpScanMode) -> HashMap<IpValue, IpMetadata> {
        use std::arch::x86_64::*;
        use crate::types::IpScanMode;
        let mut results: HashMap<IpValue, IpMetadata> = HashMap::with_capacity(data.len() / 256 + 1);
        let mut last_ip_end = 0;
        
        let chunk_size = 32;
        let limit = data.len().saturating_sub(data.len() % chunk_size);
        let dot_vec = _mm256_set1_epi8(b'.' as i8);
        let col_vec = _mm256_set1_epi8(b':' as i8);
        
        // Loop Unswitching: Branch once outside the loop
        match mode {
            IpScanMode::V4 => {
                for i in (0..limit).step_by(chunk_size) {
                    let chunk = _mm256_loadu_si256(data.as_ptr().add(i) as *const __m256i);
                    let mut dot_mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, dot_vec)) as u32;
                    while dot_mask != 0 {
                        let bit = dot_mask.trailing_zeros();
                        let abs_pos = i + bit as usize;
                        if abs_pos >= last_ip_end {
                            let actual_start = find_ip_start_v4(data, abs_pos);
                            if actual_start >= last_ip_end && actual_start < abs_pos {
                                if let Some((ip, len)) = parse_ipv4(data, actual_start) {
                                    let noise = is_noise_context(data, actual_start, len);
                                    let entry = results.entry(IpValue::V4(ip)).or_insert(IpMetadata { hits: 0, noise_hits: 0 });
                                    entry.hits += 1;
                                    if noise { entry.noise_hits += 1; }
                                    last_ip_end = actual_start + len;
                                }
                            }
                        }
                        dot_mask &= !(1u32 << bit);
                    }
                }
                for i in limit..data.len() {
                    if data[i] == b'.' {
                        let actual_start = find_ip_start_v4(data, i);
                        if actual_start >= last_ip_end && actual_start < i {
                            if let Some((ip, len)) = parse_ipv4(data, actual_start) {
                                let noise = is_noise_context(data, actual_start, len);
                                let entry = results.entry(IpValue::V4(ip)).or_insert(IpMetadata { hits: 0, noise_hits: 0 });
                                entry.hits += 1;
                                if noise { entry.noise_hits += 1; }
                                last_ip_end = actual_start + len;
                            }
                        }
                    }
                }
            }
            IpScanMode::V6 => {
                for i in (0..limit).step_by(chunk_size) {
                    let chunk = _mm256_loadu_si256(data.as_ptr().add(i) as *const __m256i);
                    let mut col_mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, col_vec)) as u32;
                    while col_mask != 0 {
                        let bit = col_mask.trailing_zeros();
                        let abs_pos = i + bit as usize;
                        if abs_pos >= last_ip_end {
                            let actual_start = find_ip_start_v6(data, abs_pos);
                            if actual_start >= last_ip_end && actual_start < abs_pos {
                                if let Some((ip, len)) = parse_ipv6_nitro(data, actual_start) {
                                    let noise = is_noise_context(data, actual_start, len);
                                    let entry = results.entry(IpValue::V6(ip)).or_insert(IpMetadata { hits: 0, noise_hits: 0 });
                                    entry.hits += 1;
                                    if noise { entry.noise_hits += 1; }
                                    last_ip_end = actual_start + len;
                                }
                            }
                        }
                        col_mask &= !(1u32 << bit);
                    }
                }
                for i in limit..data.len() {
                    if data[i] == b':' {
                        let actual_start = find_ip_start_v6(data, i);
                        if actual_start >= last_ip_end && actual_start < i {
                            if let Some((ip, len)) = parse_ipv6_nitro(data, actual_start) {
                                let noise = is_noise_context(data, actual_start, len);
                                let entry = results.entry(IpValue::V6(ip)).or_insert(IpMetadata { hits: 0, noise_hits: 0 });
                                entry.hits += 1;
                                if noise { entry.noise_hits += 1; }
                                last_ip_end = actual_start + len;
                            }
                        }
                    }
                }
            }
            IpScanMode::Both => {
                for i in (0..limit).step_by(chunk_size) {
                    let chunk = _mm256_loadu_si256(data.as_ptr().add(i) as *const __m256i);
                    let mut dot_mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, dot_vec)) as u32;
                    while dot_mask != 0 {
                        let bit = dot_mask.trailing_zeros();
                        let abs_pos = i + bit as usize;
                        if abs_pos >= last_ip_end {
                            let actual_start = find_ip_start_v4(data, abs_pos);
                            if actual_start >= last_ip_end && actual_start < abs_pos {
                                if let Some((ip_val, length)) = parse_ipv4(data, actual_start) {
                                    let noise = is_noise_context(data, actual_start, length);
                                    let entry = results.entry(IpValue::V4(ip_val)).or_insert(IpMetadata { hits: 0, noise_hits: 0 });
                                    entry.hits += 1;
                                    if noise { entry.noise_hits += 1; }
                                    last_ip_end = actual_start + length;
                                }
                            }
                        }
                        dot_mask &= !(1u32 << bit);
                    }
                    let mut col_mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, col_vec)) as u32;
                    while col_mask != 0 {
                        let bit = col_mask.trailing_zeros();
                        let abs_pos = i + bit as usize;
                        if abs_pos >= last_ip_end {
                            let actual_start = find_ip_start_v6(data, abs_pos);
                            if actual_start >= last_ip_end && actual_start < abs_pos {
                                if let Some((ip_val, length)) = parse_ipv6_nitro(data, actual_start) {
                                    let noise = is_noise_context(data, actual_start, length);
                                    let entry = results.entry(IpValue::V6(ip_val)).or_insert(IpMetadata { hits: 0, noise_hits: 0 });
                                    entry.hits += 1;
                                    if noise { entry.noise_hits += 1; }
                                    last_ip_end = actual_start + length;
                                }
                            }
                        }
                        col_mask &= !(1u32 << bit);
                    }
                }
                for i in limit..data.len() {
                    let b = data[i];
                    if b == b'.' {
                        let actual_start = find_ip_start_v4(data, i);
                        if actual_start >= last_ip_end && actual_start < i {
                            if let Some((ip, len)) = parse_ipv4(data, actual_start) {
                                let noise = is_noise_context(data, actual_start, len);
                                let entry = results.entry(IpValue::V4(ip)).or_insert(IpMetadata { hits: 0, noise_hits: 0 });
                                entry.hits += 1;
                                if noise { entry.noise_hits += 1; }
                                last_ip_end = actual_start + len;
                            }
                        }
                    } else if b == b':' {
                        let actual_start = find_ip_start_v6(data, i);
                        if actual_start >= last_ip_end && actual_start < i {
                            if let Some((ip, len)) = parse_ipv6_nitro(data, actual_start) {
                                let noise = is_noise_context(data, actual_start, len);
                                let entry = results.entry(IpValue::V6(ip)).or_insert(IpMetadata { hits: 0, noise_hits: 0 });
                                entry.hits += 1;
                                if noise { entry.noise_hits += 1; }
                                last_ip_end = actual_start + len;
                            }
                        }
                    }
                }
            }
        }
        
        results
    }
}

#[cfg(target_arch = "x86_64")]
impl IpScannerKernel for Avx2IpKernel {
    fn extract(&self, data: &[u8], mode: crate::types::IpScanMode) -> HashMap<IpValue, IpMetadata> {
        unsafe { self.extract_impl(data, mode) }
    }
}

#[inline(always)]
fn find_ip_start_v4(data: &[u8], dot_pos: usize) -> usize {
    let mut actual_start = dot_pos;
    while actual_start > 0 {
        let b = data[actual_start - 1];
        if b >= b'0' && b <= b'9' {
            actual_start -= 1;
        } else {
            break;
        }
    }
    actual_start
}

#[inline(always)]
fn find_ip_start_v6(data: &[u8], col_pos: usize) -> usize {
    let mut actual_start = col_pos;
    while actual_start > 0 {
        let b = data[actual_start - 1];
        // Walk back through hex digits AND ':' to find the true start of the IPv6 address
        if b.is_ascii_hexdigit() || b == b':' {
            actual_start -= 1;
        } else {
            break;
        }
    }
    actual_start
}

#[inline(always)]
fn parse_ipv4(data: &[u8], start: usize) -> Option<(u32, usize)> {
    if start > 0 && data[start - 1].is_ascii_alphanumeric() { return None; }
    
    let mut octets = 0;
    let mut total_ip = 0u32;
    let mut current_val = 0u32;
    let mut has_digit = false;
    let mut i = start;
    
    while i < data.len() {
        let b = data[i];
        if b.is_ascii_digit() {
            current_val = current_val * 10 + (b - b'0') as u32;
            if current_val > 255 { return None; }
            has_digit = true;
            i += 1;
        } else if b == b'.' {
            if !has_digit { return None; }
            octets += 1;
            if octets == 4 { return None; }
            total_ip = (total_ip << 8) | current_val;
            current_val = 0;
            has_digit = false;
            i += 1;
        } else {
            break;
        }
    }
    
    if octets == 3 && has_digit {
        if i < data.len() && data[i].is_ascii_alphanumeric() { return None; }
        total_ip = (total_ip << 8) | current_val;
        return Some((total_ip, i - start));
    }
    None
}

/// Nitro IPv6 parser: zero-allocation, direct byte state machine.
/// Mirrors parse_ipv4: no String, no heap, no stdlib parser.
/// Handles: full (8 groups), compressed (::), leading/trailing ::.
#[inline(always)]
fn parse_ipv6_nitro(data: &[u8], start: usize) -> Option<(u128, usize)> {
    if start > 0 && data[start - 1].is_ascii_alphanumeric() { return None; }

    let mut groups = [0u16; 8]; // groups before ::
    let mut tail   = [0u16; 8]; // groups after ::
    let mut g = 0usize;
    let mut t = 0usize;
    let mut compress = false;
    let mut cur = 0u32;
    let mut digits = 0usize;
    let mut colons = 0usize;
    let mut i = start;

    // Handle leading :: (e.g. ::1, ::ffff:192.0.2.1)
    if i + 1 < data.len() && data[i] == b':' && data[i + 1] == b':' {
        compress = true;
        colons = 2;
        i += 2;
    }

    while i < data.len() {
        let b = data[i];
        let hex = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            b':' => {
                if digits == 0 {
                    // Consecutive colon → second ':' of '::'
                    if compress { return None; } // two '::' → invalid
                    compress = true;
                    colons += 1;
                    i += 1;
                    continue;
                }
                // End of a group, check next byte for '::'
                if compress { if t >= 8 { return None; } tail[t] = cur as u16; t += 1; }
                else        { if g >= 8 { return None; } groups[g] = cur as u16; g += 1; }
                cur = 0; digits = 0; colons += 1; i += 1;
                continue;
            }
            _ => break,
        };
        cur = (cur << 4) | hex as u32;
        digits += 1;
        if digits > 4 { return None; } // group too long (e.g. fffff)
        i += 1;
    }

    // Flush last group
    if digits > 0 {
        if compress { if t >= 8 { return None; } tail[t] = cur as u16; t += 1; }
        else        { if g >= 8 { return None; } groups[g] = cur as u16; g += 1; }
    }

    let total = g + t;

    // Structural validation
    if colons < 2 { return None; }           // minimum 2 colons for IPv6
    if compress {
        if total > 7 { return None; }         // '::' must replace ≥1 group
    } else {
        if g != 8 { return None; }            // full form: must have exactly 8 groups
    }

    // Trailing boundary: must not end mid-token
    if i < data.len() && (data[i].is_ascii_alphanumeric() || data[i] == b':') { return None; }

    // Assemble u128 from stack arrays — zero heap allocation
    let zeros = 8 - total;
    let mut result = 0u128;
    for j in 0..g      { result = (result << 16) | groups[j] as u128; }
    for _  in 0..zeros { result <<= 16; }
    for j in 0..t      { result = (result << 16) | tail[j]   as u128; }

    Some((result, i - start))
}
static IS_NON_PRINTABLE: [u8; 256] = {
    let mut table = [1u8; 256];
    let mut i = 0x20;
    while i <= 0x7E {
        table[i as usize] = 0;
        i += 1;
    }
    table[0x09] = 0; // \t
    table[0x0A] = 0; // \n
    table[0x0D] = 0; // \r
    table
};

#[inline(always)]
fn is_noise_context(data: &[u8], pos: usize, len: usize) -> bool {
    // Window: pos - 16..pos + len + 16
    let start = pos.saturating_sub(16);
    let end = (pos + len + 16).min(data.len());
    let window = &data[start..end];
    
    // Quick escape for "::" noise (unspecified IPv6)
    if len <= 2 && data[pos] == b':' { return true; }
    
    let mut non_printable = 0usize;
    for &b in window {
        non_printable += IS_NON_PRINTABLE[b as usize] as usize;
    }
    
    // If more than 35% of the context is non-printable binary, it's noise.
    // Optimization: avoid floating point, use integer comparison
    non_printable * 100 > window.len() * 35
}
