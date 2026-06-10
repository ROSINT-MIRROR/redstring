use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crate::error::Error;

use super::emit::Fact;
use super::spec::Shape;


pub struct ModuleRun {
    pub module_id: String,
    pub module_label: String,
    pub target_slug: String,
    pub target_label: String,

    pub shape_id: String,

    pub facts: Vec<Fact>,

    pub raw: String,
    pub stderr_tail: String,
    pub ok: bool,
}


pub struct RunInput {
    pub module_id: String,
    pub module_label: String,
    pub bin: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub timeout_secs: u64,
    pub shapes: Vec<Shape>,
    pub target_slug: String,
    pub target_label: String,
}


pub fn spawn_module(
    input: RunInput,
    tx: Sender<ModuleRun>,
    ctx: egui::Context,
) -> Result<(), Error> {
    std::thread::spawn(move || {
        let run = execute(&input);
        if tx.send(run).is_err() {
            return;
        }
        ctx.request_repaint();
    });
    Ok(())
}

fn execute(input: &RunInput) -> ModuleRun {
    let (raw, stderr, ok) = capture(input);
    finish(input, raw, tail(&stderr), ok)
}

fn finish(input: &RunInput, raw: String, stderr_tail: String, ok: bool) -> ModuleRun {
    let mut shape_id = String::new();
    let mut facts: Vec<Fact> = Vec::new();
    let mut stderr_tail = stderr_tail;
    let mut ok = ok;
    if ok && !raw.is_empty() {
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
        module_id: input.module_id.clone(),
        module_label: input.module_label.clone(),
        target_slug: input.target_slug.clone(),
        target_label: input.target_label.clone(),
        shape_id,
        facts,
        raw,
        stderr_tail,
        ok,
    }
}

fn parse_facts(input: &RunInput, raw: &str) -> Result<(String, Vec<Fact>), Error> {
    let Some((shape, vars)) = super::extract::sniff(&input.shapes, raw)? else {
        return Ok((String::new(), Vec::new()));
    };
    let facts = super::emit::emit(shape, &vars)?;
    Ok((shape.id.clone(), facts))
}


fn capture(input: &RunInput) -> (String, String, bool) {
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
        Err(e) => return (String::new(), format!("failed to launch {}: {e}", input.bin.display()), false),
    };
    let Some(out_pipe) = child.stdout.take() else {
        drop(child.kill());
        return (String::new(), "no stdout pipe".to_owned(), false);
    };
    let Some(err_pipe) = child.stderr.take() else {
        drop(child.kill());
        return (String::new(), "no stderr pipe".to_owned(), false);
    };
    let out_h = std::thread::spawn(move || read_all(out_pipe));
    let err_h = std::thread::spawn(move || read_all(err_pipe));

    let deadline = Instant::now() + Duration::from_secs(input.timeout_secs.max(1));
    let mut timed_out = false;
    let ok = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) => {}
            Err(e) => {
                drop(child.kill());
                return (join_read(out_h), format!("wait failed: {e}"), false);
            }
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
    (stdout, stderr, ok)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn input(bin: &str, args: &[&str], timeout: u64) -> RunInput {
        RunInput {
            module_id: "t".into(),
            module_label: "t".into(),
            bin: PathBuf::from(bin),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            env: Vec::new(),
            timeout_secs: timeout,
            shapes: Vec::new(),
            target_slug: "x".into(),
            target_label: "x".into(),
        }
    }

    #[test]
    fn capture_collects_stdout_and_exit() {
        let (out, _err, ok) = capture(&input("sh", &["-c", "printf 'a: b\\n'"], 5));
        assert!(ok);
        assert_eq!(out, "a: b\n");

        let (_out, _err, ok) = capture(&input("sh", &["-c", "exit 3"], 5));
        assert!(!ok);
    }

    #[test]
    fn capture_kills_on_timeout() {
        let t0 = Instant::now();
        let (_out, err, ok) = capture(&input("sh", &["-c", "sleep 30"], 1));
        assert!(!ok);
        assert!(t0.elapsed() < Duration::from_secs(10), "killed well before 30s");
        assert!(err.contains("timed out"), "{err}");
    }

    #[test]
    fn launch_failure_is_a_failed_run_not_a_panic() {
        let (_out, err, ok) = capture(&input("/nonexistent/bin_xyz", &[], 1));
        assert!(!ok);
        assert!(err.contains("failed to launch"), "{err}");
    }
}
