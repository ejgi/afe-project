use wgpu::*;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use crate::compute::GpuContext;

pub(crate) async fn new_internal() -> Result<GpuContext> {
    let instance = Instance::new(&InstanceDescriptor {
        backends: Backends::all(),
        ..Default::default()
    });

    let adapter = instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::LowPower, // Better for integrated GPUs
        compatible_surface: None,
        force_fallback_adapter: false,
    }).await.ok_or_else(|| anyhow!("No compatible GPU adapter found. Backends tried: {:?}", Backends::all()))?;

    let (device, queue) = adapter.request_device(
        &DeviceDescriptor {
            label: Some("ZenEngine GpuContext"),
            required_features: Features::empty(),
            required_limits: Limits::downlevel_webgl2_defaults(), // More compatible limits
            memory_hints: MemoryHints::Performance,
        },
        None,
    ).await.map_err(|e| anyhow!("Failed to request GPU device: {}", e))?;

    Ok(GpuContext {
        device: Arc::new(device),
        queue: Arc::new(queue),
        adapter,
    })
}
