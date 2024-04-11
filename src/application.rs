#![allow(clippy::module_name_repetitions)]
mod bind_group;
mod render_pipeline;
mod texture;
mod wgpu_context;

use std::{
    sync::Arc,
    time::{self, Duration, Instant},
};

use egui_wgpu::ScreenDescriptor;
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, Event, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowBuilder},
};

use crate::{
    gui::{EguiRenderer, GuiState},
    resources::{load_geometry, VertexAttribute},
};

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
    delta_time: Duration,
    camera: Camera,
    mouse_pos: PhysicalPosition<f64>,
    drag: bool,
    egui: EguiRenderer,
    window: Arc<Window>,
    gui_state: GuiState,
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
        let camera_pos = Vec3::new(-0.5, -0.5, 0.5);

        let egui = EguiRenderer::new(
            &wgpu.device,       // wgpu Device
            wgpu.config.format, // TextureFormat
            None,               // this can be None
            1,                  // samples
            window,             // winit Window
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
            delta_time: Duration::from_secs_f64(1.0 / 144.0),
            mouse_pos: PhysicalPosition::default(),
            camera: Camera {
                rotation: Quat::from_mat4(&Mat4::look_at_lh(camera_pos, Vec3::ZERO, Vec3::Z)),
                translation: camera_pos,
                velocity: Vec3::ZERO,
            },
            drag: false,
            egui,
            window: window.clone(),
            gui_state: GuiState::default(),
        }
    }

    pub fn update(&mut self) {
        const SPEED: f32 = 2.0;
        let begin_frame_time = time::Instant::now();

        self.uniform_buffer.data.time = self.start_time.elapsed().as_secs_f32();
        self.camera.translation += self.camera.velocity;
        self.camera.velocity *= 0.9;
        self.uniform_buffer.data.view = self.camera.get_view_matrix();

        self.uniform_buffer.update(&self.wgpu.queue);

        self.render();

        let end_frame_time = time::Instant::now();
        self.delta_time = (end_frame_time - begin_frame_time);
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
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.wgpu.config.width, self.wgpu.config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        self.egui.draw(
            &self.wgpu.device,
            &self.wgpu.queue,
            &mut encoder,
            &self.window,
            &view,
            &screen_descriptor,
            |ui| self.gui_state.gui(ui, self.delta_time),
        );

        let command = encoder.finish();

        self.wgpu.queue.submit([command]);

        output.present();
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.wgpu.resize(new_size.width, new_size.height);
            self.depth_texture =
                texture::Texture::depth(&self.wgpu.device, new_size.width, new_size.height);
            let aspect = new_size.width as f32 / new_size.height as f32;
            self.uniform_buffer.data.projection =
                Mat4::perspective_lh(f32::to_radians(45.0), aspect, 0.01, 100.0);
        }
    }

    fn mouse_moved(&mut self, position: PhysicalPosition<f64>) {
        const SENSITIVITY: f32 = 0.005;
        if self.drag {
            let delta = (Vec2::new(position.x as f32, position.y as f32)
                - Vec2::new(self.mouse_pos.x as f32, self.mouse_pos.y as f32))
                * SENSITIVITY;

            let mut eulers = self.camera.rotation.to_euler(glam::EulerRot::XYZ);
            // eulers.0 = f32::clamp(eulers.0 + delta.y, PI / 2.0 + 1e-5, 3.0 * PI / 2.0 - 1e-5);
            eulers.0 -= delta.y;
            eulers.2 -= delta.x;
            self.camera.rotation =
                Quat::from_euler(glam::EulerRot::XYZ, eulers.0, eulers.1, eulers.2);
            self.camera.rotation = self.camera.rotation.normalize();
        }
        self.mouse_pos = position;
    }

    fn mouse_input(&mut self, button: MouseButton, state: ElementState) {
        if button == MouseButton::Middle {
            match state {
                ElementState::Pressed => self.drag = true,
                ElementState::Released => self.drag = false,
            }
        }
    }

    fn mouse_scroll(&mut self, delta: MouseScrollDelta) {
        const SENSITIVITY: f32 = 0.1;

        match delta {
            MouseScrollDelta::LineDelta(_, y) => self.camera.translation.z -= y * SENSITIVITY,
            MouseScrollDelta::PixelDelta(PhysicalPosition { x: _, y }) => {
                self.camera.translation.z -= y as f32 * SENSITIVITY;
            }
        }
    }

    fn key_input(&mut self, event: KeyCode) {
        let delta_time = self.delta_time.as_secs_f32();
        match event {
            KeyCode::KeyW => self.camera.velocity.z -= delta_time,
            KeyCode::KeyS => self.camera.velocity.z += delta_time,
            KeyCode::KeyD => self.camera.velocity.x -= delta_time,
            KeyCode::KeyA => self.camera.velocity.x += delta_time,
            KeyCode::Space => self.camera.velocity.y -= delta_time,
            KeyCode::ShiftLeft => self.camera.velocity.y += delta_time,
            _ => {}
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

#[derive(Clone, Copy, Default)]
struct Camera {
    rotation: Quat,
    translation: Vec3,
    velocity: Vec3,
}

impl Camera {
    fn get_view_matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.translation)
    }
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
                window_id,
            } if self.window.id() == window_id => {
                match window_event {
                    WindowEvent::Resized(new_size) => self.state.resize(new_size),
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::RedrawRequested => self.window.request_redraw(),
                    WindowEvent::CursorMoved { position, .. } => self.state.mouse_moved(position),
                    WindowEvent::MouseInput { state, button, .. } => {
                        self.state.mouse_input(button, state);
                    }
                    WindowEvent::MouseWheel { delta, .. } => self.state.mouse_scroll(delta),
                    WindowEvent::KeyboardInput {
                        event: KeyEvent { physical_key, .. },
                        ..
                    } => {
                        if let PhysicalKey::Code(key) = physical_key {
                            self.state.key_input(key);
                        }
                    }
                    _ => {}
                }
                self.state.egui.handle_input(&self.window, &window_event);
            }
            Event::AboutToWait => {
                self.state.update();
            }

            _ => (),
        })
    }
}
