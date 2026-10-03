use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::Instant;

use petgraph::graph::EdgeIndex;

use crate::alibi::config::{ARCHIVE_WATCHDOG, CHECK_WATCHDOG, archive_key, search_env};
use crate::alibi::exec::{AlibiEnd, AlibiEvent, AlibiRequest, spawn_alibi};
use crate::alibi::locate::{Located, locate};
use crate::alibi::wire::is_web_url;
use crate::dlog_warn;
use crate::error::Error;
use crate::model::{ArchiveRecord, ArchiveStatus, ArchiveVia, RelationKind, SourceKind, merge_archive};

use super::app::RedstringApp;

#[derive(Clone, PartialEq, Debug)]
pub struct SourceKey {
    pub root: Option<PathBuf>,
    pub from: String,
    pub to: String,
    pub kind: RelationKind,
    pub url: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct EdgeBase {
    pub root: Option<PathBuf>,
    pub from: String,
    pub to: String,
    pub ends_sealed: bool,
}

pub struct AlibiJob {
    pub id: u64,
    pub key: SourceKey,
    pub via: ArchiveVia,
    pub started: Instant,
    pub cancel: Arc<AtomicBool>,
    pub worker: Option<JoinHandle<Result<(), Error>>>,
    pub unresolved: Vec<ArchiveRecord>,
    pub confirmed: usize,
    pub other: usize,
    pub dropped: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Done,
    Failed,
}

pub struct AlibiNote {
    pub key: SourceKey,
    pub text: String,
    pub tone: Tone,
}

pub struct AlibiState {
    pub tx: Sender<AlibiEvent>,
    pub rx: Receiver<AlibiEvent>,
    pub jobs: Vec<AlibiJob>,
    pub notes: Vec<AlibiNote>,
    pub next_job: u64,
}

pub enum AlibiAsk {
    Run { source: usize, via: ArchiveVia },
    Cancel { job: u64 },
}

pub enum Prepared {
    Ready(AlibiRequest),
    Refused,
}

pub enum JobAt {
    Running(u64),
    Idle,
}

impl AlibiState {
    const NOTES_KEPT: usize = 64;

    pub fn new() -> Result<Self, Error> {
        let (tx, rx) = channel();
        Ok(Self {
            tx,
            rx,
            jobs: Vec::new(),
            notes: Vec::new(),
            next_job: 1,
        })
    }

    pub fn job_for_url(&self, key: &SourceKey) -> Result<JobAt, Error> {
        for job in &self.jobs {
            if job.key.root == key.root && job.key.url == key.url {
                return Ok(JobAt::Running(job.id));
            }
        }
        Ok(JobAt::Idle)
    }

    pub fn archiving(&self) -> Result<bool, Error> {
        Ok(self.jobs.iter().any(|job| job.via == ArchiveVia::Archive))
    }

    fn note(&mut self, key: SourceKey, text: String, tone: Tone) {
        self.notes.retain(|kept| kept.key != key);
        self.notes.push(AlibiNote { key, text, tone });
        if self.notes.len() > Self::NOTES_KEPT {
            self.notes.remove(0);
        }
    }
}

impl RedstringApp {
    pub fn alibi_base(&self, eidx: EdgeIndex) -> Result<EdgeBase, Error> {
        let (a, b) = self.g.g().edge_endpoints(eidx).ok_or(gone("relation"))?;
        let ends = [a, b];
        let mut ends_sealed = false;
        for end in ends {
            let node = self.g.g().node_weight(end).ok_or(gone("entity"))?;
            ends_sealed |= node.payload().sealed;
        }
        Ok(EdgeBase {
            root: self.project.as_ref().map(|p| p.root.clone()),
            from: self.node_slug(a)?,
            to: self.node_slug(b)?,
            ends_sealed,
        })
    }

    pub fn alibi_ask(&mut self, ctx: &egui::Context, eidx: EdgeIndex, ask: AlibiAsk) -> Result<(), Error> {
        match ask {
            AlibiAsk::Run { source, via } => self.alibi_start(ctx, eidx, source, via),
            AlibiAsk::Cancel { job } => self.alibi_cancel(job),
        }
    }

    pub fn alibi_start(&mut self, ctx: &egui::Context, eidx: EdgeIndex, source: usize, via: ArchiveVia) -> Result<(), Error> {
        let Prepared::Ready(req) = self.alibi_prepare(eidx, source, via)? else {
            return Ok(());
        };
        let id = req.job;
        let worker = match spawn_alibi(req, self.alibi.tx.clone(), ctx.clone()) {
            Ok(worker) => worker,
            Err(err) => {
                self.alibi.jobs.retain(|job| job.id != id);
                return Err(err);
            }
        };
        let job = self.alibi.jobs.iter_mut().find(|job| job.id == id).ok_or(gone("alibi job"))?;
        job.worker = Some(worker);
        Ok(())
    }

