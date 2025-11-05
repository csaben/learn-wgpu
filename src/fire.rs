use std::time::Instant;
use wgpu::util::DeviceExt;

// ===== TIME UNIFORM =====
// This gets sent to the shader to animate noise
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TimeUniform {
    pub time: f32,
    _padding: [f32; 3], // Uniforms need to be 16-byte aligned
}

impl TimeUniform {
    pub fn new() -> Self {
        Self {
            time: 0.0,
            _padding: [0.0; 3],
        }
    }

    pub fn update(&mut self, elapsed: f32) {
        self.time = elapsed;
    }
}

// ===== FIRE PARTICLE =====
// Represents a single particle in the fire effect
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FireParticleVertex {
    pub position: [f32; 3], // World position
    pub size: f32,          // Size of the billboard quad
    pub life: f32,          // 0.0 = newborn, 1.0 = dead
    pub corner: [f32; 2],   // Which corner of the quad (-1/-1, 1/-1, etc)
}

impl FireParticleVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<FireParticleVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // size
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32,
                },
                // life
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32,
                },
                // corner
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

// ===== FIRE PARTICLE SYSTEM =====
pub struct FireSystem {
    particles: Vec<Particle>,
    pub origin: [f32; 3], // Public so we can update it dynamically
    cone_angle: f32,
    spawn_rate: f32,
    accumulator: f32,
    start_time: Instant,

    // GPU resources
    pub vertex_buffer: wgpu::Buffer,
    pub time_buffer: wgpu::Buffer,
    pub time_bind_group: wgpu::BindGroup,
    pub render_pipeline: wgpu::RenderPipeline,

    // Cached data
    vertices: Vec<FireParticleVertex>,
}

// Internal particle representation (CPU side)
struct Particle {
    position: [f32; 3],
    velocity: [f32; 3],
    life: f32,
    size: f32,
}

impl FireSystem {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        origin: [f32; 3],
    ) -> Self {
        // ===== CREATE TIME UNIFORM =====
        let time_uniform = TimeUniform::new();
        let time_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Fire Time Buffer"),
            contents: bytemuck::cast_slice(&[time_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Time bind group layout
        let time_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("fire_time_bind_group_layout"),
            });

        let time_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &time_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: time_buffer.as_entire_binding(),
            }],
            label: Some("fire_time_bind_group"),
        });

        // ===== LOAD SHADER =====
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Fire Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("fire_shader.wgsl").into()),
        });

        // ===== CREATE RENDER PIPELINE =====
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Fire Pipeline Layout"),
                bind_group_layouts: &[camera_bind_group_layout, &time_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Fire Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[FireParticleVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    // IMPORTANT: Additive blending for fire!
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Don't cull - particles can be viewed from any angle
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: false, // Fire doesn't write depth
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // Create initial vertex buffer (empty)
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Fire Vertex Buffer"),
            size: (std::mem::size_of::<FireParticleVertex>() * 1024 * 4) as u64, // Max 1024 particles
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            particles: Vec::new(),
            origin,
            cone_angle: 0.3,  // ~17 degrees
            spawn_rate: 50.0, // particles per second
            accumulator: 0.0,
            start_time: Instant::now(),
            vertex_buffer,
            time_buffer,
            time_bind_group,
            render_pipeline,
            vertices: Vec::new(),
        }
    }

    // Update particles and spawn new ones
    pub fn update(&mut self, dt: f32) {
        // Update existing particles
        self.particles.retain_mut(|p| {
            p.position[0] += p.velocity[0] * dt;
            p.position[1] += p.velocity[1] * dt;
            p.position[2] += p.velocity[2] * dt;

            p.life += dt * 0.5; // Age rate
            p.size += dt * 0.3; // Grow over time

            p.life < 1.0 // Remove dead particles
        });

        // Spawn new particles
        self.accumulator += dt;
        let spawn_interval = 1.0 / self.spawn_rate;

        while self.accumulator >= spawn_interval {
            self.spawn_particle();
            self.accumulator -= spawn_interval;
        }
    }

    fn spawn_particle(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Random direction within cone
        let angle: f32 = rng.random::<f32>() * self.cone_angle;
        let rotation: f32 = rng.random::<f32>() * std::f32::consts::PI * 2.0;

        // Convert to 3D direction (cone points forward +Z, slightly up)
        let dir_x = angle.sin() * rotation.cos();
        let dir_y = 0.3 + angle.sin() * 0.2; // Slight upward component
        let dir_z = angle.cos(); // Primary direction is forward (+Z)

        let size_rand: f32 = rng.random();
        let particle = Particle {
            position: self.origin,
            velocity: [dir_x * 0.5, dir_y * 0.8, dir_z * 2.0], // Mostly forward (+Z)
            life: 0.0,
            size: 0.1 + size_rand * 0.1,
        };

        self.particles.push(particle);
    }

    // Convert particles to GPU vertex format
    pub fn prepare_vertices(&mut self) {
        self.vertices.clear();

        // Each particle becomes 6 vertices (2 triangles = 1 quad)
        let corners = [
            [-1.0, -1.0], // Bottom-left
            [1.0, -1.0],  // Bottom-right
            [1.0, 1.0],   // Top-right
            [-1.0, -1.0], // Bottom-left (again for 2nd triangle)
            [1.0, 1.0],   // Top-right (again)
            [-1.0, 1.0],  // Top-left
        ];

        for particle in &self.particles {
            for corner in corners.iter() {
                self.vertices.push(FireParticleVertex {
                    position: particle.position,
                    size: particle.size,
                    life: particle.life,
                    corner: *corner,
                });
            }
        }
    }

    pub fn render<'a>(
        &'a mut self,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        // Update time uniform
        let elapsed = self.start_time.elapsed().as_secs_f32();
        let time_uniform = TimeUniform {
            time: elapsed,
            _padding: [0.0; 3],
        };
        queue.write_buffer(&self.time_buffer, 0, bytemuck::cast_slice(&[time_uniform]));

        // Prepare vertices
        self.prepare_vertices();

        if self.vertices.is_empty() {
            return; // Nothing to render
        }

        // Upload vertices to GPU
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));

        // Draw!
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_bind_group(1, &self.time_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.vertices.len() as u32, 0..1);
    }
}

// Add missing texture import
use crate::texture;
