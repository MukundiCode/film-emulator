struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct ViewParams {
    viewport_size: vec2<f32>,
    image_size: vec2<f32>,
};

@group(0) @binding(1)
var<uniform> view: ViewParams;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VSOut {
    let quad = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0),
        vec2( 1.0, -1.0),
        vec2( 1.0,  1.0),
        vec2(-1.0, -1.0),
        vec2( 1.0,  1.0),
        vec2(-1.0,  1.0),
    );

    let uvs = array<vec2<f32>, 6>(
        vec2(0.0, 1.0),
        vec2(1.0, 1.0),
        vec2(1.0, 0.0),
        vec2(0.0, 1.0),
        vec2(1.0, 0.0),
        vec2(0.0, 0.0),
    );

    let image_aspect = view.image_size.x / view.image_size.y;
    let view_aspect  = view.viewport_size.x / view.viewport_size.y;

    var scale = vec2(1.0, 1.0);

    if (image_aspect > view_aspect) {
        scale.y = view_aspect / image_aspect;
    } else {
        scale.x = image_aspect / view_aspect;
    }

    var out: VSOut;
    out.pos = vec4(quad[i] * scale, 0.0, 1.0);
    out.uv  = uvs[i];

    return out;
}

@group(0) @binding(0)
var image_tex: texture_2d<f32>;

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
    // The UVs are already correct from the vertex shader
    // Just sample the texture directly
    let dims = vec2<i32>(textureDimensions(image_tex));
    return textureLoad(image_tex, vec2<i32>(in.uv * vec2<f32>(dims)), 0);
}
