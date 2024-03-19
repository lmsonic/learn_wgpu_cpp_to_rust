use std::mem;

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

    let vertex_data: [f32; 12] = [
        -0.5, -0.5, 0.5, -0.5, 0.0, 0.5, -0.55, -0.5, -0.05, 0.5, -0.55, 0.5,
    ];
    let vertex_count = vertex_data.len() / 2;
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(&vertex_data),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
    });

    let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: mem::size_of::<[f32; 2]>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                }],
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
                                    r: 0.9,
                                    g: 0.1,
                                    b: 0.2,
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
                    render_pass.draw(0..vertex_count as u32, 0..1);
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
