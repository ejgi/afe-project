use wgpu::*;
use anyhow::Result;
use std::sync::Arc;
use crate::compute::{GpuContext, GpuProcessor};

pub(crate) fn new_processor_impl(ctx: Arc<GpuContext>) -> Result<GpuProcessor> {
    let raw_shader = ctx.device.create_shader_module(ShaderModuleDescriptor {
        label: Some("Zen Raw Filter Shader"),
        source: ShaderSource::Wgsl(include_str!("../filter.wgsl").into()),
    });

    let row_shader = ctx.device.create_shader_module(ShaderModuleDescriptor {
        label: Some("Zen Row Filter Shader"),
        source: ShaderSource::Wgsl(include_str!("../filter_rows.wgsl").into()),
    });

    let agg_shader = ctx.device.create_shader_module(ShaderModuleDescriptor {
        label: Some("Zen Aggregation Shader"),
        source: ShaderSource::Wgsl(include_str!("../aggregate.wgsl").into()),
    });

    let vec_shader = ctx.device.create_shader_module(ShaderModuleDescriptor {
        label: Some("Zen Vectorize Shader"),
        source: ShaderSource::Wgsl(include_str!("../vectorize.wgsl").into()),
    });

    let raw_pipeline = ctx.device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("Zen Raw Filter Pipeline"),
        layout: None,
        module: &raw_shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let row_pipeline = ctx.device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("Zen Row Filter Pipeline"),
        layout: None,
        module: &row_shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let agg_pipeline = ctx.device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("Zen Aggregation Pipeline"),
        layout: None,
        module: &agg_shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let vec_pipeline = ctx.device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("Zen Vectorize Pipeline"),
        layout: None,
        module: &vec_shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    Ok(GpuProcessor { ctx, raw_pipeline, row_pipeline, agg_pipeline, vec_pipeline })
}
