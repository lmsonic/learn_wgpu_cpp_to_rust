struct VertexInput {
    @location(0) position: vec3f,
    @location(1) normal: vec3f,
    @location(2) color: vec3f,
};

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) color: vec3f,
    @location(1) normal: vec3f,
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
    out.normal = (uniforms.model * vec4f(in.normal,0.0)).xyz;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let normal = normalize(in.normal);
    let light_direction1 = vec3f(0.5,-0.9,0.1);
    let shading1 = max(0.0,dot(normal,light_direction1));

    let light_direction2 = vec3f(0.2, 0.4, 0.3);
    let shading2 = max(0.0, dot(normal,light_direction2));

    let light_color1 = vec3f(1.0, 0.9, 0.6);
    let light_color2 = vec3f(0.6, 0.9, 1.0);
    let shading = light_color1 * shading1 + light_color2 * shading2;
    let color = in.color * shading;

    
    let linear_color = pow(color, vec3f(2.2)); // Gamma correction
    return vec4f(linear_color, uniforms.color.a);
}