use std::sync::Arc;

/// Zen-Scan: High-performance SIMD-accelerated filtering engine.
/// Supports multiple architectures via dynamic dispatch.
pub struct ZenScan {
    inner: Arc<dyn ZenScanKernel + Send + Sync>,
}

pub trait ZenScanKernel: Send + Sync {
    /// Scans a block of data for a specific pattern.
    /// Returns a bitmask where 1 indicates a match start.
    fn scan(&self, data: &[u8], pattern: &[u8], case_insensitive: bool) -> Vec<u64>;
}

impl ZenScan {
    pub fn new() -> Self {
        let kernel: Arc<dyn ZenScanKernel + Send + Sync> = if is_x86_feature_detected!("avx512f") {
            #[cfg(target_arch = "x86_64")]
            { Arc::new(Avx512Kernel) }
            #[cfg(not(target_arch = "x86_64"))]
            { Arc::new(ScalarKernel) }
        } else if is_x86_feature_detected!("avx2") {
            #[cfg(target_arch = "x86_64")]
            { Arc::new(Avx2Kernel) }
            #[cfg(not(target_arch = "x86_64"))]
            { Arc::new(ScalarKernel) }
        } else if is_x86_feature_detected!("avx") {
            #[cfg(target_arch = "x86_64")]
            { Arc::new(AvxKernel) }
            #[cfg(not(target_arch = "x86_64"))]
            { Arc::new(ScalarKernel) }
        } else {
            Arc::new(ScalarKernel)
        };

        Self { inner: kernel }
    }

    pub fn scan(&self, data: &[u8], pattern: &[u8]) -> Vec<u64> {
        // Default to case-insensitive for forensics compatibility
        self.inner.scan(data, pattern, true)
    }

    pub fn scan_raw(&self, data: &[u8], pattern: &[u8], case_insensitive: bool) -> Vec<u64> {
        self.inner.scan(data, pattern, case_insensitive)
    }
}

/// --- KERNEL IMPLEMENTATIONS ---

/// Scalar (Universal) Fallback
struct ScalarKernel;
impl ZenScanKernel for ScalarKernel {
    fn scan(&self, data: &[u8], pattern: &[u8], case_insensitive: bool) -> Vec<u64> {
        let mut results = Vec::new();
        if pattern.is_empty() || data.len() < pattern.len() { return results; }
        
        let mut i = 0;
        let p_len = pattern.len();
        while i <= data.len().saturating_sub(p_len) {
            let chunk = &data[i..i + p_len];
            let is_match = if case_insensitive {
                chunk.eq_ignore_ascii_case(pattern)
            } else {
                chunk == pattern
            };
            
            if is_match {
                results.push(i as u64);
            }
            i += 1;
        }
        results
    }
}

/// AVX (v1 - Legacy / Ivy Bridge)
#[cfg(target_arch = "x86_64")]
struct AvxKernel;

