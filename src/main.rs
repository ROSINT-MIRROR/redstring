mod error;
mod model;
mod prefs;
mod shapes;
mod ui;

use ui::RedstringApp;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "redstring",
        options,
        Box::new(|cc| Ok(Box::new(RedstringApp::new(cc)))),
    )
}
