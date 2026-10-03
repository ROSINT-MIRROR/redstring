use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{Builder, JoinHandle};
use std::time::{Duration, Instant};

use crate::error::Error;
use crate::model::{ArchiveRecord, ArchiveVia, now_unix};

use super::config::{ARCHIVE_DEADLINE_SECS, ArchiveKey, KEY_ENV, PUMP_EVERY};
use super::wire::{Asked, parse_line};

pub struct AlibiRequest {
    pub job: u64,
    pub bin: PathBuf,
    pub url: String,
    pub via: ArchiveVia,
    pub key: ArchiveKey,
    pub watchdog: Duration,
    pub cancel: Arc<AtomicBool>,
}

pub enum AlibiEvent {
    Reading { job: u64, record: ArchiveRecord },
    Done { job: u64, end: AlibiEnd },
}

pub enum AlibiEnd {
    Exited { code: i32, stderr: String },
    Signalled { stderr: String },
    Cancelled { stderr: String },
    TimedOut { stderr: String },
}

enum Stop {
    Exited(ExitStatus),
    Cancelled,
    TimedOut,
}

pub fn alibi_args(via: ArchiveVia, url: &str) -> Result<Vec<String>, Error> {
    Ok(match via {
        ArchiveVia::Check => vec!["--check".to_owned(), "--json".to_owned(), url.to_owned()],
        ArchiveVia::Archive => vec![
            "--json".to_owned(),
            "--timeout".to_owned(),
            ARCHIVE_DEADLINE_SECS.to_string(),
            url.to_owned(),
        ],
    })
}

pub fn spawn_alibi(
    req: AlibiRequest,
    tx: Sender<AlibiEvent>,
    ctx: egui::Context,
) -> Result<JoinHandle<Result<(), Error>>, Error> {
    let name = format!("alibi-{}", req.job);
    Builder::new()
        .name(name)
        .spawn(move || run_alibi(&req, &tx, &ctx))
        .map_err(|source| thread_error("worker", source))
}

pub fn run_alibi(req: &AlibiRequest, tx: &Sender<AlibiEvent>, ctx: &egui::Context) -> Result<(), Error> {
    let mut child = command(req)?.spawn().map_err(|source| Error::Alibi {
        what: format!("cannot launch {}: {source}", req.bin.display()),
    })?;
    let stdout = child.stdout.take().ok_or(Error::Alibi { what: "child has no stdout pipe".to_owned() })?;
    let stderr = child.stderr.take().ok_or(Error::Alibi { what: "child has no stderr pipe".to_owned() })?;
    let (line_tx, lines) = channel::<String>();
    let reader = Builder::new()
        .name(format!("alibi-{}-out", req.job))
        .spawn(move || read_lines(stdout, &line_tx))
        .map_err(|source| thread_error("stdout reader", source))?;
    let drain = Builder::new()
        .name(format!("alibi-{}-err", req.job))
        .spawn(move || read_text(stderr))
        .map_err(|source| thread_error("stderr reader", source))?;
    let stop = match pump(req, &mut child, &lines, &reader, tx, ctx) {
        Ok(stop) => stop,
        Err(err) => {
            halt(&mut child)?;
            return Err(err);
        }
    };
    for line in lines.iter() {
        deliver(req, &line, tx, ctx)?;
    }
    joined(reader)?;
    let end = ending(stop, joined(drain)?)?;
    send(tx, ctx, AlibiEvent::Done { job: req.job, end })
}

fn command(req: &AlibiRequest) -> Result<Command, Error> {
    let mut cmd = Command::new(&req.bin);
    cmd.args(alibi_args(req.via, &req.url)?)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    if req.via == ArchiveVia::Check {
        cmd.env_remove(KEY_ENV);
    }
    if req.via == ArchiveVia::Archive
        && let ArchiveKey::FromPrefs(key) = &req.key
    {
        cmd.env(KEY_ENV, key);
    }
    Ok(cmd)
}

