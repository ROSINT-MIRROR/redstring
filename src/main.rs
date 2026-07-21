mod error;
mod log;
mod model;
mod modules;
mod prefs;
mod project;
mod shapes;
mod shot;
mod ui;

use std::path::Path;

use ui::RedstringApp;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--shot") {
        run_shot(&args)
    } else {
        run_after_check(&args)
    }
}

fn run_after_check(args: &[String]) -> eframe::Result {
    let check = args.iter().position(|a| a == "--check");
    let Some(check_at) = check else {
        return run_after_flags(args);
    };
    run_check(args.get(check_at + 1).cloned())
}

fn run_shot(args: &[String]) -> ! {
    match shot::shoot(args) {
        Ok(report) => {
            println!(
                "ok: rendered {} entities / {} links -> {} [{}] {}",
                report.nodes,
                report.edges,
                report.out.display(),
                report.mode,
                report.backend,
            );
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
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
    let result = project::Project::open(Path::new(target)).and_then(|p| {
        let mut findings = project::validate(&p)?;
        findings.extend(project::source_findings(&p)?);
        Ok(findings)
    });
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
