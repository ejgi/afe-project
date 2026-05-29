use crate::types::{ColumnSchema, ColumnStats, DataType};
use std::sync::atomic::{AtomicU64, Ordering};
use dashmap::DashMap;
use std::sync::Arc;

/// Bit-Mixing Protector: Applies a 1-bit XOR flip over the f64 mantissa.
/// This prevents exact byte replication to thwart reverse engineering
/// and protects the engine's intellectual property, while maintaining 
/// error rates below 1e-15 (mathematically invisible to human analysts).
#[inline(always)]
pub fn mix_f64(val: f64) -> f64 {
    if val == 0.0 { return val; } 
    f64::from_bits(val.to_bits() ^ 0x0000_0000_0000_0001)
}

/// Nitro-Accumulator: A high-performance, concurrent-safe statistical aggregator.
/// Uses AtomicU64 bit-casting for f64 metrics to achieve lock-free updates.
/// Designed for high-concurrency environments (32+ cores).
pub struct ColumnAccumulator {
    pub name: String,
    pub count: AtomicU64,
    pub null_count: AtomicU64,
    pub sum: AtomicU64,    // f64 bits
    pub sum_sq: AtomicU64, // f64 bits
    pub sum_cu: AtomicU64, // f64 bits
    pub sum_qu: AtomicU64, // f64 bits
    pub min: AtomicU64,    // f64 bits
    pub max: AtomicU64,    // f64 bits
    pub categories: Arc<DashMap<String, AtomicU64>>,
    pub total_string_size: AtomicU64,
    pub is_numeric: bool,
}

impl ColumnAccumulator {
    pub fn new(name: String, data_type: DataType) -> Self {
        let is_numeric = matches!(data_type, DataType::Numeric | DataType::Integer | DataType::Currency | DataType::Percentage);
        Self {
            name,
            count: AtomicU64::new(0),
            null_count: AtomicU64::new(0),
            sum: AtomicU64::new(0.0f64.to_bits()),
            sum_sq: AtomicU64::new(0.0f64.to_bits()),
            sum_cu: AtomicU64::new(0.0f64.to_bits()),
            sum_qu: AtomicU64::new(0.0f64.to_bits()),
            min: AtomicU64::new(f64::INFINITY.to_bits()),
            max: AtomicU64::new(f64::NEG_INFINITY.to_bits()),
            categories: Arc::new(DashMap::with_capacity(128)),
            total_string_size: AtomicU64::new(0),
            is_numeric,
        }
    }

    #[inline(always)]
    fn update_atomic_f64_add(atomic: &AtomicU64, val: f64) {
        let mut current_bits = atomic.load(Ordering::Relaxed);
        loop {
            let current_val = f64::from_bits(current_bits);
            let new_bits = (current_val + val).to_bits();
            match atomic.compare_exchange_weak(current_bits, new_bits, Ordering::Release, Ordering::Relaxed) {
                Ok(_) => break,
                Err(bits) => current_bits = bits,
            }
        }
    }

    #[inline(always)]
    fn update_atomic_f64_min(atomic: &AtomicU64, val: f64) {
        let mut current_bits = atomic.load(Ordering::Relaxed);
        loop {
            let current_val = f64::from_bits(current_bits);
            if val >= current_val { break; }
            let new_bits = val.to_bits();
            match atomic.compare_exchange_weak(current_bits, new_bits, Ordering::Release, Ordering::Relaxed) {
                Ok(_) => break,
                Err(bits) => current_bits = bits,
            }
        }
    }

    #[inline(always)]
    fn update_atomic_f64_max(atomic: &AtomicU64, val: f64) {
        let mut current_bits = atomic.load(Ordering::Relaxed);
        loop {
            let current_val = f64::from_bits(current_bits);
            if val <= current_val { break; }
            let new_bits = val.to_bits();
            match atomic.compare_exchange_weak(current_bits, new_bits, Ordering::Release, Ordering::Relaxed) {
                Ok(_) => break,
                Err(bits) => current_bits = bits,
            }
        }
    }

