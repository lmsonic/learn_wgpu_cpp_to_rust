
@group(0) @binding(0) var<storage,read> input_buffer:array<f32,64>;
@group(0) @binding(1) var<storage,read_write> output_buffer:array<f32,64>;

@compute @workgroup_size(32)
fn compute(@builtin(global_invocation_id) id: vec3<u32>) {
    // Apply the function f to the buffer element at index id.x:
    output_buffer[id.x] = f(input_buffer[id.x]);
}

fn f(x:f32) -> f32{
    return 2.0 * x + 1.0;
}