use std::{f32::consts::PI, fmt::Debug, fs, mem, path::Path, time};

use glam::{Mat4, Vec3, Vec4};
use pollster::FutureExt;
use tracing::{error, info};
use wgpu::util::DeviceExt;
use winit::{
    error::EventLoopError,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
fn main() -> Result<(), EventLoopError> {
    tracing_subscriber::fmt().init();
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    event_loop.set_control_flow(ControlFlow::Poll);

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    info!("{instance:?}");

    let surface = instance.create_surface(&window).unwrap();

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

    info!("{device:?}");
    info!("{queue:?}");

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
    let vertices = load_geometry("resources/mammoth.obj");

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
    let start_time = time::Instant::now();

    let model_scale = Mat4::from_scale(Vec3::splat(0.3));
    let model_translation = Mat4::from_translation(Vec3::splat(0.0));
    let angle1 = start_time.elapsed().as_secs_f32();
    let model_rotation = Mat4::from_rotation_z(angle1);
    let model = model_rotation * model_translation * model_scale;

    // Translate the view
    let camera_position = Vec3::new(0.0, 0.0, -1.0);
    let view_translation = Mat4::from_translation(-camera_position);

    // Rotate the view point
    let angle2 = 3.0 * PI / 4.0;
    let view_rotation = Mat4::from_rotation_x(-angle2);
    let view = view_translation * view_rotation;

    let ratio = window.inner_size().width as f32 / window.inner_size().height as f32;
    let fov = 45.0;
    let near = 0.01;
    let far = 100.0;
    #[rustfmt::skip]
    let projection = Mat4::perspective_lh(f32::to_radians(fov),ratio, near, far);

    let mut uniforms = Uniforms {
        model,
        view,
        projection,
        color: Vec4::new(0.0, 1.0, 0.4, 1.0),
        time: start_time.elapsed().as_secs_f32(),
        _padding: Default::default(),
    };
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniform Buffer"),
        contents: bytemuck::cast_slice(&[uniforms]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Uniform Bind Group Layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Uniform Bind Group Layout"),

        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &uniform_buffer,
                offset: 0,
                size: None,
            }),
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

    let depth_texture_format = wgpu::TextureFormat::Depth24Plus;
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d {
            width: window.inner_size().width,
            height: window.inner_size().height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: depth_texture_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[depth_texture_format],
    });
    let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Depth Texture View"),
        format: Some(depth_texture_format),
        dimension: Some(wgpu::TextureViewDimension::D2),
        aspect: wgpu::TextureAspect::DepthOnly,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: 0,
        array_layer_count: Some(1),
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: mem::size_of::<VertexAttribute>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x3],
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
                format: *surface_format,
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
    });

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                println!("The close button was pressed; stopping");
                elwt.exit();
            }
            Event::AboutToWait => {
                // Application update code.

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.
                let output = surface
                    .get_current_texture()
                    .expect("Cannot acquire next swap chain texture");
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
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
                            view: &depth_texture_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    render_pass.set_pipeline(&render_pipeline);
                    render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    // render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.set_bind_group(0, &bind_group, &[]);

                    let angle1 = start_time.elapsed().as_secs_f32();
                    let model_rotation = Mat4::from_rotation_z(angle1);
                    uniforms.model = model_rotation * model_translation * model_scale;
                    queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

                    render_pass.draw(0..vertices.len() as u32, 0..1);
                }

                let command = encoder.finish();

                queue.submit([command]);

                output.present();
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.
            }
            _ => (),
        }
    })
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct VertexAttribute {
    position: Vec3,
    normal: Vec3,
    color: Vec3,
}
fn load_geometry(path: impl AsRef<Path> + Debug) -> Vec<VertexAttribute> {
    let (models, _) = tobj::load_obj(
        path,
        &tobj::LoadOptions {
            single_index: true,
            triangulate: true,
            ignore_points: true,
            ignore_lines: true,
        },
    )
    .expect("Failed to OBJ load file");
    let mut vertices = vec![];
    for model in &models {
        let mesh = &model.mesh;
        vertices.reserve(mesh.indices.len());
        for index in &mesh.indices {
            let i = *index as usize;
            vertices.push(VertexAttribute {
                position: Vec3::new(
                    mesh.positions[i * 3],
                    // Z is up
                    -mesh.positions[i * 3 + 2],
                    mesh.positions[i * 3 + 1],
                ),
                normal: Vec3::new(
                    mesh.normals[i * 3],
                    // Z is up
                    -mesh.normals[i * 3 + 2],
                    mesh.normals[i * 3 + 1],
                ),
                color: Vec3::splat(1.0),
            });
        }
    }
    vertices
}
