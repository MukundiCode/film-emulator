// 1. GLOBAL ATTRIBUTES MUST BE AT THE VERY TOP
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

mod image_loader;
mod FilmEmulator;
mod ImageControls;
mod ImageTextureView;
mod ImagePaintCallback;
mod GpuImageRenderPipeline;
mod ImageRenderResources;
mod GpuImageComputePipeline;
mod ViewportUniform;

use eframe::{egui};
use std::env;
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));


fn main() {

    let native_options = eframe::NativeOptions::default();
    eframe::run_native("My egui App", native_options,
                       Box::new(|cc| Ok(Box::new(FilmEmulator::FilmEmulator::new(cc).expect("Error starting up")))))
        .expect("TODO: panic message");

}

pub trait View {
    fn ui(&mut self, ui: &mut egui::Ui);
}

