mod error;
mod log;
mod model;
mod modules;
mod prefs;
mod project;
mod shapes;
mod shot;
mod theme;
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
    let Some(at) = args.iter().position(|a| a == "--check") else {
        return run_after_derive(args);
    };
    run_check(args.get(at + 1).cloned())
}

fn run_after_derive(args: &[String]) -> eframe::Result {
    let Some(at) = args.iter().position(|a| a == "--derive") else {
        return run_after_export(args);
    };
    run_derive(args.get(at + 1).cloned())
}

fn run_after_export(args: &[String]) -> eframe::Result {
    let Some(at) = args.iter().position(|a| a == "--export") else {
        return run_after_flags(args);
    };
    run_export(args.get(at + 1).cloned())
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
    let opened: Result<(Vec<String>, Vec<String>, project::SourcingSummary), _> =
        project::Project::open(Path::new(target)).and_then(|p| {
            let structural = project::validate(&p)?;
            let (advisory, summary) = project::source_report(&p)?;
            Ok((structural, advisory, summary))
        });
    let (structural, advisory, summary) = match opened {
        Ok(v) => v,
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(2);
        }
    };
    for f in &structural {
        println!("error: {f}");
    }
    for f in &advisory {
        println!("warn: {f}");
    }
    println!(
        "summary: {} structural error(s), {} advisory warning(s) — {} unproven (0-src) + {} single-source edge(s), {} overclaimed status, {} uncited obs",
        structural.len(),
        advisory.len(),
        summary.unproven_edges,
        summary.single_source_edges,
        summary.overclaimed,
        summary.uncited_obs,
    );
    std::process::exit(check_exit(structural.len(), advisory.len()));
}

fn check_exit(structural: usize, advisory: usize) -> i32 {
    if structural > 0 {
        return 2;
    }
    if advisory > 0 {
        return 1;
    }
    0
}

fn run_derive(path: Option<String>) -> ! {
    let Some(target) = path else {
        derive_case(".");
    };
    derive_case(&target)
}

fn derive_case(target: &str) -> ! {
    let outcome = project::Project::open(Path::new(target)).and_then(|p| {
        let cf = project::read_case(&p)?;
        project::write_json(&p, &cf)?;
        project::write_graph_md(&p, &cf)?;
        project::write_dossiers(&p, &cf)?;
        Ok(p.root.clone())
    });
    match outcome {
        Ok(root) => {
            println!("ok: wrote {}", root.join("case.json").display());
            println!("ok: wrote {}", root.join("graph.md").display());
            println!(
                "ok: wrote per-entity dossiers under {}",
                root.join("entities").display()
            );
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(2);
        }
    }
}

fn run_export(path: Option<String>) -> ! {
    let Some(target) = path else {
        export_case(".");
    };
    export_case(&target)
}

fn export_case(target: &str) -> ! {
    let outcome = project::Project::open(Path::new(target)).and_then(|p| project::export(&p));
    match outcome {
        Ok((path, files, sealed)) => {
            println!(
                "ok: wrote {} ({files} file(s), {sealed} sealed entities redacted)",
                path.display()
            );
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::check_exit;

    #[test]
    fn exit_code_tiers_structural_advisory_clean() {
        assert_eq!(check_exit(0, 0), 0, "clean case classifies exit 0");
        assert_eq!(check_exit(0, 3), 1, "advisory-only classifies exit 1");
        assert_eq!(check_exit(2, 0), 2, "structural findings classify exit 2");
        assert_eq!(
            check_exit(1, 5),
            2,
            "any structural finding dominates advisory ones"
        );
    }
}
