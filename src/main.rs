// 1. GLOBAL ATTRIBUTES MUST BE AT THE VERY TOP
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use eframe::wgpu::util::DeviceExt;
mod image_loader;

use crate::image_loader::load_image_to_linear_rgb;
use eframe::egui_wgpu::{Callback, CallbackResources, CallbackTrait};
use eframe::wgpu::{BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, ComputePipeline, ComputePipelineDescriptor, Device, Extent3d, Origin3d, PipelineLayoutDescriptor, RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderStages, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureViewDimension, VertexState};
use eframe::{egui, wgpu};
use std::env;

// 2. INCLUDE THE GENERATED BINDINGS
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));


fn main() {

    let native_options = eframe::NativeOptions::default();
    eframe::run_native("My egui App", native_options,
                       Box::new(|cc| Ok(Box::new(FilmEmulator::new(cc).expect("Error starting up")))))
        .expect("TODO: panic message");

}

struct FilmEmulator {
    controls: ImageControls,
    image: ImageTextureView,
    gpu_image_pipeline: GpuImageRenderPipeline,
    gpu_image_compute_pipeline: GpuImageComputePipeline
}

impl FilmEmulator {
    fn new(cc: &eframe::CreationContext<'_>) -> Option<Self> {

        let wgpu_render_state = cc.wgpu_render_state.as_ref()?;
        let device = &wgpu_render_state.device;
        let queue = &wgpu_render_state.queue;
        let gpu_image_pipeline = GpuImageRenderPipeline::new(&device);
        let gpu_compute_pipeline = GpuImageComputePipeline::new(&device);
        let image_texture_view = ImageTextureView::default();
        let image_controls = ImageControls::default();

        let (width, height, pixels) = load_image_to_linear_rgb(&"images/DSC00495.ARW".to_string());
        let mut rgba_pixels = Vec::<f32>::with_capacity((width * height * 4) as usize);

        for [r, g, b] in pixels {
            rgba_pixels.push(r);
            rgba_pixels.push(g);
            rgba_pixels.push(b);
            rgba_pixels.push(1.0); // alpha
        }

        let texture = device.create_texture(&TextureDescriptor {
            label: Some("RAW Linear RGB Texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float, // IMPORTANT
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::STORAGE_BINDING, // for compute shaders later
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("RAW Texture View"),
            ..Default::default()
        });

        let processed_texture = device.create_texture(&TextureDescriptor {
            label: Some("Processed Texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let processed_view = processed_texture.create_view(&Default::default());

        let settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Settings Buffer"),
            contents: bytemuck::cast_slice(&[image_controls]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &gpu_compute_pipeline.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&processed_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(
                        wgpu::BufferBinding {
                            buffer: &settings_buffer,
                            offset: 0,
                            size: None,
                        },
                    ),
                },
            ],
        });

        let compute_pipeline = gpu_compute_pipeline.pipeline.clone();

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(&rgba_pixels),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(16 * width),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Bind group"),
            layout: &gpu_image_pipeline.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&processed_view),
                }
            ],
        });

        let render_pipeline = gpu_image_pipeline.pipeline.clone();
        wgpu_render_state
            .renderer
            .write()
            .callback_resources
            .insert(ImageRenderResources {
                render_pipeline,
                render_bind_group: bind_group,
                compute_pipeline,
                compute_bind_group,
                settings_buffer,
                texture,
                rgba_pixels,
                width: width as i32,
                height: height as i32,
            });

        Some(Self {
            controls: image_controls,
            image: image_texture_view,
            gpu_image_pipeline,
            gpu_image_compute_pipeline: gpu_compute_pipeline
        })
    }
}

impl eframe::App for FilmEmulator {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {

        if let Some(rs) = frame.wgpu_render_state() {
            // ⬇️ Extract first
            let device = &rs.device;
            let queue = &rs.queue;

            // ⬇️ Then mutably borrow renderer
            let mut renderer = rs.renderer.write();

            let resources = renderer
                .callback_resources
                .get_mut::<ImageRenderResources>()
                .unwrap();

            resources.prepare(device, queue, &self.controls);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            self.controls.ui(ui);
            self.image.ui(ui);
        });
    }
}


