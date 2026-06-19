struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) frag_uv: vec2<f32>,
};

@vertex
fn vs_solid(@location(0) position: vec2<f32>) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = vec4<f32>(position, 0.0, 1.0);
    output.frag_uv = vec2<f32>(0.0);
    return output;
}

@fragment
fn fs_solid() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}

@vertex
fn vs_textured(
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = vec4<f32>(position, 0.0, 1.0);
    output.frag_uv = uv;
    return output;
}

@group(0) @binding(0)
var tex_sampler: sampler;
@group(0) @binding(1)
var tex: texture_2d<f32>;

@fragment
fn fs_textured(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(tex, tex_sampler, uv);
}
