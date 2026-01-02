use crate::View;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ImageControls {
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
            contrast: 1.0,
            saturation: 1.0,
            brightness: 0.0,
            highlights: 0.0,
            shadows: 0.0,
        }
    }
}

impl View for ImageControls {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Image Controls");

        ui.add(egui::Slider::new(&mut self.exposure, -3.0..=3.0).text("Exposure"));
        ui.add(egui::Slider::new(&mut self.contrast, 0.0..=2.0).text("Contrast"));
        ui.add(egui::Slider::new(&mut self.saturation, 0.0..=3.0).text("Saturation"));
        ui.add(egui::Slider::new(&mut self.brightness, -0.5..=0.5).text("Brightness"));
        ui.add(egui::Slider::new(&mut self.highlights, -1.0..=1.0).text("Highlights"));
        ui.add(egui::Slider::new(&mut self.shadows, -0.5..=0.5).text("Shadows"));
    }
}