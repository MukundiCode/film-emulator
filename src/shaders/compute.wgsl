//
//struct ImageControls {
//    exposure: f32,
//    contrast: f32,
//    saturation: f32,
//    brightness: f32,
//    highlights: f32,
//    shadows: f32,
//}
//
//@group(0) @binding(0)
//var input_texture: texture_2d<f32>;
//
//@group(0) @binding(1)
//var output_texture: texture_storage_2d<rgba8unorm, write>;
//
//@group(0) @binding(2)
//var<uniform> imageControls: ImageControls;
//
//@compute @workgroup_size(16, 16)
//fn shader_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
//    let dims = textureDimensions(input_texture);
//    let coords = vec2<i32>(global_id.xy);
//
//    // Bounds check
//    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
//        return;
//    }
//
//    let color = textureLoad(input_texture, coords, 0);
//
//    // 1. Exposure
//    let exposed = color.rgb * pow(2.0, (imageControls.exposure * 0.2));
//
//    // 2. Contrast
//    let contrasted = (exposed - 0.5) * (1.0 + imageControls.contrast) + 0.5;
//
//    // 3. Saturation
//    let L = 0.299 * contrasted.r + 0.587 * contrasted.g + 0.114 * contrasted.b;
//    let L_vec = vec3<f32>(L, L, L);
//    let saturated_rgb = L_vec + (contrasted - L_vec) * (1.0 + imageControls.saturation);
//
//    // 4. Clamp to [0, 1]
//    let final_rgb = clamp(saturated_rgb, vec3<f32>(0.0), vec3<f32>(1.0));
//
//    textureStore(output_texture, coords, vec4<f32>(final_rgb, color.a));
//}
//
//

struct ImageControls {
    exposure: f32,   // Stops (e.g. -2.0 to +2.0)
    contrast: f32,   // Factor (1.0 = Neutral, 1.2 = High Contrast)
    saturation: f32, // Factor (1.0 = Neutral, 0.0 = B&W)
    brightness: f32,
    highlights: f32,
    shadows: f32,
}

@group(0) @binding(0)
var input_texture: texture_2d<f32>;

@group(0) @binding(1)
var output_texture: texture_storage_2d<rgba8unorm, write>;

@group(0) @binding(2)
var<uniform> imageControls: ImageControls;

@compute @workgroup_size(16, 16)
fn shader_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(input_texture);
    let coords = vec2<i32>(global_id.xy);

    // Bounds check
    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
        return;
    }

    // 1. Load Input (Assuming Linear Float from previous steps)
    let raw_color = textureLoad(input_texture, coords, 0);
    var color = raw_color.rgb;

    // -----------------------------------------------------------------
    // STAGE 1: LINEAR OPERATIONS (Physics based)
    // -----------------------------------------------------------------

    // Exposure
    // We strictly use base-2 power for accurate camera stops
    color = color * pow(2.0, imageControls.exposure);

    // 2. Brightness (Additive - Digital Offset)
    // Typical Range: -0.5 to +0.5
    color = color + imageControls.brightness;

    // -----------------------------------------------------------------
    // STAGE 2: GAMMA CONVERSION
    // -----------------------------------------------------------------
    // We must move to perceptual space for "Digital Contrast" to feel right.
    // If we contrast-pivot around 0.5 in Linear (where grey is 0.18), we crush shadows.
    // In Gamma 2.2, grey is approx 0.5, so the pivot works.
    let gamma = 2.2;
    color = pow(color, vec3<f32>(1.0 / gamma));

    // -----------------------------------------------------------------
    // STAGE 3: PERCEPTUAL OPERATIONS (Digital style)
    // -----------------------------------------------------------------

    // Contrast
    // Pivot around 0.5 (Mid-gray in Gamma space)
    // Formula: (Color - 0.5) * Contrast + 0.5
    color = (color - 0.5) * imageControls.contrast + 0.5;

    // Saturation (Luma-preserving-ish)
    // We use the standard Rec.709 Luma coefficients
    let luma = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    let luma_vec = vec3<f32>(luma);

    let shadow_mask = (1.0 - luma) * (1.0 - luma);
    let highlight_mask = luma * luma;

    // 3. Apply Shadows (Additive Lift)
    // imageControls.shadows range: -0.5 (Crush) to +0.5 (Lift)
    // We add light specifically to areas where shadow_mask is high
    color = color + (vec3<f32>(imageControls.shadows) * shadow_mask);

    // 4. Apply Highlights (Multiplicative Gain)
    // imageControls.highlights range: -1.0 (Recover) to +1.0 (Boost)
    // We scale the brightness specifically where highlight_mask is high
    color = color + (color * imageControls.highlights * highlight_mask);

    // Interpolate between Grayscale (luma) and Color
    color = mix(luma_vec, color, imageControls.saturation);

    // -----------------------------------------------------------------
    // STAGE 4: OUTPUT
    // -----------------------------------------------------------------

    // Clamp to valid sRGB range to prevent weird artifacts on display
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    // Note: We are writing Gamma-Corrected values to the storage texture.
    // This assumes your swapchain/display expects sRGB pixel data.
    textureStore(output_texture, coords, vec4<f32>(color, raw_color.a));
}