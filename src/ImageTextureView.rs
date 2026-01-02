use eframe::egui_wgpu::Callback;
use crate::ImagePaintCallback::ImagePaintCallback;

pub struct ImageTextureView {
    pub last_rect: Option<egui::Rect>,
    pub image_width: f32,
    pub image_height: f32,
}

impl Default for ImageTextureView {
    fn default() -> Self {
        Self {
            last_rect: None,
            image_width: 1.0,
            image_height: 1.0,
        }
    }
}

impl ImageTextureView {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Calculate desired size based on actual image aspect ratio
        let image_aspect = self.image_width / self.image_height;
        let max_width = ui.available_width();
        let max_height = ui.available_height();
        
        let desired_size = if max_width / max_height > image_aspect {
            // Viewport is wider than image - fit to height
            egui::vec2(max_height * image_aspect, max_height)
        } else {
            // Viewport is taller than image - fit to width
            egui::vec2(max_width, max_width / image_aspect)
        };

        let (rect, _response) = ui.allocate_at_least(desired_size, egui::Sense::hover());

        self.last_rect = Some(rect);

        let callback = Callback::new_paint_callback(
            rect,
            ImagePaintCallback,
        );

        ui.painter().add(callback);
    }
}