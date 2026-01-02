use crate::ImageControls::ImageControls;
use crate::ImageTextureView::ImageTextureView;
use crate::GpuImageRenderPipeline::GpuImageRenderPipeline;
use crate::GpuImageComputePipeline::GpuImageComputePipeline;
use crate::ImageRenderResources::ImageRenderResources;
use crate::ViewportUniform::ViewportUniform;
use crate::View;
use eframe::wgpu;
use eframe::wgpu::util::DeviceExt;
use eframe::wgpu::{BindGroupDescriptor, BindGroupEntry, BindingResource, Extent3d, Origin3d,
                   TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages};
use crate::image_loader::load_image_to_linear_rgb;
use egui_file_dialog::FileDialog;
use std::path::PathBuf;

pub struct FilmEmulator {
    file_dialog: FileDialog,
    selected_image_path: Option<PathBuf>,
    controls: Option<ImageControls>,
    image: Option<ImageTextureView>,
    image_loaded: bool,
    export_pending: bool,
}

impl FilmEmulator {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Option<Self> {
        use std::sync::Arc;
        
        let file_dialog = FileDialog::new().add_file_filter(
            "Image Files",
            Arc::new(|path| {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    matches!(ext_str.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "gif" | "tiff" | "tif" 
                        | "arw" | "cr2" | "nef" | "dng" | "raf" | "orf" | "cr3")
                } else {
                    false
                }
            }),
        );
        
        Some(Self {
            file_dialog,
            selected_image_path: None,
            controls: None,
            image: None,
            image_loaded: false,
            export_pending: false,
        })
    }
    
    fn load_image(&mut self, path: PathBuf, wgpu_render_state: &eframe::egui_wgpu::RenderState) {
        let device = &wgpu_render_state.device;
        let queue = &wgpu_render_state.queue;
        let gpu_render_pipeline = GpuImageRenderPipeline::new(&device);
        let gpu_compute_pipeline = GpuImageComputePipeline::new(&device);
        let mut image_texture_view = ImageTextureView::default();
        let image_controls = ImageControls::default();

        let path_str = path.to_string_lossy().to_string();
        let (width, height, pixels) = load_image_to_linear_rgb(&path_str);
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
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::STORAGE_BINDING,
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
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let processed_view = processed_texture.create_view(&Default::default());

        let settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Settings Buffer"),
            contents: bytemuck::cast_slice(&[image_controls]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let viewport_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Viewport buffer"),
            size: size_of::<ViewportUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &gpu_compute_pipeline.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&processed_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Buffer(
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

        let render_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Render Bind Group"),
            layout: &gpu_render_pipeline.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&processed_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: viewport_buffer.as_entire_binding(),
                },
            ],
        });

        let render_pipeline = gpu_render_pipeline.pipeline.clone();
        wgpu_render_state
            .renderer
            .write()
            .callback_resources
            .insert(ImageRenderResources {
                render_pipeline,
                render_bind_group,
                compute_pipeline,
                compute_bind_group,
                settings_buffer,
                viewport_buffer,
                processed_texture,
                width: width as i32,
                height: height as i32,
            });

        image_texture_view.image_width = width as f32;
        image_texture_view.image_height = height as f32;

        self.controls = Some(image_controls);
        self.image = Some(image_texture_view);
        self.image_loaded = true;
    }
    
    fn export_image(&self, save_path: PathBuf, wgpu_render_state: &eframe::egui_wgpu::RenderState) {
        use image::{ImageBuffer, Rgba};
        
        let device = &wgpu_render_state.device;
        let queue = &wgpu_render_state.queue;
        
        let renderer = wgpu_render_state.renderer.read();
        if let Some(resources) = renderer
            .callback_resources
            .get::<ImageRenderResources>()
        {
            let width = resources.width as u32;
            let height = resources.height as u32;
            
            // Calculate aligned bytes per row (must be multiple of 256)
            let bytes_per_pixel = 4u32; // RGBA8
            let unpadded_bytes_per_row = width * bytes_per_pixel;
            let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
            
            // Create buffer to copy texture data to
            let buffer_size = (padded_bytes_per_row * height) as u64;
            let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Output Buffer"),
                size: buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            
            // Copy texture to buffer
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Export Encoder"),
            });
            
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &resources.processed_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &output_buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_per_row),
                        rows_per_image: Some(height),
                    },
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            
            queue.submit(Some(encoder.finish()));
            
            // Map the buffer and save
            let buffer_slice = output_buffer.slice(..);
            let (sender, receiver) = std::sync::mpsc::channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                sender.send(result).ok();
            });
            
            // Device will be polled by the application event loop
            queue.submit(std::iter::empty());
            
            // Wait a moment for the buffer to map
            std::thread::sleep(std::time::Duration::from_millis(100));
            
            if receiver.try_recv().is_ok() {
                let data = buffer_slice.get_mapped_range();
                
                // Handle padded rows - extract only the actual image data
                let mut image_data = Vec::with_capacity((width * height * bytes_per_pixel) as usize);
                for row in 0..height {
                    let row_start = (row * padded_bytes_per_row) as usize;
                    let row_end = row_start + (width * bytes_per_pixel) as usize;
                    image_data.extend_from_slice(&data[row_start..row_end]);
                }
                
                let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(
                    width,
                    height,
                    image_data,
                )
                .expect("Failed to create image buffer");
                
                img.save(&save_path).expect("Failed to save image");
                println!("Image exported to: {:?}", save_path);
                
                drop(data);
                output_buffer.unmap();
            }
        }
    }
}

