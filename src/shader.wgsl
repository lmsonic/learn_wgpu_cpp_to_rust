struct VertexInput {
    @location(0) position: vec3f,
    @location(1) color: vec3f,
};

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) color: vec3f,
};

struct Uniforms {
    color: vec4f,
    time: f32,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let ratio = 640.0 / 480.0; // The width and height of the target surface
    let angle = uniforms.time;
    let alpha = sin(angle);
    let beta = cos(angle);
    var position=vec3f(    
        in.position.x,
        alpha * in.position.y + beta * in.position.z,
        alpha * in.position.z - beta * in.position.y
    );
    out.position = vec4f(position.x , position.y  * ratio, position.z * 0.5 + 0.5, 1.0);
    out.color = in.color; 
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let color = in.color * uniforms.color.rgb;
    let linear_color = pow(color, vec3f(2.2)); // Gamma correction
    return vec4f(linear_color, uniforms.color.a);
}