    pub fn alibi_prepare(&mut self, eidx: EdgeIndex, source: usize, via: ArchiveVia) -> Result<Prepared, Error> {
        let base = self.alibi_base(eidx)?;
        let rel = self.g.g().edge_weight(eidx).ok_or(gone("relation"))?.payload();
        let src = rel.sources.get(source).ok_or(gone("source"))?;
        let key = SourceKey {
            root: base.root.clone(),
            from: base.from.clone(),
            to: base.to.clone(),
            kind: rel.kind,
            url: src.reference.trim().to_owned(),
        };
        let sealed = base.ends_sealed || rel.sealed || src.sealed;
        if src.kind != SourceKind::Url || !is_web_url(&key.url)? {
            return self.alibi_refuse(key, "Alibi works on URL sources with an http(s) address".to_owned());
        }
        if let JobAt::Running(job) = self.alibi.job_for_url(&key)? {
            return self.alibi_refuse(key, format!("alibi job #{job} for this URL is still running"));
        }
        if via == ArchiveVia::Archive && sealed {
            return self.alibi_refuse(key, "sealed material is never submitted to public archives — unseal it first".to_owned());
        }
        if via == ArchiveVia::Archive && self.alibi.archiving()? {
            return self.alibi_refuse(key, "one archive submission at a time — wait for the running one or cancel it".to_owned());
        }
        let bin = match locate(&search_env(&self.prefs)?)? {
            Located::Found(bin) => bin,
            Located::Missing(tried) => return self.alibi_refuse(key, missing(&tried)),
        };
        Ok(Prepared::Ready(self.alibi_enlist(key, via, bin)?))
    }

    pub fn alibi_cancel(&mut self, job: u64) -> Result<(), Error> {
        let found = self.alibi.jobs.iter().find(|kept| kept.id == job).ok_or(gone("alibi job"))?;
        found.cancel.store(true, Ordering::Relaxed);
        self.status = format!("alibi #{job}: cancelling — anything already submitted stays submitted");
        Ok(())
    }

