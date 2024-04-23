#![allow(clippy::cargo_common_metadata)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::struct_field_names)]
mod application;
mod compute;
mod gui;
mod resources;

use application::Application;
use winit::error::EventLoopError;

fn main() -> Result<(), EventLoopError> {
    let app = Application::new();
    app.run()
}
