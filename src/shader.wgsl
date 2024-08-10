// Vertex shader

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    out.clip_position = vec4<f32>(model.position, 1.0);
    return out;
}

// Fragment shader

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the texture to get the red channel value
    let color = textureSample(t_diffuse, s_diffuse, in.tex_coords);

    // Treat the red channel as the alpha value
    let alpha = color.r;

    // Discard fragments with alpha close to 0 to avoid rendering them
    // if (alpha < 0.01) {
    //     discard;
    // }

    // Output the color with the correct alpha channel
    return vec4<f32>(1.0, 1.0, 1.0, alpha); // Red color with dynamic alpha
}