#[cfg(target_arch = "x86_64")]
impl AvxKernel {
    #[target_feature(enable = "avx")]
    unsafe fn scan_impl(&self, data: &[u8], pattern: &[u8], case_insensitive: bool) -> Vec<u64> {
        use std::arch::x86_64::*;
        let mut results = Vec::new();
        if pattern.is_empty() || data.len() < pattern.len() { return results; }
        
        if pattern.len() == 1 {
            let p_byte = pattern[0];
            let p_vec = _mm_set1_epi8(p_byte as i8);
            let alt_vec = if case_insensitive && p_byte.is_ascii_alphabetic() {
                Some(_mm_set1_epi8((p_byte ^ 0x20) as i8))
            } else { None };
            
            let chunk_size = 16;
            let limit = data.len().saturating_sub(data.len() % chunk_size);
            for i in (0..limit).step_by(chunk_size) {
                let chunk = _mm_loadu_si128(data.as_ptr().add(i) as *const __m128i);
                let mut mask = _mm_movemask_epi8(_mm_cmpeq_epi8(chunk, p_vec)) as u32;
                if let Some(av) = alt_vec { mask |= _mm_movemask_epi8(_mm_cmpeq_epi8(chunk, av)) as u32; }
                while mask != 0 {
                    let bit = mask.trailing_zeros();
                    results.push((i + bit as usize) as u64);
                    mask &= !(1u32 << bit);
                }
            }
            for i in limit..data.len() {
                if if case_insensitive { data[i].eq_ignore_ascii_case(&p_byte) } else { data[i] == p_byte } {
                    results.push(i as u64);
                }
            }
        } else {
            let p_first = pattern[0];
            let p_second = pattern[1];
            let p1_vec = _mm_set1_epi8(p_first as i8);
            let alt1_vec = if case_insensitive && p_first.is_ascii_alphabetic() { Some(_mm_set1_epi8((p_first ^ 0x20) as i8)) } else { None };
            let p2_vec = _mm_set1_epi8(p_second as i8);
            let alt2_vec = if case_insensitive && p_second.is_ascii_alphabetic() { Some(_mm_set1_epi8((p_second ^ 0x20) as i8)) } else { None };
            
            let chunk_size = 16;
            let limit = data.len().saturating_sub(pattern.len());
            let simd_limit = if limit >= chunk_size { limit - (limit % chunk_size) } else { 0 };

            for i in (0..simd_limit).step_by(chunk_size) {
                let chunk1 = _mm_loadu_si128(data.as_ptr().add(i) as *const __m128i);
                let mut mask1 = _mm_movemask_epi8(_mm_cmpeq_epi8(chunk1, p1_vec)) as u32;
                if let Some(av) = alt1_vec { mask1 |= _mm_movemask_epi8(_mm_cmpeq_epi8(chunk1, av)) as u32; }
                if mask1 == 0 { continue; }

                let chunk2 = _mm_loadu_si128(data.as_ptr().add(i + 1) as *const __m128i);
                let mut mask2 = _mm_movemask_epi8(_mm_cmpeq_epi8(chunk2, p2_vec)) as u32;
                if let Some(av) = alt2_vec { mask2 |= _mm_movemask_epi8(_mm_cmpeq_epi8(chunk2, av)) as u32; }
                
                let mut final_mask = mask1 & mask2;
                while final_mask != 0 {
                    let bit = final_mask.trailing_zeros();
                    let pos = i + bit as usize;
                    if pos + pattern.len() <= data.len() {
                        let chunk = &data[pos..pos + pattern.len()];
                        if if case_insensitive { chunk.eq_ignore_ascii_case(pattern) } else { chunk == pattern } {
                            results.push(pos as u64);
                        }
                    }
                    final_mask &= !(1u32 << bit);
                }
            }
            for i in simd_limit..=data.len().saturating_sub(pattern.len()) {
                let chunk = &data[i..i + pattern.len()];
                if if case_insensitive { chunk.eq_ignore_ascii_case(pattern) } else { chunk == pattern } {
                    results.push(i as u64);
                }
            }
        }
        results
    }
}

#[cfg(target_arch = "x86_64")]
impl ZenScanKernel for AvxKernel {
    fn scan(&self, data: &[u8], pattern: &[u8], case_insensitive: bool) -> Vec<u64> {
        unsafe { self.scan_impl(data, pattern, case_insensitive) }
    }
}

/// AVX2 (Modern)
#[cfg(target_arch = "x86_64")]
struct Avx2Kernel;

