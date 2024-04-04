#![allow(clippy::module_name_repetitions)]
use std::{
    mem,
    sync::Arc,
    time::{self, Instant},
};

use glam::{Mat4, Vec3, Vec4};
use pollster::FutureExt;
use tracing::{error, info};
use wgpu::util::DeviceExt;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

use crate::resources::{load_geometry, load_texture, VertexAttribute};
pub struct ApplicationState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    vertices: Vec<VertexAttribute>,
    vertex_buffer: wgpu::Buffer,
    uniforms: Uniforms,
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    start_time: Instant,
}

impl ApplicationState {
    pub fn new(window: &Arc<Window>) -> Self {
        let size = window.inner_size();
        let (surface, device, queue, config) = init_wgpu(window);
        let (depth_texture, depth_view) = init_depth(&device, size.width, size.height);
        let (texture, texture_view, sampler) = init_texture(&device, &queue);
        let (vertices, vertex_buffer) = init_geometry(&device);
        let (uniforms, uniform_buffer) =
            init_uniforms(&device, size.width as f32 / size.height as f32);
        let (bind_group_layout, bind_group) =
            init_bind_groups(&device, &uniform_buffer, &texture_view, &sampler);
        let render_pipeline = init_render_pipeline(
            &device,
            &bind_group_layout,
            depth_texture.format(),
            config.format,
        );
        let start_time = time::Instant::now();
        Self {
            surface,
            device,
            queue,
            config,
            depth_texture,
            depth_view,
            texture,
            texture_view,
            sampler,
            vertices,
            vertex_buffer,
            uniforms,
            uniform_buffer,
            bind_group_layout,
            bind_group,
            render_pipeline,
            start_time,
        }
    }

    pub fn update(&mut self) {
        self.uniforms.time = self.start_time.elapsed().as_secs_f32();
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );
    }
    pub fn render(&mut self) {
        let output = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            // If we timed out, just try again
            Err(wgpu::SurfaceError::Timeout) => self.surface
                .get_current_texture()
                .expect("Failed to acquire next surface texture!"),
            Err(
                // If the surface is outdated, or was lost, reconfigure it.
                wgpu::SurfaceError::Outdated
                | wgpu::SurfaceError::Lost
                // If OutOfMemory happens, reconfiguring may not help, but we might as well try
                | wgpu::SurfaceError::OutOfMemory,
            ) => {
                self.surface.configure(&self.device, &self.config);
                self.surface
                    .get_current_texture()
                    .expect("Failed to acquire next surface texture!")
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.05,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            // render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.set_bind_group(0, &self.bind_group, &[]);

            render_pass.draw(0..self.vertices.len() as u32, 0..1);
        }

        let command = encoder.finish();

        self.queue.submit([command]);

        output.present();
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            (self.depth_texture, self.depth_view) =
                init_depth(&self.device, new_size.width, new_size.height);
            let aspect = new_size.width as f32 / new_size.height as f32;
            self.uniforms.projection =
                Mat4::perspective_lh(f32::to_radians(45.0), aspect, 0.01, 100.0);
        }
    }
}

fn init_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    let (texture, texture_view) =
        load_texture("resources/fourareen/fourareen2K_albedo.jpg", device, queue).unwrap();

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Texture"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        lod_min_clamp: 0.0,
        lod_max_clamp: 8.0,
        compare: None,
        anisotropy_clamp: 1,
        border_color: None,
    });
    (texture, texture_view, sampler)
}

