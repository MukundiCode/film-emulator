use eframe::wgpu;
use eframe::wgpu::{ComputePipeline, Device};
use crate::ViewportUniform::ViewportUniform;
use crate::ImageControls::ImageControls;

pub struct ImageRenderResources {
    pub render_pipeline: wgpu::RenderPipeline,
    pub render_bind_group: wgpu::BindGroup,

    pub compute_pipeline: ComputePipeline,
    pub compute_bind_group: wgpu::BindGroup,

    pub settings_buffer: wgpu::Buffer,
    pub viewport_buffer: wgpu::Buffer,
    
    pub processed_texture: wgpu::Texture,

    pub width: i32,
    pub height: i32
}

impl ImageRenderResources {
    pub fn prepare(
        &self,
        device: &Device,
        queue: &wgpu::Queue,
        controls: &ImageControls,
        view_rect: egui::Rect,
    ) {
        // --- COMPUTE PASS ---
        queue.write_buffer(
            &self.settings_buffer,
            0,
            bytemuck::bytes_of(controls),
        );
        {
            let mut encoder = device.create_command_encoder(
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
        }

        // --- VIEWPORT UNIFORM UPDATE ---
        let viewport = ViewportUniform {
            viewport_size: [
                view_rect.width(),
                view_rect.height(),
            ],
            image_size: [self.width as f32, self.height as f32],
            zoom: 0.5,
            _pad0: 0.0,
            pan: [0.0, 0.0],
        };

        queue.write_buffer(
            &self.viewport_buffer,
            0,
            bytemuck::bytes_of(&viewport),
        );
    }

    pub fn paint(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.render_bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}