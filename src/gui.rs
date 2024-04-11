use std::f32::consts::{PI, TAU};
use std::time::Duration;

use egui::epaint::Shadow;
use egui::{Context, Ui, Visuals};
use egui_wgpu::Renderer;
use egui_wgpu::ScreenDescriptor;

use egui_winit::State;

use glam::{vec2, vec3, DVec3, Vec2, Vec3, Vec4, Vec4Swizzles};
use tracing_subscriber::fmt::format;
use wgpu::{CommandEncoder, Device, Queue, TextureFormat, TextureView};
use winit::event::WindowEvent;
use winit::window::Window;

#[derive(Default)]
pub struct GuiState {
    pub float: f32,
    pub counter: i32,
    pub show_demo_window: bool,
    pub show_other_window: bool,
    pub clear_color: [f32; 3],
    pub light_direction1: Vec4,
    pub light_color1: [f32; 3],
    pub light_direction2: Vec4,
    pub light_color2: [f32; 3],
}

impl GuiState {
    #[allow(clippy::shadow_unrelated)]
    pub fn gui(&mut self, ui: &Context, delta_time: Duration) {
        egui::Window::new("Hello, world!")
            .resizable(true)
            .vscroll(true)
            .default_open(false)
            .show(&ui, |ui| {
                ui.label("This is some useful text.");
                ui.checkbox(&mut self.show_demo_window, "Demo Window");
                ui.checkbox(&mut self.show_other_window, "Another Window");
                ui.add(egui::Slider::new(&mut self.float, 0.0..=1.0).text("float"));
                ui.color_edit_button_rgb(&mut self.clear_color);

                drag_direction(ui, &mut self.light_direction1);

                ui.color_edit_button_rgb(&mut self.light_color1);

                drag_direction(ui, &mut self.light_direction2);

                ui.color_edit_button_rgb(&mut self.light_color2);

                ui.horizontal(|ui| {
                    if ui.button("Click me!").clicked() {
                        self.counter += 1;
                    }
                    ui.label(format!("counter = {}", self.counter));
                });
                ui.label(format!(
                    "Application average {} ms/frame {:.3}",
                    delta_time.as_millis(),
                    delta_time.as_secs_f32()
                ));
            });
    }
}
fn drag_direction(ui: &mut Ui, v: &mut Vec4) {
    let v3 = v.truncate();
    let mut polar = cartesian_to_polar(v3);
    ui.horizontal(|ui| {
        ui.drag_angle(&mut polar.x);
        polar.x = polar.x.clamp(-PI * 0.5, PI * 0.5);
        ui.drag_angle(&mut polar.y);
        polar.y = polar.y.clamp(-PI * 0.5, PI * 0.5);
    });
    *v = polar_to_cartesian(&polar).extend(0.0);
}

fn cartesian_to_polar(cartesian: Vec3) -> Vec2 {
    let length = cartesian.length();
    let normalized = cartesian / length;
    Vec2 {
        x: normalized.y.asin(),                  // latitude
        y: (normalized.x / normalized.z).atan(), // longitude
    }
}

fn polar_to_cartesian(polar: &Vec2) -> Vec3 {
    let latitude = polar.x;
    let longitude = polar.y;
    Vec3 {
        x: latitude.cos() * longitude.sin(),
        y: latitude.sin(),
        z: latitude.cos() * longitude.cos(),
    }
}

pub struct EguiRenderer {
    pub context: Context,
    state: State,
    renderer: Renderer,
}

impl EguiRenderer {
    pub fn new(
        device: &Device,
        output_color_format: TextureFormat,
        output_depth_format: Option<TextureFormat>,
        msaa_samples: u32,
        window: &Window,
    ) -> Self {
        const BORDER_RADIUS: f32 = 2.0;
        let egui_context = Context::default();
        let id = egui_context.viewport_id();

        let visuals = Visuals {
            window_rounding: egui::Rounding::same(BORDER_RADIUS),
            window_shadow: Shadow::NONE,
            // menu_rounding: todo!(),
            ..Default::default()
        };

        egui_context.set_visuals(visuals);

        let egui_state = egui_winit::State::new(egui_context.clone(), id, &window, None, None);

        // egui_state.set_pixels_per_point(window.scale_factor() as f32);
        let egui_renderer = egui_wgpu::Renderer::new(
            device,
            output_color_format,
            output_depth_format,
            msaa_samples,
        );

        Self {
            context: egui_context,
            state: egui_state,
            renderer: egui_renderer,
        }
    }

    pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) -> bool {
        let response = self.state.on_window_event(window, event);
        response.consumed
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        window: &Window,
        window_surface_view: &TextureView,
        screen_descriptor: &ScreenDescriptor,
        run_ui: impl FnOnce(&Context),
    ) {
        // self.state.set_pixels_per_point(window.scale_factor() as f32);
        let raw_input = self.state.take_egui_input(window);
        let full_output = self.context.run(raw_input, |_| {
            run_ui(&self.context);
        });

        self.state
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }
        self.renderer
            .update_buffers(device, queue, encoder, &tris, screen_descriptor);
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: window_surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            label: Some("egui Main Render Pass"),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        self.renderer.render(&mut rpass, &tris, screen_descriptor);
        drop(rpass);
        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x);
        }
    }
}
