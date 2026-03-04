//! WGSL shaders for terrain and entity rendering.

/// Terrain shader - height-based coloring with basic lighting.
pub const TERRAIN_SHADER: &str = r#"
// Uniform buffer for camera matrices
struct Uniforms {
    view_proj: mat4x4<f32>,
}
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.world_pos = in.position;
    out.normal = in.normal;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Height-based color gradient
    let height = in.world_pos.y;
    let normalized_height = clamp(height / 48.0, 0.0, 1.0);
    
    // Color gradient: dark grey (low) -> brown (mid) -> light grey (high)
    let low_color = vec3<f32>(0.2, 0.18, 0.15);   // Dark ash
    let mid_color = vec3<f32>(0.35, 0.28, 0.2);   // Brown/rust
    let high_color = vec3<f32>(0.5, 0.48, 0.45);  // Light stone
    
    var base_color: vec3<f32>;
    if normalized_height < 0.5 {
        let t = normalized_height * 2.0;
        base_color = mix(low_color, mid_color, t);
    } else {
        let t = (normalized_height - 0.5) * 2.0;
        base_color = mix(mid_color, high_color, t);
    }
    
    // Simple directional lighting
    let light_dir = normalize(vec3<f32>(0.5, 0.8, 0.3));
    let normal = normalize(in.normal);
    let ndotl = max(dot(normal, light_dir), 0.0);
    
    // Ambient + diffuse
    let ambient = 0.3;
    let diffuse = 0.7 * ndotl;
    let lit_color = base_color * (ambient + diffuse);
    
    return vec4<f32>(lit_color, 1.0);
}
"#;

/// Entity shader - solid color cubes with lighting.
pub const ENTITY_SHADER: &str = r#"
// Uniform buffer for camera matrices and per-entity data
struct Uniforms {
    view_proj: mat4x4<f32>,
}
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct EntityUniforms {
    model: mat4x4<f32>,
    color: vec4<f32>,
}
@group(1) @binding(0)
var<uniform> entity: EntityUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = entity.model * vec4<f32>(in.position, 1.0);
    out.clip_position = uniforms.view_proj * world_pos;
    // Transform normal (assuming uniform scale, just use model mat)
    out.normal = (entity.model * vec4<f32>(in.normal, 0.0)).xyz;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple directional lighting
    let light_dir = normalize(vec3<f32>(0.5, 0.8, 0.3));
    let normal = normalize(in.normal);
    let ndotl = max(dot(normal, light_dir), 0.0);
    
    // Ambient + diffuse
    let ambient = 0.4;
    let diffuse = 0.6 * ndotl;
    let lit_color = entity.color.rgb * (ambient + diffuse);
    
    return vec4<f32>(lit_color, entity.color.a);
}
"#;
