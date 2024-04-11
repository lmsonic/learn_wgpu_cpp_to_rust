#![allow(clippy::cargo_common_metadata)]
mod application;
mod gui;
mod resources;

use application::Application;
use winit::error::EventLoopError;

fn main() -> Result<(), EventLoopError> {
    let app = Application::new();
    app.run()
}
