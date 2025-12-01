struct Settings {
    exposure: f32,
    contrast: f32,
    saturation: f32
}

@group(0) @binding(0)
var input_texture: texture_2d<f32>;

@group(0) @binding(1)
var output_texture: texture_storage_2d<rgba8unorm, write>;

@group(0) @binding(2)
var<uniform> settings: Settings;

@compute @workgroup_size(16, 16)
fn shader_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(input_texture);
    let coords = vec2<i32>(global_id.xy);

    // Bounds check
    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
        return;
    }

    let color = textureLoad(input_texture, coords, 0);

    // 1. Exposure
    let exposed = color.rgb * pow(2.0, settings.exposure);

    // 2. Contrast
    let contrasted = (exposed - 0.5) * (1.0 + settings.contrast) + 0.5;

    // 3. Saturation
    let L = 0.299 * contrasted.r + 0.587 * contrasted.g + 0.114 * contrasted.b;
    let L_vec = vec3<f32>(L, L, L);
    let saturated_rgb = L_vec + (contrasted - L_vec) * (1.0 + settings.saturation);

    // 4. Clamp to [0, 1]
    let final_rgb = clamp(saturated_rgb, vec3<f32>(0.0), vec3<f32>(1.0));

    textureStore(output_texture, coords, vec4<f32>(final_rgb, color.a));
}
