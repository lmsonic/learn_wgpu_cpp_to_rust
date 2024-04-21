#![allow(clippy::module_name_repetitions)]
mod bind_group;
mod render_pipeline;
mod texture;
mod wgpu_context;

use std::{
    f32::consts::PI,
    sync::Arc,
    time::{self, Duration, Instant},
};

use egui_wgpu::ScreenDescriptor;
use glam::{Mat4, Quat, Vec3, Vec4};

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
    buffer::{IndexBuffer, UniformBuffer, VertexBuffer},
    texture::Texture,
    wgpu_context::WgpuContext,
};
pub struct ApplicationState {
    wgpu: WgpuContext,
    depth_texture: Texture,
    texture: Texture,
    normal_texture: Texture,
    vertex_buffer: VertexBuffer<VertexAttribute>,
    index_buffer: IndexBuffer,
    uniforms: UniformBuffer<Uniforms>,
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
    light_uniforms: UniformBuffer<LightUniforms>,
}

impl ApplicationState {
    pub fn new(window: &Arc<Window>) -> Self {
        let size = window.inner_size();
        let wgpu = WgpuContext::new(window);
        let depth_texture = Texture::depth(&wgpu.device, size.width, size.height);
        let texture = Texture::new("resources/fourareen/fourareen2K_albedo.jpg", &wgpu);
        let normal_texture = Texture::new("resources/fourareen/fourareen2K_normals.png", &wgpu);

        let (vertices, indices) = load_geometry("resources/fourareen/fourareen.obj");
        let vertex_buffer = VertexBuffer::new(vertices, &wgpu.device);
        let index_buffer = IndexBuffer::new(indices, &wgpu.device);

        let start_time = time::Instant::now();
        let aspect = size.width as f32 / size.height as f32;

        let camera = Camera {
            orbit_radius: 2.0,
            ..Default::default()
        };

        let uniforms = Uniforms {
            model: Mat4::IDENTITY,
            view: camera.get_view_matrix(),
            projection: Mat4::perspective_lh(f32::to_radians(45.0), aspect, 0.01, 100.0),
            color: Vec4::new(0.0, 1.0, 0.4, 1.0),
            time: start_time.elapsed().as_secs_f32(),
            camera_world_position: camera.get_translation(),
            normal_map_strength: 0.5,
            ..Default::default()
        };
        let uniform_buffer = UniformBuffer::new(uniforms, &wgpu.device);

        let light_uniforms = UniformBuffer::new(
            LightUniforms {
                directions: [[0.5, -0.9, 0.1, 0.0].into(), [0.2, 0.4, 0.3, 0.0].into()],
                colors: [[1.0, 0.9, 0.6, 1.0].into(), [0.6, 0.9, 1.0, 1.0].into()],
                hardness: 16.0,
                diffuse: 1.0,
                specular: 0.5,
                ..Default::default()
            },
            &wgpu.device,
        );

        let bind_group = BindGroup::new(
            &wgpu.device,
            &[&uniform_buffer.buffer, &light_uniforms.buffer],
            &[&texture, &normal_texture],
        );
        let render_pipeline = render_pipeline::RenderPipeline::new::<VertexAttribute>(
            &wgpu.device,
            &bind_group.bind_group_layout,
            depth_texture.texture.format(),
            wgpu.config.format,
            wgpu::include_wgsl!("shader.wgsl"),
        );

        let egui = EguiRenderer::new(
            &wgpu.device,       // wgpu Device
            wgpu.config.format, // TextureFormat
            None,               // this can be None
            1,                  // samples
            window,             // winit Window
        );

        let gui_state = GuiState {
            clear_color: [0.05, 0.05, 0.05],
            light_color1: light_uniforms.data.colors[0].truncate().to_array(),
            light_color2: light_uniforms.data.colors[1].truncate().to_array(),
            light_direction1: light_uniforms.data.directions[0],
            light_direction2: light_uniforms.data.directions[1],
            hardness: light_uniforms.data.hardness,
            diffuse: light_uniforms.data.diffuse,
            specular: light_uniforms.data.specular,
            normal_strength: uniform_buffer.data.normal_map_strength,
        };
        Self {
            wgpu,
            depth_texture,
            texture,
            normal_texture,
            vertex_buffer,
            index_buffer,
            uniforms: uniform_buffer,
            bind_group,
            render_pipeline,
            start_time,
            delta_time: Duration::from_secs_f64(1.0 / 144.0),
            mouse_pos: PhysicalPosition::default(),
            camera,
            drag: false,
            egui,
            window: window.clone(),
            gui_state,
            light_uniforms,
        }
    }

