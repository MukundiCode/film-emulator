use std::ffi::CString;
use std::slice;
use image::ImageReader;
use crate::{libraw_close, libraw_dcraw_clear_mem, libraw_dcraw_make_mem_image, libraw_dcraw_process, libraw_init, libraw_open_file, libraw_unpack};

pub fn load_image_to_linear_rgb(path: &String) -> (u32, u32, Vec<[f32; 3]>) {
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