use eframe::egui_wgpu::{CallbackResources, CallbackTrait};
use eframe::wgpu;
use crate::ImageRenderResources::ImageRenderResources;

pub struct ImagePaintCallback;

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