fn init_geometry(device: &wgpu::Device) -> (Vec<VertexAttribute>, wgpu::Buffer) {
    let vertices = load_geometry("resources/fourareen/fourareen.obj");

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
    });
    // let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    //     label: Some("Index Buffer"),
    //     contents: bytemuck::cast_slice(&indices),
    //     usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::INDEX,
    // });
    (vertices, vertex_buffer)
}
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct Uniforms {
    model: Mat4,
    view: Mat4,
    projection: Mat4,
    color: Vec4,
    time: f32,
    _padding: [f32; 3],
}
fn init_uniforms(device: &wgpu::Device, aspect: f32) -> (Uniforms, wgpu::Buffer) {
    let start_time = time::Instant::now();

    let uniforms = Uniforms {
        model: Mat4::IDENTITY,
        view: Mat4::look_at_lh(Vec3::new(-2.0, -3.0, 2.0), Vec3::ZERO, Vec3::Z),
        projection: Mat4::perspective_lh(f32::to_radians(45.0), aspect, 0.01, 100.0),
        color: Vec4::new(0.0, 1.0, 0.4, 1.0),
        time: start_time.elapsed().as_secs_f32(),
        _padding: Default::default(),
    };
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniform Buffer"),
        contents: bytemuck::cast_slice(&[uniforms]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
    });
    (uniforms, uniform_buffer)
}

fn init_bind_groups(
    device: &wgpu::Device,
    uniform_buffer: &wgpu::Buffer,
    texture_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Uniform Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Uniform Bind Group Layout"),

        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    (bind_group_layout, bind_group)
}

fn init_render_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    depth_texture_format: wgpu::TextureFormat,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: mem::size_of::<VertexAttribute>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x3,3=>Float32x2],
            }],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_texture_format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::all(),
            })],
        }),
        multiview: None,
    })
}
fn init_wgpu(
    window: &Arc<Window>,
) -> (
    wgpu::Surface<'static>,
    wgpu::Device,
    wgpu::Queue,
    wgpu::SurfaceConfiguration,
) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    info!("{instance:?}");

    let surface = instance.create_surface(window.clone()).unwrap();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .block_on()
        .unwrap();
    info!("{adapter:?}");

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
            },
            None,
        )
        .block_on()
        .unwrap();

    device.on_uncaptured_error(Box::new(|e| {
        error!("{e}");
    }));

    queue.on_submitted_work_done(Box::new(|| {
        info!("Submitted work on queue done.");
    }));

    let surface_caps = surface.get_capabilities(&adapter);
    info!("{surface_caps:?}");
    // Shader code in this tutorial assumes an sRGB surface texture. Using a different
    // one will result in all the colors coming out darker. If you want to support non
    // sRGB surfaces, you'll need to account for that when drawing to the frame.
    let surface_format = surface_caps
        .formats
        .iter()
        .find(|f| f.is_srgb())
        .unwrap_or(&surface_caps.formats[0]);

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: *surface_format,
        width: window.inner_size().width,
        height: window.inner_size().height,
        present_mode: surface_caps.present_modes[0],
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    info!("{config:?}");
    surface.configure(&device, &config);
    (surface, device, queue, config)
}

fn init_depth(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let depth_texture_format = wgpu::TextureFormat::Depth24Plus;
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: depth_texture_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[depth_texture_format],
    });

    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Depth Texture View"),
        format: Some(depth_texture_format),
        dimension: Some(wgpu::TextureViewDimension::D2),
        aspect: wgpu::TextureAspect::DepthOnly,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: 0,
        array_layer_count: Some(1),
    });
    (depth_texture, depth_view)
}

pub struct Application {
    state: ApplicationState,
    window: Arc<Window>,
    event_loop: EventLoop<()>,
}
impl Application {
    pub fn new() -> Self {
        tracing_subscriber::fmt().init();
        let event_loop = EventLoop::new().unwrap();
        let window = Arc::new(WindowBuilder::new().build(&event_loop).unwrap());

        event_loop.set_control_flow(ControlFlow::Poll);

        let state = ApplicationState::new(&window);
        Self {
            state,
            window,
            event_loop,
        }
    }
    pub fn run(mut self) -> Result<(), winit::error::EventLoopError> {
        self.event_loop.run(move |event, elwt| match event {
            Event::WindowEvent {
                event: window_event,
                ..
            } => match window_event {
                WindowEvent::Resized(new_size) => self.state.resize(new_size),
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::RedrawRequested => self.window.request_redraw(),

                _ => {}
            },
            Event::AboutToWait => {
                self.state.update();
                self.state.render();
            }

            _ => (),
        })
    }
}
