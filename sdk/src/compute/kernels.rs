use wgpu::*;
use wgpu::util::DeviceExt;
use anyhow::{Result, anyhow};
use crate::compute::{GpuProcessor, GpuDataSource};

pub(crate) fn run_filter_impl(processor: &GpuProcessor, source: GpuDataSource, query: &[u8]) -> Result<Vec<u32>> {
    let device = &processor.ctx.device;
    let queue = &processor.ctx.queue;

    let (data_buffer, data_len) = match source {
        GpuDataSource::Memory(data) => {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Data Buffer (RAM)"),
                contents: data,
                usage: BufferUsages::STORAGE,
            });
            (buffer, data.len())
        }
        GpuDataSource::File { path, offset, size } => {
            let buffer = device.create_buffer(&BufferDescriptor {
                label: Some("Data Buffer (GDS)"),
                size,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let data = std::fs::read(path)?;
            queue.write_buffer(&buffer, 0, &data[offset as usize..(offset + size) as usize]);
            (buffer, size as usize)
        }
    };

    let query_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Query Buffer"),
        contents: query,
        usage: BufferUsages::STORAGE,
    });

    let result_size = (data_len + 31) / 32 * 4;
    let result_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Result Buffer"),
        size: result_size as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let params = [data_len as u32, query.len() as u32, 1000u32, 0u32];
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Params Buffer"),
        contents: bytemuck::cast_slice(&params),
        usage: BufferUsages::UNIFORM,
    });

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Filter Bind Group"),
        layout: &processor.raw_pipeline.get_bind_group_layout(0),
        entries: &[
            BindGroupEntry { binding: 0, resource: data_buffer.as_entire_binding() },
            BindGroupEntry { binding: 1, resource: query_buffer.as_entire_binding() },
            BindGroupEntry { binding: 2, resource: result_buffer.as_entire_binding() },
            BindGroupEntry { binding: 3, resource: params_buffer.as_entire_binding() },
        ],
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
    {
        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: None, timestamp_writes: None });
        compute_pass.set_pipeline(&processor.raw_pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups((data_len as u32 + 255) / 256, 1, 1);
    }

    let output_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Output Mapping Buffer"),
        size: result_size as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    encoder.copy_buffer_to_buffer(&result_buffer, 0, &output_buffer, 0, result_size as u64);

    queue.submit(Some(encoder.finish()));

    let buffer_slice = output_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(MapMode::Read, move |v| tx.send(v).unwrap());
    device.poll(Maintain::Wait);

    if rx.recv().unwrap().is_ok() {
        let data = buffer_slice.get_mapped_range();
        let bitmask: &[u32] = bytemuck::cast_slice(&data);
        let mut indices = Vec::new();
        for (i, &mask) in bitmask.iter().enumerate() {
            if mask != 0 {
                for bit in 0..32 {
                    if (mask & (1 << bit)) != 0 {
                        indices.push(i as u32 * 32 + bit);
                    }
                }
            }
        }
        drop(data);
        output_buffer.unmap();
        Ok(indices)
    } else {
        Err(anyhow!("GPU Map error"))
    }
}