    pub fn poll_alibi(&mut self) -> Result<(), Error> {
        let finished = self.alibi_finished()?;
        let events: Vec<AlibiEvent> = self.alibi.rx.try_iter().collect();
        for event in events {
            match event {
                AlibiEvent::Reading { job, record } => self.alibi_reading(job, record)?,
                AlibiEvent::Done { job, end } => self.alibi_done(job, end)?,
            }
        }
        for (job, outcome) in finished {
            match outcome {
                Ok(()) => {}
                Err(err) => {
                    self.alibi_close(job, format!("stopped: {err}"), Tone::Failed)?;
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    fn alibi_enlist(&mut self, key: SourceKey, via: ArchiveVia, bin: PathBuf) -> Result<AlibiRequest, Error> {
        let id = self.alibi.next_job;
        self.alibi.next_job += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        self.alibi.notes.retain(|kept| kept.key != key);
        self.status = format!("alibi #{id}: {} {}", verb(via), key.url);
        let url = key.url.clone();
        self.alibi.jobs.push(AlibiJob {
            id,
            key,
            via,
            started: Instant::now(),
            cancel: cancel.clone(),
            worker: None,
            unresolved: Vec::new(),
            confirmed: 0,
            other: 0,
            dropped: 0,
        });
        Ok(AlibiRequest {
            job: id,
            bin,
            url,
            via,
            key: archive_key(&self.prefs)?,
            watchdog: match via {
                ArchiveVia::Check => CHECK_WATCHDOG,
                ArchiveVia::Archive => ARCHIVE_WATCHDOG,
            },
            cancel,
        })
    }

    fn alibi_refuse(&mut self, key: SourceKey, why: String) -> Result<Prepared, Error> {
        self.status = format!("alibi: {why}");
        self.alibi.note(key, why, Tone::Failed);
        Ok(Prepared::Refused)
    }

    fn alibi_finished(&mut self) -> Result<Vec<(u64, Result<(), Error>)>, Error> {
        let mut out = Vec::new();
        for job in &mut self.alibi.jobs {
            let mut done = false;
            for worker in job.worker.iter() {
                done = worker.is_finished();
            }
            if !done {
                continue;
            }
            let worker = job.worker.take().ok_or(gone("alibi worker"))?;
            match worker.join() {
                Ok(outcome) => out.push((job.id, outcome)),
                Err(panicked) => panic!("alibi worker #{} panicked: {panicked:?}", job.id),
            }
        }
        Ok(out)
    }

    fn alibi_reading(&mut self, id: u64, record: ArchiveRecord) -> Result<(), Error> {
        let Some(at) = self.alibi.jobs.iter().position(|job| job.id == id) else {
            return Ok(());
        };
        if record.status == ArchiveStatus::Submitted {
            self.alibi.jobs[at].unresolved.push(record);
            return Ok(());
        }
        self.alibi.jobs[at].unresolved.retain(|sent| sent.witness != record.witness);
        let key = self.alibi.jobs[at].key.clone();
        let confirmed = record.status == ArchiveStatus::Confirmed;
        let attached = self.alibi_attach(&key, record)?;
        let job = &mut self.alibi.jobs[at];
        if attached == 0 {
            job.dropped += 1;
        } else if confirmed {
            job.confirmed += 1;
        } else {
            job.other += 1;
        }
        Ok(())
    }

    fn alibi_done(&mut self, id: u64, end: AlibiEnd) -> Result<(), Error> {
        let Some(job) = self.alibi.jobs.iter().find(|job| job.id == id) else {
            return Ok(());
        };
        let heard = job.confirmed + job.other + job.dropped + job.unresolved.len();
        let tally = format!("{} confirmed, {} without a snapshot", job.confirmed, job.other);
        let (text, tone) = match end {
            AlibiEnd::Exited { code: 0..=2, stderr } if heard == 0 => (format!("alibi printed no witness lines — {}", headline(&stderr)), Tone::Failed),
            AlibiEnd::Exited { code: 0..=2, stderr } => (format!("{tally}{}", warnings(&stderr)), Tone::Done),
            AlibiEnd::Exited { code, stderr } => (format!("alibi exited {code}: {}", headline(&stderr)), Tone::Failed),
            AlibiEnd::Signalled { stderr } => (format!("alibi was killed by a signal — {}", headline(&stderr)), Tone::Failed),
            AlibiEnd::Cancelled { stderr } => (format!("cancelled after {tally}{}", warnings(&stderr)), Tone::Failed),
            AlibiEnd::TimedOut { stderr } => (format!("no exit before the watchdog — killed after {tally}{}", warnings(&stderr)), Tone::Failed),
        };
        self.alibi_close(id, text, tone)
    }

    fn alibi_close(&mut self, id: u64, text: String, tone: Tone) -> Result<(), Error> {
        let at = self.alibi.jobs.iter().position(|job| job.id == id).ok_or(gone("alibi job"))?;
        let job = self.alibi.jobs.remove(at);
        let mut dropped = job.dropped;
        for sent in job.unresolved.iter().cloned() {
            if self.alibi_attach(&job.key, sent)? == 0 {
                dropped += 1;
            }
        }
        let mut text = text;
        if !job.unresolved.is_empty() {
            text.push_str(&format!(" · {} submitted with no verdict yet — Check again later", job.unresolved.len()));
        }
        if dropped > 0 {
            text.push_str(&format!(" · {dropped} result(s) not attached: the source, relation or case changed meanwhile"));
        }
        self.status = format!("alibi #{id} {} {}: {text}", verb(job.via), job.key.url);
        self.alibi.note(job.key, text, tone);
        Ok(())
    }

    fn alibi_attach(&mut self, key: &SourceKey, record: ArchiveRecord) -> Result<usize, Error> {
        let here = self.project.as_ref().map(|p| p.root.clone());
        let mut targets: Vec<EdgeIndex> = Vec::new();
        if here == key.root {
            for eidx in self.g.g().edge_indices() {
                let (a, b) = self.g.g().edge_endpoints(eidx).ok_or(gone("relation"))?;
                let kind = self.g.g().edge_weight(eidx).ok_or(gone("relation"))?.payload().kind;
                if kind == key.kind && self.node_slug(a)? == key.from && self.node_slug(b)? == key.to {
                    targets.push(eidx);
                }
            }
        }
        let mut attached = 0usize;
        for eidx in targets {
            let edge = self.g.edge_mut(eidx).ok_or(gone("relation"))?;
            for src in edge.payload_mut().sources.iter_mut() {
                if src.kind == SourceKind::Url && src.reference.trim() == key.url {
                    merge_archive(&mut src.archives, record.clone())?;
                    attached += 1;
                }
            }
        }
        if attached == 0 {
            dlog_warn!("alibi", "result not attached — {} {} {:?} {:?} for {}", record.witness, record.status.word()?, record.snapshot, record.reason, key.url);
        }
        Ok(attached)
    }
}

fn verb(via: ArchiveVia) -> &'static str {
    match via {
        ArchiveVia::Check => "checking archives for",
        ArchiveVia::Archive => "archiving",
    }
}

fn missing(tried: &[PathBuf]) -> String {
    let shown: Vec<String> = tried.iter().take(8).map(|p| p.display().to_string()).collect();
    format!(
        "alibi is not installed where redstring looks — set alibi_bin in prefs.ron or install it (cargo install --path …/alibi); looked in: {}{}",
        shown.join(", "),
        if tried.len() > shown.len() { ", …" } else { "" }
    )
}

fn headline(stderr: &str) -> String {
    let mut first = "no stderr";
    for line in stderr.lines().map(str::trim) {
        if line.starts_with("alibi:") {
            return line.to_owned();
        }
        if first == "no stderr" && !line.is_empty() {
            first = line;
        }
    }
    first.to_owned()
}

fn warnings(stderr: &str) -> String {
    let notes: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("alibi:") && line.contains('['))
        .collect();
    if notes.is_empty() {
        return String::new();
    }
    format!(" · {}", notes.join(" · "))
}

fn gone(what: &str) -> Error {
    Error::Alibi {
        what: format!("the {what} is gone"),
    }
}
