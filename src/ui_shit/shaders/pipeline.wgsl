struct SolidOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) frag_color: vec4<f32>,
};

struct TexturedOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) frag_uv: vec2<f32>,
    @location(1) frag_color: vec4<f32>,
};

@group(0) @binding(0)
var tex_sampler: sampler;
@group(0) @binding(1)
var tex: texture_2d<f32>;

@vertex
fn vs_solid(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> SolidOutput {
    var output: SolidOutput;
    output.clip_position = vec4<f32>(position, 0.0, 1.0);
    output.frag_color = color;
    return output;
}

@fragment
fn fs_solid(@location(0) color: vec4<f32>) -> @location(0) vec4<f32> {
    return color;
}

@vertex
fn vs_textured(
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
) -> TexturedOutput {
    var output: TexturedOutput;
    output.clip_position = vec4<f32>(position, 0.0, 1.0);
    output.frag_uv = uv;
    output.frag_color = color;
    return output;
}

@fragment
fn fs_textured(
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> @location(0) vec4<f32> {
    let glyph = textureSample(tex, tex_sampler, uv).r;
    return vec4<f32>(color.rgb, color.a * glyph);
}

struct GradOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) pos_t: vec4<f32>,
    @location(1) from_rgba: vec4<f32>,
    @location(2) to_rgba: vec4<f32>,
};

@vertex
fn vs_gradient(
    @location(0) pos_t: vec4<f32>,
    @location(1) from_rgba: vec4<f32>,
    @location(2) to_rgba: vec4<f32>,
) -> GradOutput {
    var output: GradOutput;
    output.clip_position = vec4<f32>(pos_t.xy, 0.0, 1.0);
    output.pos_t = pos_t;
    output.from_rgba = from_rgba;
    output.to_rgba = to_rgba;
    return output;
}

@fragment
fn fs_gradient(vin: GradOutput) -> @location(0) vec4<f32> {
    return mix(vin.from_rgba, vin.to_rgba, vin.pos_t.z);
}
