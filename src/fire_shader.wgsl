// ===== FIRE PARTICLE SHADER =====
// This shader renders procedural fire using billboard particles

// Camera uniform (reuse from your main shader)
struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

// Time uniform for animating noise
struct TimeUniform {
    time: f32,
};
@group(1) @binding(0)
var<uniform> u_time: TimeUniform;

// ===== NOISE FUNCTIONS =====
// Simple 3D noise function (pseudo-random)
fn hash(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// 3D Perlin-style noise
fn noise3d(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);

    // Smooth interpolation
    let u = f * f * (3.0 - 2.0 * f);

    // Sample 8 corners of cube
    return mix(
        mix(
            mix(hash(i + vec3<f32>(0.0, 0.0, 0.0)), hash(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(hash(i + vec3<f32>(0.0, 1.0, 0.0)), hash(i + vec3<f32>(1.0, 1.0, 0.0)), u.x),
            u.y
        ),
        mix(
            mix(hash(i + vec3<f32>(0.0, 0.0, 1.0)), hash(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(hash(i + vec3<f32>(0.0, 1.0, 1.0)), hash(i + vec3<f32>(1.0, 1.0, 1.0)), u.x),
            u.y
        ),
        u.z
    );
}

// Fractal Brownian Motion - layers of noise at different scales
fn fbm(p: vec3<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var p_var = p;

    // 3 octaves (layers) of noise
    for (var i = 0; i < 3; i++) {
        value += amplitude * noise3d(p_var * frequency);
        frequency *= 2.0;  // Each layer is twice as detailed
        amplitude *= 0.5;  // But half as strong
        p_var = p_var * 2.0 + 0.5;
    }

    return value;
}

// ===== VERTEX SHADER =====
// Input: Per-particle data
struct VertexInput {
    @location(0) position: vec3<f32>,    // Particle center in world space
    @location(1) size: f32,              // How big the particle quad is
    @location(2) life: f32,              // 0.0 = just born, 1.0 = dead
    @location(3) corner: vec2<f32>,      // Which corner of quad: (-1,-1), (1,-1), etc.
}

// Output: Data passed from vertex � fragment shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,  // Screen position (required!)
    @location(0) life: f32,                        // Pass life to fragment shader
    @location(1) uv: vec2<f32>,                    // UV coords for the particle quad
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // ===== BROWNIAN MOTION DISPLACEMENT =====
    // Add turbulence to particle position based on noise
    let noise_coord = in.position * 2.0 + vec3<f32>(u_time.time * 0.5, u_time.time, u_time.time * 0.3);

    // Sample noise in 3D space
    let noise_x = fbm(noise_coord) * 2.0 - 1.0;                    // -1 to 1
    let noise_z = fbm(noise_coord + vec3<f32>(100.0, 0.0, 0.0)) * 2.0 - 1.0;

    // More turbulence as particle ages (fire becomes chaotic)
    let turbulence_strength = in.life * 0.3;

    // Apply displacement
    var displaced_position = in.position;
    displaced_position.x += noise_x * turbulence_strength;
    displaced_position.z += noise_z * turbulence_strength;

    // Billboard technique: Make particle face camera
    // We need camera right and up vectors - for now, use world axes
    // (Later we'll extract from camera matrix for true billboarding)
    let camera_right = vec3<f32>(1.0, 0.0, 0.0);
    let camera_up = vec3<f32>(0.0, 1.0, 0.0);

    // Expand point to quad by offsetting in camera space
    let offset = camera_right * in.corner.x * in.size +
                 camera_up * in.corner.y * in.size;

    let world_position = vec4<f32>(displaced_position + offset, 1.0);

    // Transform to screen space
    out.clip_position = camera.view_proj * world_position;

    // Pass data to fragment shader
    out.life = in.life;
    out.uv = in.corner * 0.5 + 0.5;  // Convert -1..1 to 0..1 for UVs

    return out;
}

// ===== FRAGMENT SHADER =====
// This runs for every pixel in each particle quad
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate distance from center of particle (for circular shape)
    let center_dist = length(in.uv - vec2<f32>(0.5, 0.5)) * 2.0;

    // Discard pixels outside circle (makes square quad look round)
    if (center_dist > 1.0) {
        discard;
    }

    // Fire color gradient based on particle life
    // Young (life=0.0): Hot white/yellow
    // Middle (life=0.5): Orange/red
    // Old (life=1.0): Dark red, fades out

    let young_color = vec3<f32>(1.0, 0.9, 0.5);   // Hot yellow-white
    let mid_color = vec3<f32>(1.0, 0.3, 0.0);     // Orange
    let old_color = vec3<f32>(0.3, 0.0, 0.0);     // Dark red

    // Blend between colors based on life
    var color: vec3<f32>;
    if (in.life < 0.5) {
        // First half of life: yellow � orange
        color = mix(young_color, mid_color, in.life * 2.0);
    } else {
        // Second half: orange � dark red
        color = mix(mid_color, old_color, (in.life - 0.5) * 2.0);
    }

    // Fade out at edges (soft particle effect)
    let edge_fade = 1.0 - smoothstep(0.5, 1.0, center_dist);

    // Alpha: Fade out as particle dies AND at edges
    let alpha = (1.0 - in.life) * edge_fade;

    return vec4<f32>(color, alpha);
}
