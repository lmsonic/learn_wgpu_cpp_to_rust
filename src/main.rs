use glam::{vec3, FloatExt};
use image::DynamicImage;
use std::{
    f32::consts::{PI, TAU},
    fmt::Debug,
    mem,
    path::Path,
    time,
};

use glam::{Mat4, Vec2, Vec3, Vec4};
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

    let mut uniforms = Uniforms {
        model: Mat4::IDENTITY,
        view: Mat4::look_at_lh(Vec3::new(-2.0, -3.0, 2.0), Vec3::ZERO, Vec3::Z),
        projection: Mat4::perspective_lh(
            f32::to_radians(45.0),
            window.inner_size().width as f32 / window.inner_size().height as f32,
            0.01,
            100.0,
        ),
        color: Vec4::new(0.0, 1.0, 0.4, 1.0),
        time: start_time.elapsed().as_secs_f32(),
        _padding: Default::default(),
    };
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniform Buffer"),
        contents: bytemuck::cast_slice(&[uniforms]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
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

    let (texture, texture_view) = load_texture(
        "resources/fourareen/fourareen2K_albedo.jpg",
        &device,
        &queue,
    )
    .unwrap();

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
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
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

                    // uniforms.time = start_time.elapsed().as_secs_f32();

                    // queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

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
fn bit_width(x: u32) -> u32 {
    if x == 0 {
        0
    } else {
        1 + x.ilog2()
    }
}
fn load_texture(
    path: impl AsRef<Path>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> image::ImageResult<(wgpu::Texture, wgpu::TextureView)> {
    let image = image::open(&path)?;
    let label = path.as_ref().to_str();
    let texture_label = label.map(|label| format!("{label} Texture"));
    let mip_level_count = bit_width(u32::max(image.width(), image.height()));
    let texture_descriptor = wgpu::TextureDescriptor {
        label: texture_label.as_deref(),
        size: wgpu::Extent3d {
            width: image.width(),
            height: image.height(),
            depth_or_array_layers: 1,
        },
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    };
    let view_label = label.map(|label| format!("{label} Texture View"));
    let texture = device.create_texture(&texture_descriptor);
    write_mipmaps(queue, &texture, image);
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: view_label.as_deref(),
        format: Some(texture.format()),
        dimension: Some(wgpu::TextureViewDimension::D2),
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: 0,
        array_layer_count: Some(1),
    });
    Ok((texture, view))
}

fn write_mipmaps(queue: &wgpu::Queue, texture: &wgpu::Texture, image: DynamicImage) {
    let data = image.into_rgba8().into_raw();
    let mut mip_level_size = texture.size();
    let mut previous_level_pixels = vec![];
    for level in 0..texture.mip_level_count() {
        let mut pixels =
            Vec::with_capacity((4 * mip_level_size.width * mip_level_size.height) as usize);

        if level == 0 {
            pixels = data.clone();
        } else {
            for i in 0..mip_level_size.width {
                for j in 0..mip_level_size.height {
                    // Get the corresponding 4 pixels from the previous level

                    let width = mip_level_size.width as usize;
                    let mip_level_index = 2 * width;
                    let height_index = 2 * j as usize;
                    let width_index = 2 * i as usize;
                    let i00 = 4 * (mip_level_index * height_index + width_index);
                    let i01 = 4 * (mip_level_index * height_index + (width_index + 1));
                    let i10 = 4 * (mip_level_index * (height_index + 1) + width_index);
                    let i11 = 4 * (mip_level_index * (height_index + 1) + (width_index + 1));

                    let p00: &[u8] = &previous_level_pixels[i00..(i00 + 4)];
                    let p01 = &previous_level_pixels[i01..(i01 + 4)];
                    let p10 = &previous_level_pixels[i10..(i10 + 4)];
                    let p11 = &previous_level_pixels[i11..(i11 + 4)];
                    // Average
                    let r = (p00[0] as u32 + p01[0] as u32 + p10[0] as u32 + p11[0] as u32) / 4;
                    let g = (p00[1] as u32 + p01[1] as u32 + p10[1] as u32 + p11[1] as u32) / 4;
                    let b = (p00[2] as u32 + p01[2] as u32 + p10[2] as u32 + p11[2] as u32) / 4;
                    let a = (p00[3] as u32 + p01[3] as u32 + p10[3] as u32 + p11[3] as u32) / 4;
                    pixels.extend([r as u8, g as u8, b as u8, a as u8])
                }
            }
        }
        let destination = wgpu::ImageCopyTextureBase {
            texture,
            mip_level: level,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        };
        let source = wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * mip_level_size.width),
            rows_per_image: Some(mip_level_size.height),
        };
        queue.write_texture(destination, &pixels, source, mip_level_size);
        mip_level_size.width /= 2;
        mip_level_size.height /= 2;
        previous_level_pixels = pixels;
    }
}
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct VertexAttribute {
    position: Vec3,
    normal: Vec3,
    color: Vec3,
    uv: Vec2,
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
                normal: if mesh.normals.is_empty() {
                    Vec3::ZERO
                } else {
                    Vec3::new(
                        mesh.normals[i * 3],
                        // Z is up
                        -mesh.normals[i * 3 + 2],
                        mesh.normals[i * 3 + 1],
                    )
                },
                color: if mesh.vertex_color.is_empty() {
                    Vec3::ONE
                } else {
                    Vec3::new(
                        mesh.vertex_color[i * 3],
                        -mesh.vertex_color[i * 3 + 2],
                        mesh.vertex_color[i * 3 + 1],
                    )
                },
                uv: if mesh.texcoords.is_empty() {
                    Vec2::ZERO
                } else {
                    Vec2::new(
                        mesh.texcoords[i * 2],
                        // Modern graphics APIs use a different UV coordinate system than the OBJ file format.
                        1.0 - mesh.texcoords[i * 2 + 1],
                    )
                },
            });
        }
    }
    vertices
}