pub(crate) fn run_filter_rows_impl(processor: &GpuProcessor, source: GpuDataSource, offsets: &[u32], query: &[u8]) -> Result<Vec<u32>> {
    let device = &processor.ctx.device;
    let queue = &processor.ctx.queue;

    let (data_buffer, data_len) = match source {
        GpuDataSource::Memory(data) => {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Data Buffer (RAM)"),
                contents: data,
                usage: BufferUsages::STORAGE,
            });
            (buffer, data.len())
        }
        GpuDataSource::File { path, offset, size } => {
            let buffer = device.create_buffer(&BufferDescriptor {
                label: Some("Data Buffer (GDS)"),
                size,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let data = std::fs::read(path)?;
            queue.write_buffer(&buffer, 0, &data[offset as usize..(offset + size) as usize]);
            (buffer, size as usize)
        }
    };

    let offsets_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Offsets Buffer"),
        contents: bytemuck::cast_slice(offsets),
        usage: BufferUsages::STORAGE,
    });

    let query_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Query Buffer"),
        contents: query,
        usage: BufferUsages::STORAGE,
    });

    let num_rows = offsets.len() as u32;
    let result_size = (num_rows + 31) / 32 * 4;
    let result_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Result Buffer"),
        size: result_size as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let params = [data_len as u32, query.len() as u32, num_rows, 0u32];
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Params Buffer"),
        contents: bytemuck::cast_slice(&params),
        usage: BufferUsages::UNIFORM,
    });

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Filter Bind Group"),
        layout: &processor.row_pipeline.get_bind_group_layout(0),
        entries: &[
            BindGroupEntry { binding: 0, resource: data_buffer.as_entire_binding() },
            BindGroupEntry { binding: 1, resource: query_buffer.as_entire_binding() },
            BindGroupEntry { binding: 2, resource: result_buffer.as_entire_binding() },
            BindGroupEntry { binding: 3, resource: params_buffer.as_entire_binding() },
            BindGroupEntry { binding: 4, resource: offsets_buffer.as_entire_binding() },
        ],
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
    {
        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: None, timestamp_writes: None });
        compute_pass.set_pipeline(&processor.row_pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups((num_rows + 255) / 256, 1, 1);
    }

    let output_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Output Mapping Buffer"),
        size: result_size as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    encoder.copy_buffer_to_buffer(&result_buffer, 0, &output_buffer, 0, result_size as u64);

    queue.submit(Some(encoder.finish()));

    let buffer_slice = output_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(MapMode::Read, move |v| tx.send(v).unwrap());
    device.poll(Maintain::Wait);

    if rx.recv().unwrap().is_ok() {
        let data = buffer_slice.get_mapped_range();
        let bitmask: Vec<u32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        output_buffer.unmap();
        Ok(bitmask)
    } else {
        Err(anyhow!("GPU Map error"))
    }
}

pub(crate) fn run_aggregate_impl(
    processor: &GpuProcessor,
    source: GpuDataSource,
    offsets: &[u32],
    col_index: u32,
    delimiter: u8,
    selection_mask: &[u32]
) -> Result<Vec<f32>> {
    let device = &processor.ctx.device;
    let queue = &processor.ctx.queue;

    let (data_buffer, data_len) = match source {
        GpuDataSource::Memory(data) => {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Data Buffer (RAM)"),
                contents: data,
                usage: BufferUsages::STORAGE,
            });
            (buffer, data.len())
        }
        GpuDataSource::File { path, offset, size } => {
            let buffer = device.create_buffer(&BufferDescriptor {
                label: Some("Data Buffer (GDS)"),
                size,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let data = std::fs::read(path)?;
            queue.write_buffer(&buffer, 0, &data[offset as usize..(offset + size) as usize]);
            (buffer, size as usize)
        }
    };

    let offsets_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Offsets Buffer"),
        contents: bytemuck::cast_slice(offsets),
        usage: BufferUsages::STORAGE,
    });

    let mask_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Selection Mask Buffer"),
        contents: bytemuck::cast_slice(selection_mask),
        usage: BufferUsages::STORAGE,
    });

    let num_rows = offsets.len() as u32;
    let num_workgroups = (num_rows + 255) / 256;
    let result_size = num_workgroups * 7 * 4;
    let result_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Agg Result Buffer"),
        size: result_size as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let params = [data_len as u32, num_rows, col_index, delimiter as u32];
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Params Buffer"),
        contents: bytemuck::cast_slice(&params),
        usage: BufferUsages::UNIFORM,
    });

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Agg Bind Group"),
        layout: &processor.agg_pipeline.get_bind_group_layout(0),
        entries: &[
            BindGroupEntry { binding: 0, resource: data_buffer.as_entire_binding() },
            BindGroupEntry { binding: 1, resource: offsets_buffer.as_entire_binding() },
            BindGroupEntry { binding: 2, resource: result_buffer.as_entire_binding() },
            BindGroupEntry { binding: 3, resource: params_buffer.as_entire_binding() },
            BindGroupEntry { binding: 4, resource: mask_buffer.as_entire_binding() },
        ],
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
    {
        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: None, timestamp_writes: None });
        compute_pass.set_pipeline(&processor.agg_pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
    }

    let output_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Agg Output Mapping Buffer"),
        size: result_size as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    encoder.copy_buffer_to_buffer(&result_buffer, 0, &output_buffer, 0, result_size as u64);

    queue.submit(Some(encoder.finish()));

    let buffer_slice = output_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(MapMode::Read, move |v| tx.send(v).unwrap());
    device.poll(Maintain::Wait);

    if rx.recv().unwrap().is_ok() {
        let data = buffer_slice.get_mapped_range();
        let partials: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        output_buffer.unmap();
        Ok(partials)
    } else {
        Err(anyhow!("GPU Map error in aggregation"))
    }
}