impl eframe::App for FilmEmulator {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Update file dialog once and get state
        let dialog_state = self.file_dialog.update(ctx);
        let picked_path = dialog_state.picked().map(|p| p.to_path_buf());
        
        // Handle export if pending
        if self.export_pending {
            if let Some(path) = picked_path {
                // Save dialog was confirmed with a path
                if let Some(rs) = frame.wgpu_render_state() {
                    self.export_image(path, rs);
                }
                self.export_pending = false;
            }
        } else {
            // Handle normal file opening (not export)
            if let Some(path) = picked_path {
                // Only load if it's a different image than what's currently loaded
                let should_load = self.selected_image_path.as_ref() != Some(&path);
                
                if should_load {
                    self.selected_image_path = Some(path.clone());
                    
                    // Try to load the image
                    if let Some(rs) = frame.wgpu_render_state() {
                        self.load_image(path, rs);
                    }
                }
            }
        }

        // Prepare resources if image is loaded
        if self.image_loaded {
            if let (Some(rs), Some(image)) = (frame.wgpu_render_state(), &self.image) {
                if let Some(rect) = image.last_rect {
                    let device = &rs.device;
                    let queue = &rs.queue;

                    let mut renderer = rs.renderer.write();
                    if let Some(resources) = renderer
                        .callback_resources
                        .get_mut::<ImageRenderResources>()
                    {
                        if let Some(controls) = &self.controls {
                            resources.prepare(device, queue, controls, rect);
                        }
                    }
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.image_loaded {
                // Show file picker UI when no image is loaded
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.heading("Film Emulator");
                    ui.add_space(20.0);
                    ui.label("Select an image to begin editing");
                    ui.add_space(20.0);
                    
                    if ui.button("Select Image").clicked() {
                        self.file_dialog.pick_file();
                    }
                });
            } else {
                // Show controls and image when loaded
                if let Some(controls) = &mut self.controls {
                    controls.ui(ui);
                    
                    ui.separator();
                    
                    // Export button
                    if ui.button("Export Image").clicked() {
                        self.export_pending = true;
                        self.file_dialog.save_file();
                    }
                }
                if let Some(image) = &mut self.image {
                    image.ui(ui);
                }
            }
        });
    }
}
