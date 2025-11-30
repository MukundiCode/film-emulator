// 1. GLOBAL ATTRIBUTES MUST BE AT THE VERY TOP
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use image::{Rgb, ImageReader};
use std::default::Default;
use std::env;
use std::ffi::CString;
use std::slice;
use clap::Parser;
use wgpu::{InstanceDescriptor};
use pollster::FutureExt;


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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let output_path = args.image_out;

    let (width, height, pixels) = load_image_to_linear_rgb(&args.image_in);

    println!("Processing {} pixels...", width * height);

    // --- GPU Setup ---

    let instance = wgpu::Instance::new(&InstanceDescriptor::from_env_or_default());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .block_on()
        .expect("Failed to find adapter");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .block_on()?;

    // --- Create textures ---
    let texture_size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let input_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Input Texture"),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        input_texture.as_image_copy(),
        bytemuck::cast_slice(&pixels),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: None,
        },
        texture_size,
    );

    let output_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Output Texture"),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    // --- Create compute pipeline ---

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Grayscale Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Grayscale Pipeline"),
        layout: None,
        module: &shader,
        entry_point: Some("shader_main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    // --- Bind Group ---

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Textures Bind Group"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &input_texture.create_view(&Default::default()),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(
                    &output_texture.create_view(&Default::default()),
                ),
            },
        ],
    });

    // --- Dispatch compute ---

    let mut encoder = device.create_command_encoder(&Default::default());

    let (wg_x, wg_y) = (
        (width + 15) / 16,
        (height + 15) / 16,
    );

    {
        let mut compute_pass = encoder.begin_compute_pass(&Default::default());
        compute_pass.set_pipeline(&pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(wg_x, wg_y, 1);
    }

    // --- Copy result into CPU buffer ---

    let padded_bytes_per_row = ((width * 4 + 255) / 256) * 256;
    let output_buffer_size = padded_bytes_per_row as u64 * height as u64;

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: output_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        output_texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &output_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        texture_size,
    );

    queue.submit(Some(encoder.finish()));

    // --- Read back data ---

    let buffer_slice = output_buffer.slice(..);
    buffer_slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely())?;

    let data = buffer_slice.get_mapped_range();

    let mut final_pixels = vec![0u8; (width * height * 4) as usize];

    for (row, chunk) in data
        .chunks_exact(padded_bytes_per_row as usize)
        .zip(final_pixels.chunks_exact_mut((width * 4) as usize))
    {
        chunk.copy_from_slice(&row[..(width * 4) as usize]);
    }

    drop(data);
    output_buffer.unmap();

    // Save
    let img = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, final_pixels)
        .expect("Failed to construct image");

    img.save(&output_path)?;
    println!("Saved to {}", output_path);

    Ok(())


    // // creating nodes
    // let mut nodes: Vec<Box<dyn Node>> = Vec::new();
    // match args.exposure {
    //     None => {}
    //     Some(exposure) => {nodes.push(Box::new(ExposureNode {exposure}))}
    // }
    //
    // match args.contrast {
    //     None => {}
    //     Some(contrast) => { nodes.push(Box::new(ContrastNode {contrast}))}
    // }
    //
    // match args.saturation {
    //     None => {}
    //     Some(saturation) => { nodes.push(Box::new(SaturationNode {saturation}))}
    // }
    //
    // nodes.push(Box::new(SubtractiveDensityNode {
    //     density_saturation: args.density_saturation
    // }));
    //
    // nodes.push(Box::new(FilmCurveNode {
    //     k_contrast: args.k_contrast,
    //     x0_offset: args.x0_offset
    // }));
    //
    // nodes.push(Box::new(GammaEncodeNode {
    //     gamma: 1.0 / 2.2
    // }));
    //
    // for y in 0..height {
    //     for x in 0..width {
    //         let i = (y * width + x) as usize;
    //         let [r, g, b] = pixels[i];
    //
    //         let mut pixel = Rgb([r, g, b]);
    //         for node in &nodes {
    //             pixel = node.transform(pixel);
    //         }
    //
    //         // Gamma Encode
    //         let gamma = 1.0 / 2.2;
    //         output_buffer
    //             .put_pixel(x, y, Rgb(pixel.0.map(|x| (x.powf(gamma) * 255.0).clamp(0.0, 255.9) as u8)));
    //     }
    // }
    //
    // output_buffer.save(&output_path).unwrap();
    // println!("Success! Saved to {}", output_path);
}

fn compute_work_group_count(
    (width, height): (u32, u32),
    (workgroup_width, workgroup_height): (u32, u32),
) -> (u32, u32) {
    let x = (width + workgroup_width - 1) / workgroup_width;
    let y = (height + workgroup_height - 1) / workgroup_height;

    (x, y)
}

fn padded_bytes_per_row(width: u32) -> usize {
    let bytes_per_row = width as usize * 4;
    let padding = (256 - bytes_per_row % 256) % 256;
    bytes_per_row + padding
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
    fn transform(&self, _pixel: Rgb<f32>) -> Rgb<f32> {
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