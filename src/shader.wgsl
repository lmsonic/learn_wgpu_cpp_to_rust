struct VertexInput {
    @location(0) position: vec3f,
    @location(1) normal: vec3f,
    @location(2) color: vec3f,
    @location(3) uv: vec2f,
};

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) color: vec3f,
    @location(1) normal: vec3f,
    @location(2) uv: vec2f,
    @location(3) view_direction: vec3f,
};

struct Uniforms {
    model: mat4x4f,
    view: mat4x4f,
    projection: mat4x4f,
    color: vec4f,
    camera_world_position: vec3f,
    time: f32,
};

struct LightUniforms{
    directions:array<vec4f,2>,
    colors:array<vec4f,2>,
    hardness:f32,
    diffuse:f32,
    specular:f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<uniform> light_uniforms: LightUniforms;
@group(0) @binding(2) var texture: texture_2d<f32>;
@group(0) @binding(3) var texture_sampler: sampler;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_position = uniforms.model * vec4f(in.position, 1.0);
    out.position = uniforms.projection * uniforms.view * world_position;
    out.color = in.color; 
    out.normal = (uniforms.model * vec4f(in.normal,0.0)).xyz;
    out.uv = in.uv;
    out.view_direction = uniforms.camera_world_position - world_position.xyz;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let N = normalize(in.normal);
    let V = normalize(in.view_direction);
    var shading = vec3f(0.0);

    let base_color = textureSample(texture,texture_sampler,in.uv).rgb;


    for (var i:i32 = 0 ; i<2 ; i++){
        let L = normalize(light_uniforms.directions[i].xyz);
        let R = reflect(-L, N); // equivalent to 2.0 * dot(N, L) * N - L

        let color = light_uniforms.colors[i].rgb;
        let diffuse = max(0.0,dot(L,N)) * color;

        // We clamp the dot product to 0 when it is negative
        let RoV = max(0.0, dot(R, V));
        let hardness = 32.0;
        let specular = pow(RoV, light_uniforms.hardness);

        shading += diffuse * light_uniforms.diffuse * base_color + specular * light_uniforms.specular;
    }

    
    let color = shading;
    

    let linear_color = pow(color, vec3f(2.2)); // Gamma correction
    return vec4f(linear_color, uniforms.color.a);
}