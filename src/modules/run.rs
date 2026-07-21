use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use crate::error::Error;

use super::emit::{Fact, emit};
use super::extract::extractor;
use super::spec::Shape;


pub struct ModuleRun {
    pub run_id: u64,
    pub module_id: String,
    pub module_label: String,
    pub target_slug: String,
    pub target_label: String,

    pub project_root: Option<PathBuf>,

    pub shape_id: String,

    pub facts: Vec<Fact>,

    pub raw: String,
    pub stderr_tail: String,
    pub ok: bool,

    pub cancelled: bool,

    pub evidence_rel: String,

    pub last: bool,
}


pub struct RunInput {
    pub run_id: u64,
    pub module_id: String,
    pub module_label: String,
    pub bin: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub timeout_secs: u64,
    pub shapes: Vec<Shape>,
    pub target_slug: String,
    pub target_label: String,

    pub project_root: Option<PathBuf>,

    pub evidence_rel: String,

    pub cancel: Arc<AtomicBool>,
}


pub fn spawn_module(
    input: RunInput,
    tx: Sender<ModuleRun>,
    ctx: egui::Context,
) -> Result<(), Error> {
    let is_stream = streaming(&input.shapes)?;
    std::thread::spawn(move || {
        if is_stream {
            stream(&input, &tx, &ctx);
            return;
        }
        let run = execute(&input);
        if tx.send(run).is_err() {
            return;
        }
        ctx.request_repaint();
    });
    Ok(())
}

pub fn streaming(shapes: &[Shape]) -> Result<bool, Error> {
    let Some(first) = shapes.first() else {
        return Ok(false);
    };
    Ok(first.extract.id == "ndjson")
}

fn execute(input: &RunInput) -> ModuleRun {
    let (raw, stderr, ok, cancelled) = capture(input);
    finish(input, raw, tail(&stderr), ok, cancelled)
}

fn finish(input: &RunInput, raw: String, stderr_tail: String, ok: bool, cancelled: bool) -> ModuleRun {
    let mut shape_id = String::new();
    let mut facts: Vec<Fact> = Vec::new();
    let mut stderr_tail = stderr_tail;
    let mut ok = ok;
    if ok && !cancelled && !raw.is_empty() {
        match parse_facts(input, &raw) {
            Ok((sid, fs)) => {
                shape_id = sid;
                facts = fs;
            }
            Err(err) => {
                stderr_tail = format!("parse failed: {err}\n{stderr_tail}");
                ok = false;
            }
        }
    }
    ModuleRun {
        run_id: input.run_id,
        module_id: input.module_id.clone(),
        module_label: input.module_label.clone(),
        target_slug: input.target_slug.clone(),
        target_label: input.target_label.clone(),
        project_root: input.project_root.clone(),
        evidence_rel: input.evidence_rel.clone(),
        shape_id,
        facts,
        raw,
        stderr_tail,
        ok,
        cancelled,
        last: true,
    }
}

fn parse_facts(input: &RunInput, raw: &str) -> Result<(String, Vec<Fact>), Error> {
    let Some((shape, records)) = super::extract::sniff(&input.shapes, raw)? else {
        return Ok((String::new(), Vec::new()));
    };
    let facts = super::emit::emit(shape, &records)?;
    Ok((shape.id.clone(), facts))
}


fn capture(input: &RunInput) -> (String, String, bool, bool) {
    let mut cmd = Command::new(&input.bin);
    cmd.args(&input.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in &input.env {
        cmd.env(k, v);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return (
                String::new(),
                format!("failed to launch {}: {e}", input.bin.display()),
                false,
                false,
            );
        }
    };
    let Some(out_pipe) = child.stdout.take() else {
        drop(child.kill());
        return (String::new(), "no stdout pipe".to_owned(), false, false);
    };
    let Some(err_pipe) = child.stderr.take() else {
        drop(child.kill());
        return (String::new(), "no stderr pipe".to_owned(), false, false);
    };
    let out_h = std::thread::spawn(move || read_all(out_pipe));
    let err_h = std::thread::spawn(move || read_all(err_pipe));

    let deadline = Instant::now() + Duration::from_secs(input.timeout_secs.max(1));
    let mut timed_out = false;
    let mut cancelled = false;
    let ok = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) => {}
            Err(e) => {
                drop(child.kill());
                return (join_read(out_h), format!("wait failed: {e}"), false, false);
            }
        }
        if input.cancel.load(Ordering::Relaxed) {
            drop(child.kill());
            cancelled = true;
            break false;
        }
        if Instant::now() >= deadline {
            drop(child.kill());
            timed_out = true;
            break false;
        }
        std::thread::sleep(Duration::from_millis(60));
    };
    let stdout = join_read(out_h);
    let mut stderr = join_read(err_h);
    if timed_out {
        stderr = format!("timed out after {}s\n{stderr}", input.timeout_secs);
    }
    if cancelled {
        stderr = format!("cancelled by user\n{stderr}");
    }
    (stdout, stderr, ok, cancelled)
}

