@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba8unorm,write>;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;
struct Uniforms {
    kernel: mat3x3<f32>,
    test: f32,
}

@compute @workgroup_size(8, 8)
fn compute_sobel_x(@builtin(global_invocation_id) id: vec3<u32>) {
    let color = abs(
          1 * textureLoad(input_texture, vec2<u32>(id.x - 1, id.y - 1), 0).rgb
        + 2 * textureLoad(input_texture, vec2<u32>(id.x - 1, id.y + 0), 0).rgb
        + 1 * textureLoad(input_texture, vec2<u32>(id.x - 1, id.y + 1), 0).rgb
        - 1 * textureLoad(input_texture, vec2<u32>(id.x + 1, id.y - 1), 0).rgb
        - 2 * textureLoad(input_texture, vec2<u32>(id.x + 1, id.y + 0), 0).rgb
        - 1 * textureLoad(input_texture, vec2<u32>(id.x + 1, id.y + 1), 0).rgb
    );
    textureStore(output_texture, id.xy, vec4<f32>(color, 1.0));
}


@compute @workgroup_size(8, 8)
fn compute_filter(@builtin(global_invocation_id) id: vec3<u32>) {
    let color = 
        uniforms.kernel[0][0] * textureLoad(input_texture, vec2<u32>(id.x - 1u, id.y - 1u), 0).rgb+
        uniforms.kernel[1][0] * textureLoad(input_texture, vec2<u32>(id.x - 1u, id.y + 0u), 0).rgb+
        uniforms.kernel[2][0] * textureLoad(input_texture, vec2<u32>(id.x - 1u, id.y + 1u), 0).rgb+
        uniforms.kernel[0][1] * textureLoad(input_texture, vec2<u32>(id.x + 0u, id.y - 1u), 0).rgb+
        uniforms.kernel[1][1] * textureLoad(input_texture, vec2<u32>(id.x + 0u, id.y + 0u), 0).rgb+
        uniforms.kernel[2][1] * textureLoad(input_texture, vec2<u32>(id.x + 0u, id.y + 1u), 0).rgb+
        uniforms.kernel[0][2] * textureLoad(input_texture, vec2<u32>(id.x + 1u, id.y - 1u), 0).rgb+
        uniforms.kernel[1][2] * textureLoad(input_texture, vec2<u32>(id.x + 1u, id.y + 0u), 0).rgb+
        uniforms.kernel[2][2] * textureLoad(input_texture, vec2<u32>(id.x + 1u, id.y + 1u), 0).rgb
    ;
        textureStore(output_texture, id.xy, vec4<f32>(color, 1.0));
}