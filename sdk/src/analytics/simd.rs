#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// SIMD-accelerated Power Sums (x, x^2, x^3, x^4) for statistical moments.
/// This kernel processes 4 f64 values at a time using AVX2.
pub struct SimdPowerSums {
    pub sum1: f64,
    pub sum2: f64,
    pub sum3: f64,
    pub sum4: f64,
    pub min: f64,
    pub max: f64,
    pub count: u64,
}

impl SimdPowerSums {
    pub fn new() -> Self {
        Self {
            sum1: 0.0,
            sum2: 0.0,
            sum3: 0.0,
            sum4: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            count: 0,
        }
    }

    /// Aggregates a batch of f64 values using AVX2 if available.
    /// Returns the number of elements processed by the SIMD kernel.
    pub fn batch_update(&mut self, data: &[f64]) -> usize {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return unsafe { self.batch_update_avx2(data) };
            }
        }
        0 // Fallback to scalar or other architectures
    }

    /// Optimized path for Basic level: only sum and sum_sq.
    pub fn batch_update_basic(&mut self, data: &[f64]) -> usize {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return unsafe { self.batch_update_basic_avx2(data) };
            }
        }
        0
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn batch_update_avx2(&mut self, data: &[f64]) -> usize {
        let n = data.len();
        let chunk_size = 4;
        let limit = n - (n % chunk_size);
        
        let mut v_sum1 = _mm256_setzero_pd();
        let mut v_sum2 = _mm256_setzero_pd();
        let mut v_sum3 = _mm256_setzero_pd();
        let mut v_sum4 = _mm256_setzero_pd();
        let mut v_min = _mm256_set1_pd(self.min);
        let mut v_max = _mm256_set1_pd(self.max);

        // Prefetch stride: 16 f64 values ahead = 128 bytes (2 cache lines)
        // This keeps the CPU pipeline fed without stalls on L1 cache misses.
        const PREFETCH_STRIDE: usize = 16;

        for i in (0..limit).step_by(chunk_size) {
            // Opaque prefetch: non-temporal hint for the next cache line window
            if i + PREFETCH_STRIDE < n {
                _mm_prefetch(
                    data.as_ptr().add(i + PREFETCH_STRIDE) as *const i8,
                    _MM_HINT_T0,
                );
            }

            let x = _mm256_loadu_pd(data.as_ptr().add(i));
            
            // x^2
            let x2 = _mm256_mul_pd(x, x);
            // x^3
            let x3 = _mm256_mul_pd(x2, x);
            // x^4
            let x4 = _mm256_mul_pd(x2, x2);

            v_sum1 = _mm256_add_pd(v_sum1, x);
            v_sum2 = _mm256_add_pd(v_sum2, x2);
            v_sum3 = _mm256_add_pd(v_sum3, x3);
            v_sum4 = _mm256_add_pd(v_sum4, x4);
            
            v_min = _mm256_min_pd(v_min, x);
            v_max = _mm256_max_pd(v_max, x);
        }

        // Horizontal reduction
        let mut res1 = [0.0f64; 4];
        let mut res2 = [0.0f64; 4];
        let mut res3 = [0.0f64; 4];
        let mut res4 = [0.0f64; 4];
        let mut res_min = [0.0f64; 4];
        let mut res_max = [0.0f64; 4];

        _mm256_storeu_pd(res1.as_mut_ptr(), v_sum1);
        _mm256_storeu_pd(res2.as_mut_ptr(), v_sum2);
        _mm256_storeu_pd(res3.as_mut_ptr(), v_sum3);
        _mm256_storeu_pd(res4.as_mut_ptr(), v_sum4);
        _mm256_storeu_pd(res_min.as_mut_ptr(), v_min);
        _mm256_storeu_pd(res_max.as_mut_ptr(), v_max);

        for i in 0..4 {
            self.sum1 += res1[i];
            self.sum2 += res2[i];
            self.sum3 += res3[i];
            self.sum4 += res4[i];
            if res_min[i] < self.min { self.min = res_min[i]; }
            if res_max[i] > self.max { self.max = res_max[i]; }
        }

        self.count += limit as u64;
        limit
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn batch_update_basic_avx2(&mut self, data: &[f64]) -> usize {
        let n = data.len();
        let chunk_size = 4;
        let limit = n - (n % chunk_size);
        
        let mut v_sum1 = _mm256_setzero_pd();
        let mut v_sum2 = _mm256_setzero_pd();
        let mut v_min = _mm256_set1_pd(self.min);
        let mut v_max = _mm256_set1_pd(self.max);

        // Prefetch stride: 16 f64 values ahead = 128 bytes (2 cache lines)
        const PREFETCH_STRIDE: usize = 16;

        for i in (0..limit).step_by(chunk_size) {
            // Opaque prefetch: non-temporal hint for the next cache line window
            if i + PREFETCH_STRIDE < n {
                _mm_prefetch(
                    data.as_ptr().add(i + PREFETCH_STRIDE) as *const i8,
                    _MM_HINT_T0,
                );
            }

            let x = _mm256_loadu_pd(data.as_ptr().add(i));
            let x2 = _mm256_mul_pd(x, x);

            v_sum1 = _mm256_add_pd(v_sum1, x);
            v_sum2 = _mm256_add_pd(v_sum2, x2);
            v_min = _mm256_min_pd(v_min, x);
            v_max = _mm256_max_pd(v_max, x);
        }

        let mut res1 = [0.0f64; 4];
        let mut res2 = [0.0f64; 4];
        let mut res_min = [0.0f64; 4];
        let mut res_max = [0.0f64; 4];

        _mm256_storeu_pd(res1.as_mut_ptr(), v_sum1);
        _mm256_storeu_pd(res2.as_mut_ptr(), v_sum2);
        _mm256_storeu_pd(res_min.as_mut_ptr(), v_min);
        _mm256_storeu_pd(res_max.as_mut_ptr(), v_max);

        for i in 0..4 {
            self.sum1 += res1[i];
            self.sum2 += res2[i];
            if res_min[i] < self.min { self.min = res_min[i]; }
            if res_max[i] > self.max { self.max = res_max[i]; }
        }

        self.count += limit as u64;
        limit
    }
    
    /// Scalar fallback for remaining elements or non-AVX systems.
    pub fn scalar_update(&mut self, val: f64) {
        let x2 = val * val;
        let x3 = x2 * val;
        let x4 = x2 * x2;
        
        self.sum1 += val;
        self.sum2 += x2;
        self.sum3 += x3;
        self.sum4 += x4;
        
        if val < self.min { self.min = val; }
        if val > self.max { self.max = val; }
        self.count += 1;
    }

    pub fn scalar_update_basic(&mut self, val: f64) {
        let x2 = val * val;
        self.sum1 += val;
        self.sum2 += x2;
        if val < self.min { self.min = val; }
        if val > self.max { self.max = val; }
        self.count += 1;
    }
}