fn read_all(mut pipe: impl Read) -> String {
    let mut buf = Vec::new();
    if pipe.read_to_end(&mut buf).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn join_read(h: std::thread::JoinHandle<String>) -> String {
    let Ok(s) = h.join() else {
        return String::new();
    };
    s
}

fn tail(s: &str) -> String {
    let mut lines: Vec<&str> = s.lines().rev().take(6).collect();
    lines.reverse();
    lines.join("\n")
}


fn stream(input: &RunInput, tx: &Sender<ModuleRun>, ctx: &egui::Context) {
    let mut cmd = Command::new(&input.bin);
    cmd.args(&input.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in &input.env {
        cmd.env(k, v);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            send(tx, ctx, fail_run(input, format!("failed to launch {}: {e}", input.bin.display())));
            return;
        }
    };
    let Some(out_pipe) = child.stdout.take() else {
        drop(child.kill());
        send(tx, ctx, fail_run(input, "no stdout pipe".to_owned()));
        return;
    };
    let Some(err_pipe) = child.stderr.take() else {
        drop(child.kill());
        send(tx, ctx, fail_run(input, "no stderr pipe".to_owned()));
        return;
    };
    let err_h = std::thread::spawn(move || read_all(err_pipe));
    let (line_tx, line_rx) = std::sync::mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(out_pipe).lines() {
            let Ok(l) = line else {
                break;
            };
            if line_tx.send(l).is_err() {
                break;
            }
        }
    });

    let shape = input.shapes.first();
    let deadline = Instant::now() + Duration::from_secs(input.timeout_secs.max(1));
    let mut cancelled = false;
    let mut timed_out = false;
    loop {
        match line_rx.recv_timeout(Duration::from_millis(60)) {
            Ok(line) => {
                let Some(run) = stream_line(input, shape, &line) else {
                    continue;
                };
                send(tx, ctx, run);
            }
            Err(e) => {
                if matches!(e, RecvTimeoutError::Disconnected) {
                    break;
                }
            }
        }
        if input.cancel.load(Ordering::Relaxed) {
            drop(child.kill());
            cancelled = true;
            break;
        }
        if Instant::now() >= deadline {
            drop(child.kill());
            timed_out = true;
            break;
        }
    }
    drop(reader.join());
    let mut stderr = join_read(err_h);
    let status_ok = match child.wait() {
        Ok(s) => s.success(),
        Err(e) => {
            stderr = format!("wait failed: {e}\n{stderr}");
            false
        }
    };
    if timed_out {
        stderr = format!("timed out after {}s\n{stderr}", input.timeout_secs);
    }
    if cancelled {
        stderr = format!("cancelled by user\n{stderr}");
    }
    let ok = status_ok && !timed_out && !cancelled;
    send(tx, ctx, final_run(input, tail(&stderr), ok, cancelled));
}

fn stream_line(input: &RunInput, shape: Option<&Shape>, line: &str) -> Option<ModuleRun> {
    let Some(shape) = shape else {
        return None;
    };
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let Ok(built) = extractor(&shape.extract.id, &shape.extract.config) else {
        return None;
    };
    let Ok(records) = built.extract(trimmed) else {
        return None;
    };
    let Ok(facts) = emit(shape, &records) else {
        return None;
    };
    if facts.is_empty() {
        return None;
    }
    Some(partial_run(input, shape.id.clone(), format!("{trimmed}\n"), facts))
}

fn send(tx: &Sender<ModuleRun>, ctx: &egui::Context, run: ModuleRun) {
    if tx.send(run).is_ok() {
        ctx.request_repaint();
    }
}

fn partial_run(input: &RunInput, shape_id: String, raw: String, facts: Vec<Fact>) -> ModuleRun {
    ModuleRun {
        run_id: input.run_id,
        module_id: input.module_id.clone(),
        module_label: input.module_label.clone(),
        target_slug: input.target_slug.clone(),
        target_label: input.target_label.clone(),
        project_root: input.project_root.clone(),
        evidence_rel: input.evidence_rel.clone(),
        shape_id,
        facts,
        raw,
        stderr_tail: String::new(),
        ok: true,
        cancelled: false,
        last: false,
    }
}

