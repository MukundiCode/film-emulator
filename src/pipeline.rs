use std::default::Default;
use wgpu::{InstanceDescriptor};
use wgpu::util::DeviceExt;
use pollster::FutureExt;
use bevy::prelude::*;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Settings {
    exposure: f32,
    contrast: f32,
    saturation: f32
}

fn setup_pipeline(width: u32, height: u32, pixels: Vec<[f32; 3]>, settings: Settings) -> Result<(), Box<dyn std::error::Error>> {
    let instance = wgpu::Instance::new(&InstanceDescriptor::from_env_or_default());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .block_on()
        .expect("Failed to find adapter");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .block_on()?;

    // --- Create textures ---
    let texture_size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let input_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Input Texture"),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let mut rgba8 = Vec::with_capacity((width * height * 4) as usize);

    for [r, g, b] in pixels {
        rgba8.push((r * 255.0).clamp(0.0, 255.0) as u8);
        rgba8.push((g * 255.0).clamp(0.0, 255.0) as u8);
        rgba8.push((b * 255.0).clamp(0.0, 255.0) as u8);
        rgba8.push(255); // alpha
    }

    queue.write_texture(
        input_texture.as_image_copy(),
        &rgba8,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: None,
        },
        texture_size,
    );

    let output_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Output Texture"),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    // --- Bind Group ---

    let settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Settings Uniform Buffer"),
        contents: bytemuck::bytes_of(&settings),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Pipeline BindGroupLayout"),
        entries: &[
            // binding 0: input texture
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                },
                count: None,
            },
            // binding 1: output storage texture
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            // binding 2: uniform buffer
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<Settings>() as u64),
                },
                count: None,
            },
        ],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Texture + Uniform BindGroup"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&input_texture.create_view(&wgpu::TextureViewDescriptor::default())),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&output_texture.create_view(&wgpu::TextureViewDescriptor::default())),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: settings_buffer.as_entire_binding(),
            },
        ],
    });

    // --- Create compute pipeline ---

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Compute Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Grayscale Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Grayscale Pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("shader_main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    // --- Dispatch compute ---

    let mut encoder = device.create_command_encoder(&Default::default());

    let (wg_x, wg_y) = (
        (width + 15) / 16,
        (height + 15) / 16,
    );

    {
        let mut compute_pass = encoder.begin_compute_pass(&Default::default());
        compute_pass.set_pipeline(&pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(wg_x, wg_y, 1);
    }

    // --- Copy result into CPU buffer ---

    let padded_bytes_per_row = ((width * 4 + 255) / 256) * 256;
    let output_buffer_size = padded_bytes_per_row as u64 * height as u64;

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: output_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        output_texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &output_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        texture_size,
    );

    queue.submit(Some(encoder.finish()));

    // --- Read back data ---

    let buffer_slice = output_buffer.slice(..);
    buffer_slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely())?;

    let data = buffer_slice.get_mapped_range();

    let mut final_pixels = vec![0u8; (width * height * 4) as usize];

    for (row, chunk) in data
        .chunks_exact(padded_bytes_per_row as usize)
        .zip(final_pixels.chunks_exact_mut((width * 4) as usize))
    {
        chunk.copy_from_slice(&row[..(width * 4) as usize]);
    }

    drop(data);
    output_buffer.unmap();

    // Save
    let img = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, final_pixels)
        .expect("Failed to construct image");

    // img.save(&output_path)?;
    // println!("Saved to {}", output_path);

    Ok(())
}

fn compute_work_group_count(
    (width, height): (u32, u32),
    (workgroup_width, workgroup_height): (u32, u32),
) -> (u32, u32) {
    let x = (width + workgroup_width - 1) / workgroup_width;
    let y = (height + workgroup_height - 1) / workgroup_height;

    (x, y)
}

fn padded_bytes_per_row(width: u32) -> usize {
    let bytes_per_row = width as usize * 4;
    let padding = (256 - bytes_per_row % 256) % 256;
    bytes_per_row + padding
}