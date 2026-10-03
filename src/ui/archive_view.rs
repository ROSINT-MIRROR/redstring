use std::time::Duration;

use egui::{Response, RichText, Ui};
use egui_phosphor::regular::{ARCHIVE, CHECK_CIRCLE, CLOCK_COUNTER_CLOCKWISE, MAGNIFYING_GLASS, WARNING_CIRCLE, X};

use crate::alibi::wire::is_web_url;
use crate::error::Error;
use crate::model::{ArchiveRecord, ArchiveStatus, ArchiveVia, RelationKind, Source, SourceKind, civil_ymd};

use super::app::{ALARM_RED, OK_GREEN};
use super::archive::{AlibiAsk, AlibiJob, AlibiNote, AlibiState, EdgeBase, JobAt, SourceKey, Tone};

pub struct AlibiCtx<'a> {
    pub state: &'a AlibiState,
    pub base: EdgeBase,
    pub kind: RelationKind,
    pub sealed: bool,
}

impl AlibiCtx<'_> {
    const CHECK_HINT: &'static str = "read-only: asks web.archive.org, archive.today and ghostarchive.org which snapshots of this exact URL they already hold. Submits nothing.";
    const ARCHIVE_HINT: &'static str = "submits this exact URL to web.archive.org, archive.today and ghostarchive.org, then polls up to 5 min for public confirmation. Public and rate-limited per IP; the archives see your IP. Wayback needs the Web Archive key in settings.";
    const CANCEL_HINT: &'static str = "stop waiting and kill alibi — what alibi already submitted stays submitted";

    fn key_for(&self, src: &Source) -> SourceKey {
        SourceKey {
            root: self.base.root.clone(),
            from: self.base.from.clone(),
            to: self.base.to.clone(),
            kind: self.kind,
            url: src.reference.trim().to_owned(),
        }
    }
}

pub fn source_menu(resp: &Response, i: usize, src: &Source, cx: &AlibiCtx, asks: &mut Vec<AlibiAsk>) -> Result<(), Error> {
    if src.kind != SourceKind::Url {
        return Ok(());
    }
    let mut outcome: Result<(), Error> = Ok(());
    resp.context_menu(|ui| outcome = menu_items(ui, i, src, cx, asks));
    outcome
}

pub fn archive_rows(ui: &mut Ui, src: &Source, cx: &AlibiCtx, asks: &mut Vec<AlibiAsk>) -> Result<(), Error> {
    if src.kind != SourceKind::Url {
        return Ok(());
    }
    let key = cx.key_for(src);
    for job in cx.state.jobs.iter().filter(|job| job.key == key) {
        job_row(ui, job, asks)?;
    }
    for rec in &src.archives {
        record_row(ui, rec, &key.url)?;
    }
    for note in cx.state.notes.iter().filter(|note| note.key == key) {
        note_row(ui, note)?;
    }
    Ok(())
}

fn menu_items(ui: &mut Ui, i: usize, src: &Source, cx: &AlibiCtx, asks: &mut Vec<AlibiAsk>) -> Result<(), Error> {
    let key = cx.key_for(src);
    let web = is_web_url(&key.url)?;
    let running = cx.state.job_for_url(&key)?;
    let idle = matches!(running, JobAt::Idle);
    let blocked = if !web {
        "Alibi needs an http(s) URL in this source"
    } else if !idle {
        "an alibi job for this URL is already running"
    } else {
        ""
    };
    ui.label(RichText::new("alibi · third-party archives").weak().small());
    let check = ui.add_enabled(blocked.is_empty(), egui::Button::new(format!("{MAGNIFYING_GLASS} Check existing archives")));
    let check = check.on_hover_text(AlibiCtx::CHECK_HINT).on_disabled_hover_text(blocked);
    if check.clicked() {
        asks.push(AlibiAsk::Run { source: i, via: ArchiveVia::Check });
        ui.close();
    }
    let archive_blocked = if !blocked.is_empty() {
        blocked
    } else if cx.sealed || src.sealed {
        "sealed material is never submitted to public archives — unseal it first"
    } else if cx.state.archiving()? {
        "one archive submission at a time — wait for the running one or cancel it"
    } else {
        ""
    };
    let archive = ui.add_enabled(archive_blocked.is_empty(), egui::Button::new(format!("{ARCHIVE} Archive now with Alibi")));
    let archive = archive.on_hover_text(AlibiCtx::ARCHIVE_HINT).on_disabled_hover_text(archive_blocked);
    if archive.clicked() {
        asks.push(AlibiAsk::Run { source: i, via: ArchiveVia::Archive });
        ui.close();
    }
    if let JobAt::Running(job) = running {
        ui.separator();
        if ui.button(format!("{X} Cancel alibi job #{job}")).on_hover_text(AlibiCtx::CANCEL_HINT).clicked() {
            asks.push(AlibiAsk::Cancel { job });
            ui.close();
        }
    }
    Ok(())
}