fn final_run(input: &RunInput, stderr_tail: String, ok: bool, cancelled: bool) -> ModuleRun {
    ModuleRun {
        run_id: input.run_id,
        module_id: input.module_id.clone(),
        module_label: input.module_label.clone(),
        target_slug: input.target_slug.clone(),
        target_label: input.target_label.clone(),
        project_root: input.project_root.clone(),
        evidence_rel: input.evidence_rel.clone(),
        shape_id: String::new(),
        facts: Vec::new(),
        raw: String::new(),
        stderr_tail,
        ok,
        cancelled,
        last: true,
    }
}

fn fail_run(input: &RunInput, msg: String) -> ModuleRun {
    final_run(input, tail(&msg), false, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(bin: &str, args: &[&str], timeout: u64) -> RunInput {
        RunInput {
            run_id: 1,
            module_id: "t".into(),
            module_label: "t".into(),
            bin: PathBuf::from(bin),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            env: Vec::new(),
            timeout_secs: timeout,
            shapes: Vec::new(),
            target_slug: "x".into(),
            target_label: "x".into(),
            project_root: None,
            evidence_rel: String::new(),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn capture_collects_stdout_and_exit() {
        let (out, _err, ok, _c) = capture(&input("sh", &["-c", "printf 'a: b\\n'"], 5));
        assert!(ok);
        assert_eq!(out, "a: b\n");

        let (_out, _err, ok, _c) = capture(&input("sh", &["-c", "exit 3"], 5));
        assert!(!ok);
    }

    #[test]
    fn capture_kills_on_timeout() {
        let t0 = Instant::now();
        let (_out, err, ok, cancelled) = capture(&input("sh", &["-c", "sleep 30"], 1));
        assert!(!ok);
        assert!(!cancelled);
        assert!(t0.elapsed() < Duration::from_secs(10), "killed well before 30s");
        assert!(err.contains("timed out"), "{err}");
    }

    #[test]
    fn cancel_flag_kills_promptly() {
        let inp = input("sh", &["-c", "sleep 30"], 25);
        let flag = inp.cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            flag.store(true, Ordering::Relaxed);
        });
        let t0 = Instant::now();
        let (_out, err, ok, cancelled) = capture(&inp);
        assert!(!ok);
        assert!(cancelled);
        assert!(t0.elapsed() < Duration::from_secs(5), "killed on cancel, not timeout");
        assert!(err.contains("cancelled by user"), "{err}");
    }

    #[test]
    fn launch_failure_is_a_failed_run_not_a_panic() {
        let (_out, err, ok, _c) = capture(&input("/nonexistent/bin_xyz", &[], 1));
        assert!(!ok);
        assert!(err.contains("failed to launch"), "{err}");
    }

    fn ndjson_shape() -> Shape {
        Shape {
            id: "candel".into(),
            extract: super::super::spec::Extract {
                id: "ndjson".into(),
                config: ron::Value::Unit,
            },
            min_hits: 1,
            emit: vec![super::super::spec::EmitRule::Entity {
                from: "url".into(),
                kind: "website".into(),
                rel: "uses_account".into(),
                outbound: true,
                attrs: Vec::new(),
                label: super::super::spec::LabelMode::Host,
            }],
        }
    }

    #[test]
    fn stream_emits_one_run_per_line_then_a_final_closer() {
        let mut inp = input(
            "sh",
            &[
                "-c",
                r#"printf '{"url":"https://a.test/x"}\n{"url":"https://b.test/y"}\n'"#,
            ],
            5,
        );
        inp.shapes = vec![ndjson_shape()];
        let (tx, rx) = std::sync::mpsc::channel();
        stream(&inp, &tx, &egui::Context::default());
        let runs: Vec<ModuleRun> = rx.try_iter().collect();
        assert_eq!(runs.len(), 3, "2 partials + 1 final");
        assert!(!runs[0].last && runs[0].facts.len() == 1);
        assert_eq!(runs[0].raw, "{\"url\":\"https://a.test/x\"}\n");
        assert!(!runs[1].last && runs[1].facts.len() == 1);
        let last = &runs[2];
        assert!(last.last && last.ok && last.facts.is_empty());
    }

    #[test]
    fn stream_cancel_stops_early_keeping_nothing_extra() {
        let mut inp = input("sh", &["-c", "printf '{\"url\":\"https://a.test/x\"}\\n'; sleep 30"], 25);
        inp.shapes = vec![ndjson_shape()];
        let flag = inp.cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            flag.store(true, Ordering::Relaxed);
        });
        let (tx, rx) = std::sync::mpsc::channel();
        let t0 = Instant::now();
        stream(&inp, &tx, &egui::Context::default());
        assert!(t0.elapsed() < Duration::from_secs(5), "cancel beat the 30s sleep");
        let runs: Vec<ModuleRun> = rx.try_iter().collect();
        let last = runs.last().expect("a final run");
        assert!(last.last && last.cancelled && !last.ok);
    }
}