trait View {
    fn ui(&mut self, ui: &mut egui::Ui);
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ImageControls {
    exposure: f32,
    contrast: f32,
    saturation: f32,
    brightness: f32,
    highlights: f32,
    shadows: f32,
}


impl Default for ImageControls {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 0.0,
            saturation: 0.0,
            brightness: 0.0,
            highlights: 0.0,
            shadows: 0.0,
        }
    }
}

impl View for ImageControls {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Image Controls");

        ui.add(egui::Slider::new(&mut self.exposure, -10.0..=10.0).text("Exposure"));
        ui.add(egui::Slider::new(&mut self.contrast, -10.0..=10.0).text("Contrast"));
        ui.add(egui::Slider::new(&mut self.saturation, -10.0..=10.0).text("Saturation"));
        ui.add(egui::Slider::new(&mut self.brightness, -10.0..=10.0).text("Brightness"));
        ui.add(egui::Slider::new(&mut self.highlights, -10.0..=10.0).text("Highlights"));
        ui.add(egui::Slider::new(&mut self.shadows, -10.0..=10.0).text("Shadows"));
    }
}

#[derive(Default)]
struct ImageTextureView;

impl ImageTextureView {
    fn ui(&self, ui: &mut egui::Ui) {
        let desired_size = egui::vec2(2000.0, 1500.0);

        let (rect, _response) = ui.allocate_at_least(desired_size, egui::Sense::hover());

        let callback = Callback::new_paint_callback(
            rect,
            ImagePaintCallback,
        );

        ui.painter().add(callback);
    }
}

struct ImagePaintCallback;

impl CallbackTrait for ImagePaintCallback {
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'_>,
        resources: &CallbackResources,
    ) {
        let image = resources
            .get::<ImageRenderResources>()
            .expect("ImageRenderResources not found");

        image.paint(render_pass);
    }
}

struct GpuImageRenderPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl GpuImageRenderPipeline {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Image Settings shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Render Bind Group"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TextureFormat::Bgra8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });


        Self {
            pipeline,
            bind_group_layout,
        }
    }
}

struct ImageRenderResources {
    render_pipeline: wgpu::RenderPipeline,
    render_bind_group: wgpu::BindGroup,

    compute_pipeline: ComputePipeline,
    compute_bind_group: wgpu::BindGroup,

    settings_buffer: wgpu::Buffer,

    texture: wgpu::Texture,
    rgba_pixels: Vec<f32>,
    width: i32,
    height: i32
}

impl ImageRenderResources {
    fn prepare(&self, _device: &Device, queue: &wgpu::Queue, controls: &ImageControls) {
        // Update our uniform buffer with the angle from the UI
        queue.write_buffer(
            &self.settings_buffer,
            0,
            bytemuck::bytes_of(controls),
        );

        let mut encoder = _device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("Compute Encoder"),
            },
        );

        {
            let mut cpass = encoder.begin_compute_pass(&Default::default());
            cpass.set_pipeline(&self.compute_pipeline);
            cpass.set_bind_group(0, &self.compute_bind_group, &[]);

            let gx = ((self.width + 15) / 16) as u32;
            let gy = ((self.height + 15) / 16) as u32;
            cpass.dispatch_workgroups(gx, gy, 1);
        }

        queue.submit(Some(encoder.finish()));
        // queue.write_texture(
        //     TexelCopyTextureInfo {
        //         texture: &self.texture,
        //         mip_level: 0,
        //         origin: Origin3d::ZERO,
        //         aspect: TextureAspect::All,
        //     },
        //     bytemuck::cast_slice(&self.rgba_pixels),
        //     TexelCopyBufferLayout {
        //         offset: 0,
        //         bytes_per_row: Some((16 * self.width) as u32),
        //         rows_per_image: Some(self.height as u32),
        //     },
        //     Extent3d {
        //         width: self.width as u32,
        //         height: self.height as u32,
        //         depth_or_array_layers: 1,
        //     },
        // );
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.render_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

struct GpuImageComputePipeline {
    pipeline: ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl GpuImageComputePipeline {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Image Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/compute.wgsl").into()
            ),
        });

        let bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Compute Bind Group Layout"),
                entries: &[
                    // input texture
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: false },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // output texture
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: TextureFormat::Rgba8Unorm,
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    // uniform buffer
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout =
            device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Compute Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        let pipeline =
            device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Image Compute Pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("shader_main"),
                compilation_options: Default::default(),
                cache: None,
            });

        Self { pipeline, bind_group_layout }
    }
}