fn job_row(ui: &mut Ui, job: &AlibiJob, asks: &mut Vec<AlibiAsk>) -> Result<(), Error> {
    let doing = match job.via {
        ArchiveVia::Check => "checking archives",
        ArchiveVia::Archive => "archiving",
    };
    let mut text = format!("alibi #{} {doing} · {}s", job.id, job.started.elapsed().as_secs());
    if !job.unresolved.is_empty() {
        let sent: Vec<&str> = job.unresolved.iter().map(|rec| rec.witness.as_str()).collect();
        text.push_str(&format!(" · submitted to {}", sent.join(", ")));
    }
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::Spinner::new().size(12.0));
        ui.label(RichText::new(text).weak().small());
        if ui.small_button(X).on_hover_text(AlibiCtx::CANCEL_HINT).clicked() {
            asks.push(AlibiAsk::Cancel { job: job.id });
        }
    });
    ui.ctx().request_repaint_after(Duration::from_millis(500));
    Ok(())
}

fn record_row(ui: &mut Ui, rec: &ArchiveRecord, url: &str) -> Result<(), Error> {
    let via = match rec.via {
        ArchiveVia::Check => "check",
        ArchiveVia::Archive => "archive",
    };
    let mut hover = format!(
        "{} said {} ({})\nasked about {}\nseen by redstring {} via {via}",
        rec.witness,
        rec.status.word()?,
        rec.reason,
        rec.url,
        fmt_utc(rec.checked_at)?
    );
    for detail in rec.detail.iter() {
        hover.push_str(&format!("\n{detail}"));
    }
    let earlier = rec.url != url;
    for snapshot in rec.snapshot.iter().filter(|_snap| rec.status == ArchiveStatus::Confirmed) {
        let label = format!("{} · {}", rec.witness, captured(rec)?);
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(OK_GREEN, RichText::new(CHECK_CIRCLE).small());
            ui.hyperlink_to(RichText::new(label).small(), snapshot)
                .on_hover_text(format!("{snapshot}\n{hover}"));
            if earlier {
                ui.label(RichText::new("earlier URL").weak().small()).on_hover_text(&rec.url);
            }
        });
    }
    if rec.status == ArchiveStatus::Confirmed {
        return Ok(());
    }
    let icon = match rec.status {
        ArchiveStatus::Errored => WARNING_CIRCLE,
        ArchiveStatus::Confirmed | ArchiveStatus::Pending | ArchiveStatus::Absent | ArchiveStatus::Skipped | ArchiveStatus::Submitted => CLOCK_COUNTER_CLOCKWISE,
    };
    let mut text = format!("{icon} {} · {} ({}) · {}", rec.witness, rec.status.word()?, rec.reason, fmt_utc(rec.checked_at)?);
    if earlier {
        text.push_str(" · earlier URL");
    }
    ui.label(RichText::new(text).weak().small()).on_hover_text(hover);
    Ok(())
}

fn note_row(ui: &mut Ui, note: &AlibiNote) -> Result<(), Error> {
    let (icon, color) = match note.tone {
        Tone::Done => (CHECK_CIRCLE, OK_GREEN),
        Tone::Failed => (WARNING_CIRCLE, ALARM_RED),
    };
    ui.colored_label(color, RichText::new(format!("{icon} {}", note.text)).small());
    Ok(())
}

fn captured(rec: &ArchiveRecord) -> Result<String, Error> {
    let Some(secs) = rec.archived_at else {
        return Ok("capture time not published".to_owned());
    };
    Ok(format!("captured {}", fmt_utc(secs)?))
}

fn fmt_utc(secs: i64) -> Result<String, Error> {
    let (year, month, day) = civil_ymd(secs)?;
    let in_day = secs.rem_euclid(86_400);
    Ok(format!("{year:04}-{month:02}-{day:02} {:02}:{:02} UTC", in_day / 3_600, in_day % 3_600 / 60))
}
