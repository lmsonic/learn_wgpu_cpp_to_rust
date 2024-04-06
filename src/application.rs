#![allow(clippy::module_name_repetitions)]
mod bind_group;
mod render_pipeline;
mod texture;
mod wgpu_context;

use std::{
    sync::Arc,
    time::{self, Instant},
};

use glam::{Mat4, Vec3, Vec4};

use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

use crate::resources::{load_geometry, VertexAttribute};

mod buffer;

use self::{
    bind_group::BindGroup,
    buffer::{UniformBuffer, VertexBuffer},
    texture::Texture,
    wgpu_context::WgpuContext,
};
pub struct ApplicationState {
    wgpu: WgpuContext,
    depth_texture: Texture,
    texture: Texture,
    vertex_buffer: VertexBuffer<VertexAttribute>,
    uniform_buffer: UniformBuffer<Uniforms>,
    bind_group: BindGroup,
    render_pipeline: render_pipeline::RenderPipeline,
    start_time: Instant,
}

impl ApplicationState {
    pub fn new(window: &Arc<Window>) -> Self {
        let size = window.inner_size();
        let wgpu = WgpuContext::new(window);
        let depth_texture = Texture::depth(&wgpu.device, size.width, size.height);
        let texture = Texture::new("resources/fourareen/fourareen2K_albedo.jpg", &wgpu);

        let vertices = load_geometry("resources/fourareen/fourareen.obj");
        let vertex_buffer = VertexBuffer::new(vertices, &wgpu.device);

        let start_time = time::Instant::now();
        let aspect = size.width as f32 / size.height as f32;
        let uniforms = Uniforms {
            model: Mat4::IDENTITY,
            view: Mat4::look_at_lh(Vec3::new(-2.0, -3.0, 2.0), Vec3::ZERO, Vec3::Z),
            projection: Mat4::perspective_lh(f32::to_radians(45.0), aspect, 0.01, 100.0),
            color: Vec4::new(0.0, 1.0, 0.4, 1.0),
            time: start_time.elapsed().as_secs_f32(),
            _padding: Default::default(),
        };
        let uniform_buffer = UniformBuffer::new(uniforms, &wgpu.device);

        let bind_group = BindGroup::new(&wgpu.device, &uniform_buffer.buffer, &[&texture]);
        let render_pipeline = render_pipeline::RenderPipeline::new::<VertexAttribute>(
            &wgpu.device,
            &bind_group.bind_group_layout,
            depth_texture.texture.format(),
            wgpu.config.format,
            wgpu::include_wgsl!("shader.wgsl"),
        );
        Self {
            wgpu,
            depth_texture,
            texture,
            vertex_buffer,
            uniform_buffer,
            bind_group,
            render_pipeline,
            start_time,
        }
    }

    pub fn update(&mut self) {
        self.uniform_buffer.data.time = self.start_time.elapsed().as_secs_f32();
        self.uniform_buffer.update(&self.wgpu.queue);
    }
    pub fn render(&mut self) {
        let output = self.wgpu.get_current_texture();

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .wgpu
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
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline.render_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.buffer.slice(..));
            // render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.set_bind_group(0, &self.bind_group.bind_group, &[]);

            render_pass.draw(0..self.vertex_buffer.vertices.len() as u32, 0..1);
        }

        let command = encoder.finish();

        self.wgpu.queue.submit([command]);

        output.present();
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.wgpu.resize(new_size.width, new_size.height);
            self.depth_texture =
                texture::Texture::depth(&self.wgpu.device, new_size.width, new_size.height);
            let aspect = new_size.width as f32 / new_size.height as f32;
            self.uniform_buffer.data.projection =
                Mat4::perspective_lh(f32::to_radians(45.0), aspect, 0.01, 100.0);
        }
    }
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
