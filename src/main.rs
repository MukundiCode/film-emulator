// 1. GLOBAL ATTRIBUTES MUST BE AT THE VERY TOP
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

mod image_loader;

use std::env;
use eframe::egui;
use egui::{AtomExt, Color32};
use crate::image_loader::load_image_to_linear_rgb;

// 2. INCLUDE THE GENERATED BINDINGS
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));


fn main() {

    let native_options = eframe::NativeOptions::default();
    eframe::run_native("My egui App", native_options, Box::new(|cc| Ok(Box::new(FilmEmulator::new(cc)))));

}

#[derive(Default)]
struct FilmEmulator {
    controls: ImageControls,
    image: ImageTextureView
}

impl FilmEmulator {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            controls: ImageControls::default(),
            image: ImageTextureView::default()
        }
    }
}

impl eframe::App for FilmEmulator {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.controls.ui(ui);
            self.image.ui(ui);// ← YOUR VIEW RUNS HERE
        });
    }
}

trait View {
    fn ui(&mut self, ui: &mut egui::Ui);
}


struct ImageControls {
    exposure: i32,
    contrast: i32,
    saturation: i32,
    brightness: i32,
    highlights: i32,
    shadows: i32
}

impl Default for ImageControls {
    fn default() -> Self {
        Self {
            exposure: 0,
            contrast: 0,
            saturation: 0,
            brightness: 0,
            highlights: 0,
            shadows: 0,
        }
    }
}

impl View for ImageControls {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Image Controls");

        ui.add(egui::Slider::new(&mut self.exposure, -100..=100).text("Exposure"));
        ui.add(egui::Slider::new(&mut self.contrast, -100..=100).text("Contrast"));
        ui.add(egui::Slider::new(&mut self.saturation, -100..=100).text("Saturation"));
        ui.add(egui::Slider::new(&mut self.brightness, -100..=100).text("Brightness"));
        ui.add(egui::Slider::new(&mut self.highlights, -100..=100).text("Highlights"));
        ui.add(egui::Slider::new(&mut self.shadows, -100..=100).text("Shadows"));
    }
}

#[derive(Default)]
struct ImageTextureView {
    texture: Option<egui::TextureHandle>,
}

impl ImageTextureView {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let texture: &egui::TextureHandle = self.texture.get_or_insert_with(|| {
            // Load the texture only once.
            let (width, height, pixels) = load_image_to_linear_rgb(&"images/DSC00495.ARW".to_string());
            let colour_32 = pixels.into_iter().map(|it|  {
                let denormalized: [u8; 3] = [
                    (it[0] * 255.0).clamp(0.0, 255.0) as u8,
                    (it[1] * 255.0).clamp(0.0, 255.0) as u8,
                    (it[2] * 255.0).clamp(0.0, 255.0) as u8,
                ];
                Color32::from_rgba_unmultiplied(denormalized[0], denormalized[1], denormalized[2], 255)
            }).collect();
            
            ui.ctx().load_texture(
                "my-image",
                egui::ColorImage::new([width as usize, height as usize], colour_32),
                Default::default()
            )
        });

        // Show the image:
        ui.add(egui::Image::new((texture.id(), texture.size_vec2())).max_width(1000.0));
    }
}




