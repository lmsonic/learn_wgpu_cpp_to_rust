struct VertexInput {
    @location(0) position: vec3f,
    @location(1) color: vec3f,
};

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) color: vec3f,
};

struct Uniforms {
    model: mat4x4f,
    view: mat4x4f,
    projection: mat4x4f,
    color: vec4f,
    time: f32,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;


@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = uniforms.projection * uniforms.view * uniforms.model * vec4f(in.position, 1.0);
    out.color = in.color; 
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let color = in.color * uniforms.color.rgb;
    let linear_color = pow(color, vec3f(2.2)); // Gamma correction
    return vec4f(linear_color, uniforms.color.a);
}