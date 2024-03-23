use std::{fs, mem, num::NonZeroU64, path::Path, time};

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

    // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
    // dispatched any events. This is ideal for games and similar applications.
    event_loop.set_control_flow(ControlFlow::Poll);

    // ControlFlow::Wait pauses the event loop if no events are available to process.
    // This is ideal for non-game applications that only update in response to user
    // input, and uses significantly less power/CPU time than ControlFlow::Poll.
    // event_loop.set_control_flow(ControlFlow::Wait);

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

    info!("{:?}", adapter.features());

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

    let (vertices, indices) = load_geometry("resources/webgpu.txt");
    println!("{vertices:?}");
    println!("{indices:?}");
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Index Buffer"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::INDEX,
    });

    #[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(C)]
    struct Uniforms {
        color: [f32; 4],
        time: f32,
        _padding: [f32; 3],
    }
    let start_time = time::Instant::now();
    let mut uniforms = Uniforms {
        time: start_time.elapsed().as_secs_f32(),
        color: [1.0, 1.0, 1.0, 1.0],
        _padding: Default::default(),
    };
    // let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    //     label: Some("Uniform Buffer"),
    //     contents: bytemuck::cast_slice(&[uniforms]),
    //     usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
    // });
    fn divide_and_ceil(value: usize, step: usize) -> usize {
        let divide_and_ceil = value / step + if value % step == 0 { 0 } else { 1 };
        step * divide_and_ceil
    }
    let size = mem::size_of::<Uniforms>();
    let uniform_stride = divide_and_ceil(
        size,
        device.limits().min_uniform_buffer_offset_alignment as usize,
    );
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Uniform Buffer"),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        size: (uniform_stride + size) as u64,
        mapped_at_creation: false,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Uniform Bind Group Layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
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
                size: Some(NonZeroU64::new(size as u64).unwrap()),
            }),
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: mem::size_of::<[f32; 5]>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x3],
            }],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
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
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    render_pass.set_pipeline(&render_pipeline);
                    render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);

                    uniforms.time = start_time.elapsed().as_secs_f32();
                    uniforms.color = [0.0, 1.0, 0.4, 1.0];
                    queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

                    let mut dynamic_offset = 0;
                    render_pass.set_bind_group(0, &bind_group, &[dynamic_offset]);
                    render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);

                    uniforms.time -= 1.0;
                    uniforms.color = [1.0, 1.0, 1.0, 0.7];
                    queue.write_buffer(
                        &uniform_buffer,
                        uniform_stride as u64,
                        bytemuck::cast_slice(&[uniforms]),
                    );
                    dynamic_offset = uniform_stride as u32;
                    render_pass.set_bind_group(0, &bind_group, &[dynamic_offset]);
                    render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
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

fn load_geometry(path: impl AsRef<Path>) -> (Vec<f32>, Vec<u16>) {
    let file = fs::read_to_string(path).expect("File not found ");
    let mut vertices = vec![];
    let mut indices = vec![];
    enum Section {
        Points,
        Indices,
    }
    let mut section = Section::Points;
    for line in file.lines() {
        if line == "[points]" {
            section = Section::Points;
            continue;
        }
        if line == "[indices]" {
            section = Section::Indices;
            continue;
        }
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        match section {
            Section::Points => {
                let numbers = line
                    .split_whitespace()
                    .map(|n| n.parse::<f32>().unwrap())
                    .collect::<Vec<_>>();
                vertices.extend(numbers);
            }
            Section::Indices => {
                let numbers = line
                    .split_whitespace()
                    .map(|n| n.parse::<u16>().unwrap())
                    .collect::<Vec<_>>();
                indices.extend(numbers);
            }
        }
    }
    (vertices, indices)
}
