// Vertex Shader
struct VertexInput {
    @location(0) a_Position: vec2<f32>,
    @location(1) a_TexCoord: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) Position: vec4<f32>,
    @location(0) v_TexCoord: vec2<f32>,
};

@vertex
fn main_vertex(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.Position = vec4<f32>(input.a_Position, 0.0, 1.0);
    output.v_TexCoord = input.a_TexCoord;
    return output;
}

// Fragment Shader
@group(0) @binding(0) var u_Texture: texture_2d<f32>;
@group(0) @binding(1) var u_Sampler: sampler;
@group(0) @binding(2) var<uniform> u_TextColor: vec4<f32>;

struct FragmentInput {
    @location(0) v_TexCoord: vec2<f32>,
};

@fragment
fn main_fragment(input: FragmentInput) -> @location(0) vec4<f32> {
    let texColor: vec4<f32> = textureSample(u_Texture, u_Sampler, input.v_TexCoord);
    return texColor;
}
