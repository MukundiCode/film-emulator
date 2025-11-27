// 1. GLOBAL ATTRIBUTES MUST BE AT THE VERY TOP
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use image::{ImageBuffer, Rgb, ImageReader};
use std::env;
use std::ffi::CString;
use std::slice;
use clap::builder::TypedValueParser;
use clap::Parser;

// 2. INCLUDE THE GENERATED BINDINGS
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// --- MATH HELPER FUNCTIONS ---

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

fn dot(v1: [f32; 3], v2: [f32; 3]) -> f32 {
    v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2]
}

// --- THE NEW LIBRAW LOADER (Using Unsafe C-Bindings) ---

fn load_image_to_linear_rgb(path: &String) -> (u32, u32, Vec<[f32; 3]>) {
    let lower = path.to_lowercase();
    let is_raw = lower.ends_with(".dng") || lower.ends_with(".cr2") ||
        lower.ends_with(".nef") || lower.ends_with(".arw") ||
        lower.ends_with(".raf") || lower.ends_with(".orf") ||
        lower.ends_with(".cr3");

    if is_raw {
        println!("Loading RAW via LibRaw C-API...");

        unsafe {
            // 1. Init LibRaw
            let raw_data = libraw_init(0);
            if raw_data.is_null() {
                panic!("Failed to init LibRaw");
            }

            // 2. Open File
            let c_path = CString::new(path.as_str()).expect("CString failed");
            if libraw_open_file(raw_data, c_path.as_ptr()) != 0 {
                panic!("LibRaw could not open file");
            }

            // 3. Unpack (Decompress)
            if libraw_unpack(raw_data) != 0 {
                panic!("LibRaw unpack failed");
            }

            // 4. CONFIGURE PARAMS (Critical for Film Emulation)

            // Disable Gamma Curve (Set to 1.0 linear)
            (*raw_data).params.gamm[0] = 1.0;
            (*raw_data).params.gamm[1] = 1.0;

            // Disable Auto Brightness (Histogram stretching)
            (*raw_data).params.no_auto_bright = 1;

            // Request 16-bit output (Linear needs precision)
            (*raw_data).params.output_bps = 16;

            // Use Camera White Balance
            (*raw_data).params.use_camera_wb = 1;

            // 5. Process (Demosaic)
            if libraw_dcraw_process(raw_data) != 0 {
                panic!("LibRaw processing failed");
            }

            // 6. Get Memory Image
            // FIXED: Used the function name suggested by the compiler
            let mut err = 0;
            let processed = libraw_dcraw_make_mem_image(raw_data, &mut err);

            if processed.is_null() || err != 0 {
                panic!("Failed to create memory image");
            }

            // 7. Read Data
            let width = (*processed).width as u32;
            let height = (*processed).height as u32;
            let data_size = (*processed).data_size as usize;

            let data_ptr = (*processed).data.as_ptr();

            println!("Raw Decode: {}x{} (16-bit Linear)", width, height);

            let raw_slice = slice::from_raw_parts(data_ptr, data_size);

            let mut out_pixels = Vec::with_capacity((width * height) as usize);

            // Iterate 6 bytes at a time (2 bytes Red + 2 bytes Green + 2 bytes Blue)
            for chunk in raw_slice.chunks_exact(6) {
                // Parse Little Endian u16
                let r = u16::from_ne_bytes([chunk[0], chunk[1]]) as f32 / 65535.0;
                let g = u16::from_ne_bytes([chunk[2], chunk[3]]) as f32 / 65535.0;
                let b = u16::from_ne_bytes([chunk[4], chunk[5]]) as f32 / 65535.0;

                out_pixels.push([r, g, b]);
            }

            // Cleanup C memory
            libraw_dcraw_clear_mem(processed);
            libraw_close(raw_data);

            return (width, height, out_pixels);
        }
    }

    // Fallback for Standard Images
    println!("Loading Standard Image via image crate...");
    let img = ImageReader::open(path).unwrap().decode().unwrap().to_rgb8();
    let (w, h) = img.dimensions();
    let mut out = Vec::with_capacity((w * h) as usize);

    for (_, _, pixel) in img.enumerate_pixels() {
        out.push([
            (pixel[0] as f32 / 255.0).powf(2.2), // Gamma Decode
            (pixel[1] as f32 / 255.0).powf(2.2),
            (pixel[2] as f32 / 255.0).powf(2.2),
        ]);
    }
    (w, h, out)
}

