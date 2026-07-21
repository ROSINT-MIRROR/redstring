mod error;
mod log;
mod model;
mod modules;
mod prefs;
mod project;
mod shapes;
mod ui;

use std::path::Path;

use ui::RedstringApp;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let check = args.iter().position(|a| a == "--check");
    let Some(check_at) = check else {
        return run_after_flags(&args);
    };
    run_check(args.get(check_at + 1).cloned())
}

fn run_after_flags(args: &[String]) -> eframe::Result {
    if args.iter().any(|a| a == "--debug") {
        enable_debug();
    }
    run_gui()
}

fn enable_debug() {
    let Err(err) = log::enable() else {
        return;
    };
    eprintln!("debug logging unavailable: {err}");
}

fn run_gui() -> eframe::Result {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "redstring",
        options,
        Box::new(|cc| Ok(Box::new(RedstringApp::new(cc)?))),
    )
}

fn run_check(path: Option<String>) -> ! {
    let Some(target) = path else {
        check_case(".");
    };
    check_case(&target)
}

fn check_case(target: &str) -> ! {
    let result = project::Project::open(Path::new(target)).and_then(|p| project::validate(&p));
    let findings = match result {
        Ok(f) => f,
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    };
    if findings.is_empty() {
        println!("ok: case is consistent");
        std::process::exit(0);
    }
    for f in &findings {
        println!("warn: {f}");
    }
    std::process::exit(1);
}