#[cfg(target_arch = "x86_64")]
impl Avx2Kernel {
    #[target_feature(enable = "avx2")]
    unsafe fn scan_impl(&self, data: &[u8], pattern: &[u8], case_insensitive: bool) -> Vec<u64> {
        use std::arch::x86_64::*;
        let mut results = Vec::new();
        if pattern.is_empty() || data.len() < pattern.len() { return results; }
        
        if pattern.len() == 1 {
            let p_byte = pattern[0];
            let p_vec = _mm256_set1_epi8(p_byte as i8);
            let alt_vec = if case_insensitive && p_byte.is_ascii_alphabetic() { Some(_mm256_set1_epi8((p_byte ^ 0x20) as i8)) } else { None };
            
            let chunk_size = 32;
            let limit = data.len().saturating_sub(data.len() % chunk_size);
            for i in (0..limit).step_by(chunk_size) {
                let chunk = _mm256_loadu_si256(data.as_ptr().add(i) as *const __m256i);
                let mut mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, p_vec)) as u32;
                if let Some(av) = alt_vec { mask |= _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, av)) as u32; }
                while mask != 0 {
                    let bit = mask.trailing_zeros();
                    results.push((i + bit as usize) as u64);
                    mask &= !(1u32 << bit);
                }
            }
            for i in limit..data.len() {
                if if case_insensitive { data[i].eq_ignore_ascii_case(&p_byte) } else { data[i] == p_byte } {
                    results.push(i as u64);
                }
            }
        } else {
            let p_first = pattern[0];
            let p_second = pattern[1];
            let p1_vec = _mm256_set1_epi8(p_first as i8);
            let alt1_vec = if case_insensitive && p_first.is_ascii_alphabetic() { Some(_mm256_set1_epi8((p_first ^ 0x20) as i8)) } else { None };
            let p2_vec = _mm256_set1_epi8(p_second as i8);
            let alt2_vec = if case_insensitive && p_second.is_ascii_alphabetic() { Some(_mm256_set1_epi8((p_second ^ 0x20) as i8)) } else { None };
            
            let chunk_size = 32;
            let limit = data.len().saturating_sub(pattern.len());
            let simd_limit = if limit >= chunk_size { limit - (limit % chunk_size) } else { 0 };

            for i in (0..simd_limit).step_by(chunk_size) {
                let chunk1 = _mm256_loadu_si256(data.as_ptr().add(i) as *const __m256i);
                let mut mask1 = _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk1, p1_vec)) as u32;
                if let Some(av) = alt1_vec { mask1 |= _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk1, av)) as u32; }
                if mask1 == 0 { continue; }

                let chunk2 = _mm256_loadu_si256(data.as_ptr().add(i + 1) as *const __m256i);
                let mut mask2 = _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk2, p2_vec)) as u32;
                if let Some(av) = alt2_vec { mask2 |= _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk2, av)) as u32; }
                
                let mut final_mask = mask1 & mask2;
                while final_mask != 0 {
                    let bit = final_mask.trailing_zeros();
                    let pos = i + bit as usize;
                    if pos + pattern.len() <= data.len() {
                        let chunk = &data[pos..pos + pattern.len()];
                        if if case_insensitive { chunk.eq_ignore_ascii_case(pattern) } else { chunk == pattern } {
                            results.push(pos as u64);
                        }
                    }
                    final_mask &= !(1u32 << bit);
                }
            }
            for i in simd_limit..=data.len().saturating_sub(pattern.len()) {
                let chunk = &data[i..i + pattern.len()];
                if if case_insensitive { chunk.eq_ignore_ascii_case(pattern) } else { chunk == pattern } {
                    results.push(i as u64);
                }
            }
        }
        results
    }
}

#[cfg(target_arch = "x86_64")]
impl ZenScanKernel for Avx2Kernel {
    fn scan(&self, data: &[u8], pattern: &[u8], case_insensitive: bool) -> Vec<u64> {
        unsafe { self.scan_impl(data, pattern, case_insensitive) }
    }
}

/// AVX-512 (Nitro)
#[cfg(target_arch = "x86_64")]
struct Avx512Kernel;