fn main() {
    let args = Args::parse();

    let output_path = args.image_out;

    let (width, height, pixels) = load_image_to_linear_rgb(&args.image_in);
    let mut output_buffer = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(width, height);

    println!("Processing {} pixels...", width * height);

    // creating nodes
    let mut nodes: Vec<Box<dyn Node>> = Vec::new();
    match args.exposure {
        None => {}
        Some(exposure) => {nodes.push(Box::new(ExposureNode {exposure}))}
    }

    match args.contrast {
        None => {}
        Some(contrast) => { nodes.push(Box::new(ContrastNode {contrast}))}
    }

    match args.saturation {
        None => {}
        Some(saturation) => { nodes.push(Box::new(SaturationNode {saturation}))}
    }

    nodes.push(Box::new(SubtractiveDensityNode {
        density_saturation: args.density_saturation
    }));

    nodes.push(Box::new(FilmCurveNode {
        k_contrast: args.k_contrast,
        x0_offset: args.x0_offset
    }));

    // nodes.push(Box::new(GammaEncodeNode {
    //     gamma: 1.0 / 2.2
    // }));

    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) as usize;
            let [r, g, b] = pixels[i];

            let mut pixel = Rgb([r, g, b]);
            for node in &nodes {
                pixel = node.transform(pixel);
            }

            // Gamma Encode
            let gamma = 1.0 / 2.2;
            output_buffer
                .put_pixel(x, y, Rgb(pixel.0.map(|x| (x.powf(gamma) * 255.0).clamp(0.0, 255.9) as u8)));
        }
    }

    // for y in 0..height {
    //     for x in 0..width {
    //         let i = (y * width + x) as usize;
    //         let [r, g, b] = pixels[i];
    //
    //         let dense = apply_subtractive_density([r, g, b], density_saturation);
    //
    //         let r_fin = film_curve(dense[0], k_contrast, x0_offset);
    //         let g_fin = film_curve(dense[1], k_contrast, x0_offset);
    //         let b_fin = film_curve(dense[2], k_contrast, x0_offset);
    //
    //         // Gamma Encode
    //         let gamma = 1.0 / 2.2;
    //         let out_r = (r_fin.powf(gamma) * 255.0).clamp(0.0, 255.0) as u8;
    //         let out_g = (g_fin.powf(gamma) * 255.0).clamp(0.0, 255.0) as u8;
    //         let out_b = (b_fin.powf(gamma) * 255.0).clamp(0.0, 255.0) as u8;
    //
    //         output_buffer.put_pixel(x, y, Rgb([out_r, out_g, out_b]));
    //     }
    // }

    output_buffer.save(&output_path).unwrap();
    println!("Success! Saved to {}", output_path);
}

trait Node {
    fn transform(&self, pixel: Rgb<f32>) -> Rgb<f32>;
}

struct ExposureNode {
    exposure: f32
}

impl Node for ExposureNode {
    fn transform(&self, pixel: Rgb<f32>) -> Rgb<f32> {
        Rgb(pixel.0.map(|x| x * 2.0_f32.powf(self.exposure)))
    }
}

struct ContrastNode {
    contrast: f32
}

impl Node for ContrastNode {
    fn transform(&self, pixel: Rgb<f32>) -> Rgb<f32> {
        Rgb(pixel.0.map(|x| (x - 0.5) * (1.0 + self.contrast) + 0.5)) // assuming they have been normalized
    }
}

struct  SaturationNode {
    saturation: f32
}

impl Node for SaturationNode {
    fn transform(&self, pixel: Rgb<f32>) -> Rgb<f32> {
        let L = 0.299*pixel.0[0] + 0.587*pixel.0[1] + 0.114*pixel.0[2];
        Rgb(pixel.0.map(|x| L + (x - L) * (1.0 + self.saturation)))
    }
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

struct GammaEncodeNode {
    gamma: f32
}

impl Node for GammaEncodeNode {
    fn transform(&self, pixel: Rgb<f32>) -> Rgb<f32> {
        todo!()
    }
}

#[derive(Parser)]
struct Args {
    image_in: String,

    #[arg(default_value = "film_output.png")]
    image_out: String,

    #[arg(short, long)]
    exposure: Option<f32>,

    #[arg(short, long)]
    contrast: Option<f32>,

    #[arg(short, long)]
    saturation: Option<f32>,

    #[arg(default_value_t = 1.4)]
    density_saturation: f32,

    #[arg(default_value_t = 5.5)]
    k_contrast: f32,

    #[arg(default_value_t = 0.4)]
    x0_offset: f32
}
/*
- Exposure
- Contrast
- Saturation
-
 */