pub(crate) fn run_vectorize_impl(
    processor: &GpuProcessor,
    source: GpuDataSource,
    offsets: &[u32],
    col_index: u32,
    delimiter: u8,
) -> Result<Vec<f32>> {
    let device = &processor.ctx.device;
    let queue = &processor.ctx.queue;

    let (data_buffer, data_len) = match source {
        GpuDataSource::Memory(data) => {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vec Data Buffer (RAM)"),
                contents: data,
                usage: BufferUsages::STORAGE,
            });
            (buffer, data.len())
        }
        GpuDataSource::File { path, offset, size } => {
            let buffer = device.create_buffer(&BufferDescriptor {
                label: Some("Vec Data Buffer (GDS)"),
                size,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let data = std::fs::read(path)?;
            queue.write_buffer(&buffer, 0, &data[offset as usize..(offset + size) as usize]);
            (buffer, size as usize)
        }
    };

    let offsets_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vec Offsets Buffer"),
        contents: bytemuck::cast_slice(offsets),
        usage: BufferUsages::STORAGE,
    });

    let num_rows = offsets.len() as u32;
    let result_size = num_rows * 4;
    let result_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Vec Result Buffer"),
        size: result_size as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let params = [data_len as u32, num_rows, col_index, delimiter as u32];
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vec Params Buffer"),
        contents: bytemuck::cast_slice(&params),
        usage: BufferUsages::UNIFORM,
    });

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Vec Bind Group"),
        layout: &processor.vec_pipeline.get_bind_group_layout(0),
        entries: &[
            BindGroupEntry { binding: 0, resource: data_buffer.as_entire_binding() },
            BindGroupEntry { binding: 1, resource: offsets_buffer.as_entire_binding() },
            BindGroupEntry { binding: 2, resource: result_buffer.as_entire_binding() },
            BindGroupEntry { binding: 3, resource: params_buffer.as_entire_binding() },
        ],
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Vec Encoder") });
    {
        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor { label: Some("Vec Pass"), timestamp_writes: None });
        cpass.set_pipeline(&processor.vec_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        let num_workgroups = (num_rows + 255) / 256;
        cpass.dispatch_workgroups(num_workgroups, 1, 1);
    }

    let staging_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Vec Staging Buffer"),
        size: result_size as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    encoder.copy_buffer_to_buffer(&result_buffer, 0, &staging_buffer, 0, result_size as u64);
    queue.submit(Some(encoder.finish()));

    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(MapMode::Read, move |res| { let _ = tx.send(res); });
    device.poll(Maintain::Wait);
    rx.recv()?.map_err(|e| anyhow!("GPU mapping failed: {:?}", e))?;

    let data_map = buffer_slice.get_mapped_range();
    let results: Vec<f32> = bytemuck::cast_slice(&data_map).to_vec();
    drop(data_map);
    staging_buffer.unmap();

    Ok(results)
}
