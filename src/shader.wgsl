struct VertexInput {
    @location(0) position: vec3f,
    @location(1) tangent: vec3f,
    @location(2) bitangent: vec3f,
    @location(3) normal: vec3f,
    @location(4) color: vec3f,
    @location(5) uv: vec2f,
};

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) color: vec3f,
    @location(1) tangent: vec3f,
    @location(2) bitangent: vec3f,
    @location(3) normal: vec3f,
    @location(4) uv: vec2f,
    @location(5) view_direction: vec3f,
};

struct Uniforms {
    model: mat4x4f,
    view: mat4x4f,
    projection: mat4x4f,
    color: vec4f,
    camera_world_position: vec3f,
    time: f32,
    normal_map_strength:f32
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
@group(0) @binding(4) var normal_texture: texture_2d<f32>;
@group(0) @binding(5) var normal_sampler: sampler;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_position = uniforms.model * vec4f(in.position, 1.0);
    out.position = uniforms.projection * uniforms.view * world_position;
    out.color = in.color; 
    out.tangent = (uniforms.model * vec4f(in.tangent,0.0)).xyz;
    out.bitangent = (uniforms.model * vec4f(in.bitangent,0.0)).xyz;
    out.normal = (uniforms.model * vec4f(in.normal,0.0)).xyz;
    out.uv = in.uv;
    out.view_direction = uniforms.camera_world_position - world_position.xyz;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let normal_map = textureSample(normal_texture,normal_sampler,in.uv).rgb;
    let tangent_normal = normal_map * 2.0 - 1.0;
    let tangent_to_world = mat3x3f(
        normalize(in.tangent),
        normalize(in.bitangent),
        normalize(in.normal),
    );
    let world_normal = tangent_to_world * tangent_normal;
    let N = mix(in.normal,world_normal,uniforms.normal_map_strength);
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