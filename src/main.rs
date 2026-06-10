mod error;
mod model;
mod modules;
mod prefs;
mod project;
mod shapes;
mod ui;

use std::path::Path;

use ui::RedstringApp;

fn main() -> eframe::Result {
    let mut args = std::env::args().skip(1);
    let Some(flag) = args.next() else {
        return run_gui();
    };
    if flag == "--check" {
        run_check(args.next());
    }
    eprintln!("usage: redstring [--check <case dir or case.redstring>]");
    std::process::exit(2);
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