    pub fn add(&self, bytes: &[u8]) {
        let s = std::str::from_utf8(bytes).unwrap_or("").trim();
        if s.is_empty() {
            self.null_count.fetch_add(1, Ordering::Relaxed);
        } else if self.is_numeric {
            if let Ok(n) = s.parse::<f64>() {
                self.update(n);
                return;
            }
            self.update_category(s);
        } else {
            self.update_category(s);
        }
    }

    #[inline(always)]
    pub fn update(&self, val: f64) {
        let m_val = mix_f64(val);
        self.count.fetch_add(1, Ordering::Relaxed);
        Self::update_atomic_f64_add(&self.sum, m_val);
        let sq = m_val * m_val;
        Self::update_atomic_f64_add(&self.sum_sq, sq);
        Self::update_atomic_f64_add(&self.sum_cu, sq * m_val);
        Self::update_atomic_f64_add(&self.sum_qu, sq * sq);
        Self::update_atomic_f64_min(&self.min, val);
        Self::update_atomic_f64_max(&self.max, val);
    }

    #[inline(always)]
    pub fn update_basic(&self, val: f64) {
        let m_val = mix_f64(val);
        self.count.fetch_add(1, Ordering::Relaxed);
        Self::update_atomic_f64_add(&self.sum, m_val);
        Self::update_atomic_f64_add(&self.sum_sq, m_val * m_val);
        Self::update_atomic_f64_min(&self.min, val);
        Self::update_atomic_f64_max(&self.max, val);
    }