#[cfg(target_arch = "x86_64")]
impl Avx512Kernel {
    #[target_feature(enable = "avx512f")]
    unsafe fn scan_impl(&self, data: &[u8], pattern: &[u8], case_insensitive: bool) -> Vec<u64> {
        use std::arch::x86_64::*;
        let mut results = Vec::new();
        if pattern.is_empty() || data.len() < pattern.len() { return results; }
        
        if pattern.len() == 1 {
            let p_byte = pattern[0];
            let p_vec = _mm512_set1_epi8(p_byte as i8);
            let alt_vec = if case_insensitive && p_byte.is_ascii_alphabetic() { Some(_mm512_set1_epi8((p_byte ^ 0x20) as i8)) } else { None };
            
            let chunk_size = 64;
            let limit = data.len().saturating_sub(data.len() % chunk_size);
            for i in (0..limit).step_by(chunk_size) {
                let chunk = _mm512_loadu_si512(data.as_ptr().add(i) as *const __m512i);
                let mut mask: u64 = _mm512_cmpeq_epi8_mask(chunk, p_vec);
                if let Some(av) = alt_vec { mask |= _mm512_cmpeq_epi8_mask(chunk, av); }
                while mask != 0 {
                    let bit = mask.trailing_zeros();
                    results.push((i + bit as usize) as u64);
                    mask &= !(1u64 << bit);
                }
            }
            for i in limit..data.len() {
                if if case_insensitive { data[i].eq_ignore_ascii_case(&p_byte) } else { data[i] == p_byte } {
                    results.push(i as u64);
                }
            }
        } else {
            let p_first = pattern[0];
            let p_second = pattern[1];
            let p1_vec = _mm512_set1_epi8(p_first as i8);
            let alt1_vec = if case_insensitive && p_first.is_ascii_alphabetic() { Some(_mm512_set1_epi8((p_first ^ 0x20) as i8)) } else { None };
            let p2_vec = _mm512_set1_epi8(p_second as i8);
            let alt2_vec = if case_insensitive && p_second.is_ascii_alphabetic() { Some(_mm512_set1_epi8((p_second ^ 0x20) as i8)) } else { None };
            
            let chunk_size = 64;
            let limit = data.len().saturating_sub(pattern.len());
            let simd_limit = if limit >= chunk_size { limit - (limit % chunk_size) } else { 0 };

            for i in (0..simd_limit).step_by(chunk_size) {
                let chunk1 = _mm512_loadu_si512(data.as_ptr().add(i) as *const __m512i);
                let mut mask1: u64 = _mm512_cmpeq_epi8_mask(chunk1, p1_vec);
                if let Some(av) = alt1_vec { mask1 |= _mm512_cmpeq_epi8_mask(chunk1, av); }
                if mask1 == 0 { continue; }

                let chunk2 = _mm512_loadu_si512(data.as_ptr().add(i + 1) as *const __m512i);
                let mut mask2: u64 = _mm512_cmpeq_epi8_mask(chunk2, p2_vec);
                if let Some(av) = alt2_vec { mask2 |= _mm512_cmpeq_epi8_mask(chunk2, av); }
                
                let mut final_mask = mask1 & mask2;
                while final_mask != 0 {
                    let bit = final_mask.trailing_zeros();
                    let pos = i + bit as usize;
                    if pos + pattern.len() <= data.len() {
                        let chunk = &data[pos..pos + pattern.len()];
                        if if case_insensitive { chunk.eq_ignore_ascii_case(pattern) } else { chunk == pattern } {
                            results.push(pos as u64);
                        }
                    }
                    final_mask &= !(1u64 << bit);
                }
            }
            for i in simd_limit..=data.len().saturating_sub(pattern.len()) {
                let chunk = &data[i..i + pattern.len()];
                if if case_insensitive { chunk.eq_ignore_ascii_case(pattern) } else { chunk == pattern } {
                    results.push(i as u64);
                }
            }
        }
        results
    }
}

#[cfg(target_arch = "x86_64")]
impl ZenScanKernel for Avx512Kernel {
    fn scan(&self, data: &[u8], pattern: &[u8], case_insensitive: bool) -> Vec<u64> {
        unsafe { self.scan_impl(data, pattern, case_insensitive) }
    }
}
