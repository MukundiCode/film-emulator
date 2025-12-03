// 1. GLOBAL ATTRIBUTES MUST BE AT THE VERY TOP
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

mod pipeline;
mod image_loader;
mod window;

use image::{Rgb};
use std::default::Default;
use std::env;
use clap::Parser;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};


// 2. INCLUDE THE GENERATED BINDINGS
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// --- MATH HELPER FUNCTIONS ---

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

fn dot(v1: [f32; 3], v2: [f32; 3]) -> f32 {
    v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2]
}


fn main() {
    
    // let (width, height, pixels) = load_image_to_linear_rgb(&args.image_in);
    // take in image, for now just add a sample image
    // init app, should return an object Settings
    // pass to pipeline, which should return a texture
    // texture should be passed into the app, first converted into the egui texture

}

trait Node {
    fn transform(&self, pixel: Rgb<f32>) -> Rgb<f32>;
}

struct SubtractiveDensityNode {
    density_saturation: f32
}

impl Node for SubtractiveDensityNode {
    fn transform(&self, pixel: Rgb<f32>) -> Rgb<f32> {
        let cmy = [1.0 - pixel.0[0], 1.0 - pixel.0[1], 1.0 - pixel.0[2]];
        let luma_weights = [0.2126, 0.7152, 0.0722];
        let dye_luma = dot(cmy, luma_weights);

        let cmy_dense = [
            mix(dye_luma, cmy[0], self.density_saturation),
            mix(dye_luma, cmy[1], self.density_saturation),
            mix(dye_luma, cmy[2], self.density_saturation)
        ];

        Rgb([
            (1.0 - cmy_dense[0]).clamp(0.0, 1.0),
            (1.0 - cmy_dense[1]).clamp(0.0, 1.0),
            (1.0 - cmy_dense[2]).clamp(0.0, 1.0)
        ])
    }
}

struct FilmCurveNode {
    k_contrast: f32,
    x0_offset: f32
}

impl Node for FilmCurveNode {
    fn transform(&self, pixel: Rgb<f32>) -> Rgb<f32> {
        Rgb(pixel.0.map(|x| 1.0 / (1.0 + (-self.k_contrast * (x - self.x0_offset)).exp())))
    }
}