fn pump(
    req: &AlibiRequest,
    child: &mut Child,
    lines: &Receiver<String>,
    reader: &JoinHandle<Result<(), Error>>,
    tx: &Sender<AlibiEvent>,
    ctx: &egui::Context,
) -> Result<Stop, Error> {
    let deadline = Instant::now() + req.watchdog;
    loop {
        let stdout_closed = reader.is_finished();
        for line in lines.try_iter() {
            deliver(req, &line, tx, ctx)?;
        }
        if stdout_closed {
            let waited = child.try_wait().map_err(|source| child_error("wait for", source))?;
            for status in waited.iter() {
                return Ok(Stop::Exited(*status));
            }
        }
        if req.cancel.load(Ordering::Relaxed) {
            halt(child)?;
            return Ok(Stop::Cancelled);
        }
        if Instant::now() >= deadline {
            halt(child)?;
            return Ok(Stop::TimedOut);
        }
        std::thread::sleep(PUMP_EVERY);
    }
}

fn deliver(req: &AlibiRequest, line: &str, tx: &Sender<AlibiEvent>, ctx: &egui::Context) -> Result<(), Error> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let asked = Asked {
        url: &req.url,
        via: req.via,
        now: now_unix()?,
    };
    let record = parse_line(trimmed, &asked)?;
    send(tx, ctx, AlibiEvent::Reading { job: req.job, record })
}

fn send(tx: &Sender<AlibiEvent>, ctx: &egui::Context, event: AlibiEvent) -> Result<(), Error> {
    tx.send(event).map_err(|lost| Error::Alibi {
        what: format!("redstring stopped listening: {lost}"),
    })?;
    ctx.request_repaint();
    Ok(())
}

fn ending(stop: Stop, stderr: String) -> Result<AlibiEnd, Error> {
    Ok(match stop {
        Stop::Exited(status) => {
            let Some(code) = status.code() else {
                return Ok(AlibiEnd::Signalled { stderr });
            };
            AlibiEnd::Exited { code, stderr }
        }
        Stop::Cancelled => AlibiEnd::Cancelled { stderr },
        Stop::TimedOut => AlibiEnd::TimedOut { stderr },
    })
}

fn halt(child: &mut Child) -> Result<(), Error> {
    let group = i32::try_from(child.id()).map_err(|source| Error::Alibi {
        what: format!("alibi pid does not fit a process group id: {source}"),
    })?;
    let signalled = unsafe { libc::kill(-group, libc::SIGKILL) };
    if signalled != 0 {
        let failure = std::io::Error::last_os_error();
        let Some(code) = failure.raw_os_error() else {
            return Err(child_error("signal", failure));
        };
        if code != libc::ESRCH {
            return Err(child_error("signal", failure));
        }
    }
    child.wait().map_err(|source| child_error("reap", source))?;
    Ok(())
}

fn read_lines(out: ChildStdout, tx: &Sender<String>) -> Result<(), Error> {
    for line in BufReader::new(out).lines() {
        let line = line.map_err(|source| child_error("read stdout of", source))?;
        tx.send(line).map_err(|lost| Error::Alibi {
            what: format!("stdout reader lost its worker: {lost}"),
        })?;
    }
    Ok(())
}

fn read_text(err: ChildStderr) -> Result<String, Error> {
    let mut bytes = Vec::new();
    BufReader::new(err)
        .read_to_end(&mut bytes)
        .map_err(|source| child_error("read stderr of", source))?;
    String::from_utf8(bytes).map_err(|bad| Error::Alibi {
        what: format!("stderr is not utf-8: {bad}"),
    })
}

fn joined<T>(handle: JoinHandle<Result<T, Error>>) -> Result<T, Error> {
    match handle.join() {
        Ok(outcome) => outcome,
        Err(panicked) => panic!("an alibi reader thread panicked: {panicked:?}"),
    }
}

fn child_error(what: &str, source: std::io::Error) -> Error {
    Error::Alibi {
        what: format!("cannot {what} the alibi process: {source}"),
    }
}

fn thread_error(what: &str, source: std::io::Error) -> Error {
    Error::Alibi {
        what: format!("cannot start the {what} thread: {source}"),
    }
}
