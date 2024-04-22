
@group(0) @binding(0) var previous_mip_level: texture_2d<f32>;
@group(0) @binding(1) var next_mip_level: texture_storage_2d<rgba8unorm,write>;

@compute @workgroup_size(8, 8)
fn compute_mip_map(@builtin(global_invocation_id) id: vec3<u32>) {
    let offset = vec2<u32>(0, 1);
    let color = (
        textureLoad(previous_mip_level, 2 * id.xy + offset.xx, 0) +
        textureLoad(previous_mip_level, 2 * id.xy + offset.xy, 0) +
        textureLoad(previous_mip_level, 2 * id.xy + offset.yx, 0) +
        textureLoad(previous_mip_level, 2 * id.xy + offset.yy, 0)
    ) * 0.25;
    textureStore(next_mip_level, id.xy, color);
}