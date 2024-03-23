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

const pi = 3.14159265359; 

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    let ratio = 640.0 / 480.0; // The width and height of the target surface
        // Rotate the model in the XY plane
    let angle1 = uniforms.time;
    let c1 = cos(angle1);
    let s1 = sin(angle1);
    let R1 = transpose(mat4x4f(
         c1,  s1, 0.0,0.0,
        -s1,  c1, 0.0,0.0,
        0.0, 0.0, 1.0,0.0,
        0.0, 0.0, 0.0,1.0,
    ));

    // Tilt the view point in the YZ plane
    // by three 8th of turn (1 turn = 2 pi)
    let angle2 = 3.0 * pi / 4.0;
    let c2 = cos(angle2);
    let s2 = sin(angle2);
    let R2 = transpose(mat4x4f(
        1.0, 0.0, 0.0,0.0,
        0.0,  c2,  s2,0.0,
        0.0, -s2,  c2,0.0,
        0.0, 0.0, 0.0,1.0,
    ));
        // Scale the object
    let S = transpose(mat4x4f(
        0.3,  0.0, 0.0, 0.0,
        0.0,  0.3, 0.0, 0.0,
        0.0,  0.0, 0.3, 0.0,
        0.0,  0.0, 0.0, 1.0,
    ));

    // Translate the object
    let T = transpose(mat4x4f(
        1.0,  0.0, 0.0, 0.5,
        0.0,  1.0, 0.0, 0.0,
        0.0,  0.0, 1.0, 0.0,
        0.0,  0.0, 0.0, 1.0,
    ));
    var homogeneous_position = vec4f(in.position,1.0);

    var position = (R2 * R1 * S * T * homogeneous_position).xyz;
    out.position = vec4f(position.x, position.y * ratio, position.z * 0.5 + 0.5, 1.0);
    out.color = in.color; 
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let color = in.color * uniforms.color.rgb;
    let linear_color = pow(color, vec3f(2.2)); // Gamma correction
    return vec4f(linear_color, uniforms.color.a);
}