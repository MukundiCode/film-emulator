#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ViewportUniform {
    pub viewport_size: [f32; 2], // egui rect width, height
    pub image_size: [f32; 2],    // image width, height
    pub zoom: f32,
    pub _pad0: f32,
    pub pan: [f32; 2],           // normalized pan
}