    pub fn update(&mut self) {
        let begin_frame_time = time::Instant::now();

        self.uniforms.data.time = self.start_time.elapsed().as_secs_f32();

        self.uniforms.data.view = self.camera.get_view_matrix();
        self.uniforms.data.camera_world_position = self.camera.get_translation();

        self.uniforms.update(&self.wgpu.queue);

        self.light_uniforms.update(&self.wgpu.queue);

        self.render();

        let end_frame_time = time::Instant::now();
        self.delta_time = end_frame_time - begin_frame_time;
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
                            r: self.gui_state.clear_color[0].into(),
                            g: self.gui_state.clear_color[0].into(),
                            b: self.gui_state.clear_color[0].into(),
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
            render_pass.set_index_buffer(
                self.index_buffer.buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.set_bind_group(0, &self.bind_group.bind_group, &[]);
            render_pass.draw_indexed(0..self.index_buffer.indices.len() as u32, 0, 0..1);
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
        self.light_uniforms.data = LightUniforms {
            directions: [
                self.gui_state.light_direction1,
                self.gui_state.light_direction2,
            ],
            colors: [
                Vec3::from(self.gui_state.light_color1).extend(1.0),
                Vec3::from(self.gui_state.light_color2).extend(1.0),
            ],
            hardness: self.gui_state.hardness,
            diffuse: self.gui_state.diffuse,
            specular: self.gui_state.specular,
            _padding: Default::default(),
        };
        self.uniforms.data.normal_map_strength = self.gui_state.normal_strength;

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
            self.uniforms.data.projection =
                Mat4::perspective_lh(f32::to_radians(45.0), aspect, 0.01, 100.0);
        }
    }

    fn mouse_moved(&mut self, position: PhysicalPosition<f64>) {
        const SENSITIVITY: f32 = 0.005;
        if self.drag {
            let delta_y = (position.y - self.mouse_pos.y) as f32 * SENSITIVITY;
            let delta_x = (position.x - self.mouse_pos.x) as f32 * SENSITIVITY;
            self.camera.yaw += delta_x;
            self.camera.pitch -= delta_y;
            self.camera.pitch = self.camera.pitch.clamp(-PI * 0.4, PI * 0.4);
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
            MouseScrollDelta::LineDelta(_, y) => self.camera.orbit_radius -= y * SENSITIVITY,
            MouseScrollDelta::PixelDelta(PhysicalPosition { x: _, y }) => {
                self.camera.orbit_radius -= y as f32 * SENSITIVITY;
            }
        }
    }

    fn key_input(&mut self, event: KeyCode) {
        if self.drag {
            return;
        }
        // let delta_time = self.delta_time.as_secs_f32();
        // match event {
        //     KeyCode::KeyW => self.camera.velocity.z -= delta_time,
        //     KeyCode::KeyS => self.camera.velocity.z += delta_time,
        //     KeyCode::KeyD => self.camera.velocity.x -= delta_time,
        //     KeyCode::KeyA => self.camera.velocity.x += delta_time,
        //     KeyCode::Space => self.camera.velocity.y -= delta_time,
        //     KeyCode::ShiftLeft => self.camera.velocity.y += delta_time,
        //     _ => {}
        // }
    }
}

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct Uniforms {
    model: Mat4,
    view: Mat4,
    projection: Mat4,
    color: Vec4,
    camera_world_position: Vec3,
    time: f32,
    normal_map_strength: f32,
    _padding: [f32; 3],
}

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct LightUniforms {
    directions: [Vec4; 2],
    colors: [Vec4; 2],
    hardness: f32,
    diffuse: f32,
    specular: f32,
    _padding: f32,
}

#[derive(Clone, Copy, Default)]
struct Camera {
    orbit_radius: f32,
    yaw: f32,
    pitch: f32,
}

impl Camera {
    fn get_translation(&self) -> Vec3 {
        Quat::from_rotation_y(self.yaw)
            * Quat::from_rotation_x(self.pitch)
            * Vec3::Z
            * self.orbit_radius
    }
    fn get_view_matrix(&self) -> Mat4 {
        Mat4::look_at_lh(self.get_translation(), Vec3::ZERO, Vec3::Y)
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
                let consumed = self.state.egui.handle_input(&self.window, &window_event);
                if consumed {
                    return;
                }
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
            }
            Event::AboutToWait => {
                self.state.update();
            }

            _ => (),
        })
    }
}
