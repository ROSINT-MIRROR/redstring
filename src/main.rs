mod app;
mod edge_shape;
mod model;

use app::RedstringApp;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "redstring",
        options,
        Box::new(|cc| Ok(Box::new(RedstringApp::new(cc)))),
    )
}