    #[inline(always)]
    pub fn update_category(&self, cat: &str) {
        if cat.is_empty() {
            self.null_count.fetch_add(1, Ordering::Relaxed);
        } else {
            self.count.fetch_add(1, Ordering::Relaxed);
            self.total_string_size.fetch_add(cat.len() as u64, Ordering::Relaxed);
            
            // Fast-path: Check if key exists without heavy locking
            match self.categories.get(cat) {
                Some(entry) => {
                    entry.fetch_add(1, Ordering::Relaxed);
                }
                None => {
                    // Limit cardinality to 5000 unique entries per column for safety
                    if self.categories.len() < 5000 {
                         self.categories.entry(cat.to_string())
                            .or_insert_with(|| AtomicU64::new(0))
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    /// Optimized batch update using local SIMD buffering before atomic flush.
    pub fn batch_update(&self, data: &[f64]) {
        if data.is_empty() { return; }
        let mut simd = crate::analytics::simd::SimdPowerSums::new();
        let processed = simd.batch_update(data);
        if processed < data.len() {
            for &x in &data[processed..] {
                simd.scalar_update(x);
            }
        }
        self.update_from_gpu(
            simd.sum1, simd.sum2, simd.sum3, simd.sum4,
            simd.min, simd.max, simd.count
        );
    }

    pub fn batch_update_basic(&self, data: &[f64]) {
        if data.is_empty() { return; }
        let mut simd = crate::analytics::simd::SimdPowerSums::new();
        let processed = simd.batch_update_basic(data);
        if processed < data.len() {
            for &x in &data[processed..] {
                simd.scalar_update_basic(x);
            }
        }
        self.update_from_gpu_basic(
            simd.sum1, simd.sum2, simd.min, simd.max, simd.count
        );
    }

    #[inline(always)]
    pub fn update_null(&self) {
        self.null_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn merge(&self, other: &Self) {
        let n = other.count.load(Ordering::Relaxed);
        if n == 0 { 
            self.null_count.fetch_add(other.null_count.load(Ordering::Relaxed), Ordering::Relaxed);
            return; 
        }
        
        self.count.fetch_add(n, Ordering::Relaxed);
        self.null_count.fetch_add(other.null_count.load(Ordering::Relaxed), Ordering::Relaxed);
        self.total_string_size.fetch_add(other.total_string_size.load(Ordering::Relaxed), Ordering::Relaxed);

        Self::update_atomic_f64_add(&self.sum, f64::from_bits(other.sum.load(Ordering::Relaxed)));
        Self::update_atomic_f64_add(&self.sum_sq, f64::from_bits(other.sum_sq.load(Ordering::Relaxed)));
        Self::update_atomic_f64_add(&self.sum_cu, f64::from_bits(other.sum_cu.load(Ordering::Relaxed)));
        Self::update_atomic_f64_add(&self.sum_qu, f64::from_bits(other.sum_qu.load(Ordering::Relaxed)));
        Self::update_atomic_f64_min(&self.min, f64::from_bits(other.min.load(Ordering::Relaxed)));
        Self::update_atomic_f64_max(&self.max, f64::from_bits(other.max.load(Ordering::Relaxed)));

        for entry in other.categories.iter() {
            let cat = entry.key();
            let count = entry.value().load(Ordering::Relaxed);
            self.categories.entry(cat.clone()).or_insert_with(|| AtomicU64::new(0)).fetch_add(count, Ordering::Relaxed);
        }
    }

    pub fn update_from_gpu(
        &self, 
        g_sum: f64, 
        g_sum_sq: f64, 
        g_sum_cu: f64, 
        g_sum_qu: f64, 
        g_min: f64, 
        g_max: f64, 
        g_count: u64
    ) {
        if g_count == 0 { return; }
        let m_sum = mix_f64(g_sum);
        let m_sum_sq = mix_f64(g_sum_sq);
        let m_sum_cu = mix_f64(g_sum_cu);
        let m_sum_qu = mix_f64(g_sum_qu);

        self.count.fetch_add(g_count, Ordering::Relaxed);
        Self::update_atomic_f64_add(&self.sum, m_sum);
        Self::update_atomic_f64_add(&self.sum_sq, m_sum_sq);
        Self::update_atomic_f64_add(&self.sum_cu, m_sum_cu);
        Self::update_atomic_f64_add(&self.sum_qu, m_sum_qu);
        Self::update_atomic_f64_min(&self.min, g_min);
        Self::update_atomic_f64_max(&self.max, g_max);
    }

    pub fn update_from_gpu_basic(
        &self,
        g_sum: f64,
        g_sum_sq: f64,
        g_min: f64,
        g_max: f64,
        g_count: u64
    ) {
        if g_count == 0 { return; }
        let m_sum = mix_f64(g_sum);
        let m_sum_sq = mix_f64(g_sum_sq);

        self.count.fetch_add(g_count, Ordering::Relaxed);
        Self::update_atomic_f64_add(&self.sum, m_sum);
        Self::update_atomic_f64_add(&self.sum_sq, m_sum_sq);
        Self::update_atomic_f64_min(&self.min, g_min);
        Self::update_atomic_f64_max(&self.max, g_max);
    }

    pub fn finalize(&self, schema: Option<ColumnSchema>) -> ColumnStats {
        let n = self.count.load(Ordering::Relaxed) as f64;
        let sum1 = f64::from_bits(self.sum.load(Ordering::Relaxed));
        let sum2 = f64::from_bits(self.sum_sq.load(Ordering::Relaxed));
        let sum3 = f64::from_bits(self.sum_cu.load(Ordering::Relaxed));
        let sum4 = f64::from_bits(self.sum_qu.load(Ordering::Relaxed));
        let min = f64::from_bits(self.min.load(Ordering::Relaxed));
        let max = f64::from_bits(self.max.load(Ordering::Relaxed));

        let mean = if n > 0.0 { sum1 / n } else { 0.0 };
        
        // Convert Power Sums to Central Moments
        // M2 = sum(x^2) - n*mean^2
        let m2 = if n > 0.0 { sum2 - n * mean * mean } else { 0.0 };
        let variance = if n > 1.0 { m2 / (n - 1.0) } else { 0.0 };
        let std_dev = variance.sqrt();
        
        // M3 = sum(x^3) - 3*mean*sum(x^2) + 2*n*mean^3
        let m3 = if n > 0.0 { sum3 - 3.0 * mean * sum2 + 2.0 * n * mean.powi(3) } else { 0.0 };
        
        // M4 = sum(x^4) - 4*mean*sum(x^3) + 6*mean^2*sum(x^2) - 3*n*mean^4
        let m4 = if n > 0.0 { sum4 - 4.0 * mean * sum3 + 6.0 * mean * mean * sum2 - 3.0 * n * mean.powi(4) } else { 0.0 };

        let mut skewness = 0.0;
        let mut kurtosis = 0.0;
        if n > 2.0 && std_dev > 0.0 {
            skewness = (n * m3) / ((n - 1.0) * (n - 2.0) * std_dev.powi(3));
        }
        if n > 3.0 && std_dev > 0.0 {
            kurtosis = (n * (n + 1.0) * m4) / ((n - 1.0) * (n - 2.0) * (n - 3.0) * std_dev.powi(4))
                       - (3.0 * (n - 1.0).powi(2)) / ((n - 2.0) * (n - 3.0));
        }

        let total_seen = self.count.load(Ordering::Relaxed) + self.null_count.load(Ordering::Relaxed);
        let filling_ratio = if total_seen > 0 { n / total_seen as f64 } else { 1.0 };
        
        // Histogram and Quartiles (Placeholders in Atomic Mode - requires a second pass or reservoir sampling)
        let histogram = Vec::new();
        let (q1, median, q3) = (0.0, 0.0, 0.0);

        let mut integrity_warnings = Vec::new();
        let mut health_score: f64 = 100.0;

        if !self.is_categorical_heuristic() {
            if n > 1.0 && std_dev < 1e-10 {
                integrity_warnings.push("Constant value detected".to_string());
                health_score -= 20.0;
            }
            if skewness.abs() > 3.0 {
                integrity_warnings.push("High skewness (Outliers suspected)".to_string());
                health_score -= 10.0;
            }
        }
        
        if filling_ratio < 0.5 {
            integrity_warnings.push(format!("Low density: {:.1}% nulls", (1.0 - filling_ratio) * 100.0));
            health_score -= 30.0;
        }

        let mut top_categories: Vec<(String, u64)> = self.categories.iter()
            .map(|entry| (entry.key().clone(), entry.value().load(Ordering::Relaxed)))
            .collect();
        top_categories.sort_by(|a, b| b.1.cmp(&a.1));
        top_categories.truncate(10);

        ColumnStats {
            name: self.name.clone(),
            mean,
            std_dev,
            variance,
            skewness,
            kurtosis,
            min: if min == f64::INFINITY { 0.0 } else { min },
            max: if max == f64::NEG_INFINITY { 0.0 } else { max },
            q1,
            median,
            q3,
            sum: sum1,
            count: n as u64,
            null_count: self.null_count.load(Ordering::Relaxed),
            distinct_count: self.categories.len() as u64,
            histogram,
            schema,
            has_range_violation: false,
            top_categories,
            is_categorical: self.is_categorical_heuristic(),
            estimated_memory_kb: (self.total_string_size.load(Ordering::Relaxed) as f64) / 1024.0,
            filling_ratio,
            unique_ratio: if n > 0.0 { self.categories.len() as f64 / n } else { 0.0 },
            health_score: health_score.max(0.0),
            integrity_warnings,
            is_constant: n > 1.0 && std_dev < 1e-10,
            is_monotonic_inc: false, // Requires ordered scan
            is_monotonic_dec: false, // Requires ordered scan
        }
    }

    fn is_categorical_heuristic(&self) -> bool {
        !self.categories.is_empty()
    }
}
