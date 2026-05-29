use std::time::Instant;
use zen_engine::analytics::simd::SimdPowerSums;
use zen_engine::accumulator::ColumnAccumulator;

fn main() {
    println!("🚀 Zen SIMD Benchmark — Scalar vs AVX2");
    println!("--------------------------------------");

    let n = 100_000_000; // 100 million elements
    println!("Generating {} test elements...", n);
    let data: Vec<f64> = (0..n).map(|i| i as f64 * 0.01).collect();

    // 1. Scalar Benchmark (Welford's algorithm as used in ColumnAccumulator)
    println!("Running Scalar (Welford) Benchmark...");
    let acc = ColumnAccumulator::new("test".to_string(), zen_engine::types::DataType::Numeric);
    let t0 = Instant::now();
    for &x in &data {
        acc.update(x);
    }
    let d0 = t0.elapsed();
    let final_sum = f64::from_bits(acc.sum.load(std::sync::atomic::Ordering::Relaxed));
    let final_count = acc.count.load(std::sync::atomic::Ordering::Relaxed) as f64;
    let final_mean = if final_count > 0.0 { final_sum / final_count } else { 0.0 };
    println!("  Scalar Time: {:.2?}", d0);
    println!("  Mean: {:.4}, Sum: {:.4}", final_mean, final_sum);

    // 2. SIMD Benchmark (AVX2 Power Sums)
    println!("Running SIMD (AVX2) Benchmark...");
    let mut simd = SimdPowerSums::new();
    let t1 = Instant::now();
    
    // Process in large chunks to simulate real world usage
    let chunk_size = 1024 * 1024;
    for i in (0..n).step_by(chunk_size) {
        let end = (i + chunk_size).min(n);
        let chunk = &data[i..end];
        let processed = simd.batch_update(chunk);
        
        // Handle remainder if any
        if processed < chunk.len() {
            for &x in &chunk[processed..] {
                simd.scalar_update(x);
            }
        }
    }
    let d1 = t1.elapsed();
    
    // Finalize mean from power sums
    let simd_mean = simd.sum1 / simd.count as f64;
    
    println!("  SIMD Time:   {:.2?}", d1);
    println!("  Mean: {:.4}, Sum: {:.4}", simd_mean, simd.sum1);

    println!("--------------------------------------");
    let speedup = d0.as_secs_f64() / d1.as_secs_f64();
    println!("🏎️  Speedup: {:.2}x", speedup);
}
