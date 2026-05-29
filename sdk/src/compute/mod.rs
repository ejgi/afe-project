pub mod dma;
pub mod driver;
pub mod pipeline;
pub mod kernels;

use wgpu::*;
use std::sync::Arc;
use anyhow::Result;
use std::path::Path;

pub use crate::compute::dma::get_dma;

/// The GPU Context encapsulates the wgpu device, queue, and adapter.
/// This is the engine room for all GPU-accelerated operations.
pub struct GpuContext {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub adapter: Adapter,
}

impl GpuContext {
    /// Blocking initialization of the GPU context with detailed reporting.
    pub fn new() -> Result<Self> {
        pollster::block_on(driver::new_internal())
    }

    /// Returns the name of the GPU adapter being used.
    pub fn get_adapter_info(&self) -> AdapterInfo {
        self.adapter.get_info()
    }
}

/// Source for GPU data: either a memory slice or a direct file path for GDS.
pub enum GpuDataSource<'a> {
    Memory(&'a [u8]),
    File {
        path: &'a Path,
        offset: u64,
        size: u64,
    },
}

pub struct GpuProcessor {
    pub ctx: Arc<GpuContext>,
    pub raw_pipeline: ComputePipeline,
    pub row_pipeline: ComputePipeline,
    pub agg_pipeline: ComputePipeline,
    pub vec_pipeline: ComputePipeline,
}

impl GpuProcessor {
    pub fn new(ctx: Arc<GpuContext>) -> Result<Self> {
        pipeline::new_processor_impl(ctx)
    }

    pub fn run_filter(&self, source: GpuDataSource, query: &[u8]) -> Result<Vec<u32>> {
        kernels::run_filter_impl(self, source, query)
    }

    pub fn run_filter_rows(&self, source: GpuDataSource, offsets: &[u32], query: &[u8]) -> Result<Vec<u32>> {
        kernels::run_filter_rows_impl(self, source, offsets, query)
    }

    pub fn run_aggregate(
        &self, 
        source: GpuDataSource, 
        offsets: &[u32], 
        col_index: u32, 
        delimiter: u8,
        selection_mask: &[u32]
    ) -> Result<Vec<f32>> {
        kernels::run_aggregate_impl(self, source, offsets, col_index, delimiter, selection_mask)
    }

    pub fn run_vectorize(
        &self,
        source: GpuDataSource,
        offsets: &[u32],
        col_index: u32,
        delimiter: u8,
    ) -> Result<Vec<f32>> {
        kernels::run_vectorize_impl(self, source, offsets, col_index, delimiter)
    }
}
