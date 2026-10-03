use egui::{RichText, Sense};
use egui_phosphor::regular::{
    ARROW_SQUARE_OUT, CLOCK_COUNTER_CLOCKWISE, CROSSHAIR, EYE_SLASH, LINK, NOT_EQUALS, QUESTION,
    SEAL_CHECK,
};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::{
    Author, EntityKind, KIND_META, Origin, SOURCE_META, SOURCE_TIER_META, Source, SourceKind,
    SourceTier, Verification, status_color, status_label,
};

use super::super::app::RedstringApp;
use super::super::style;
use super::derive::{Digest, Slot};
use super::event::{EventKey, Footing, Lane, Record, TimelineEvent, Via};
use super::glyph::{Sample, sample};
use super::when::{When, Zone};

#[derive(Clone, PartialEq, Debug)]
pub enum CardAct {
    Select(EventKey),
    Show(Record),
    Open(Source),
    Nothing,
}

struct RecordView {
    origin: Origin,
    author: Author,
    sources: Vec<Source>,
    subject: Vec<(Record, String)>,
}

impl RedstringApp {
    pub(super) fn timeline_card(
        &self,
        ui: &mut egui::Ui,
        digest: &Digest,
    ) -> Result<CardAct, Error> {
        let Some(key) = self.timeline.selected else {
            return overview(ui, digest);
        };
        let i = match digest.slot(key)? {
            Slot::At(i) => i,
            Slot::Gone => return overview(ui, digest),
        };
        let ev = &digest.events[i];
        let rec = self.record_view(ev.key.record)?;
        let mut act = CardAct::Nothing;
        heading(ui, ev)?;
        standing(ui, ev, &rec)?;
        conflicts(ui, digest, ev, &mut act)?;
        across_time(ui, digest, ev, &mut act)?;
        sources(ui, &rec, ev, &mut act)?;
        links(ui, ev, &rec, &mut act)?;
        Ok(act)
    }

    fn record_view(&self, record: Record) -> Result<RecordView, Error> {
        match record {
            Record::Entity(n) => Err(Error::Timeline {
                what: format!("entity {} carries no dated record of its own", n.index()),
            }),
            Record::Obs(n, i) => {
                let node = self.g.node(n).ok_or(gone("entity"))?;
                let e = node.payload();
                let o = e.observations.get(i).ok_or(gone("observation"))?;
                Ok(RecordView {
                    origin: o.origin.clone(),
                    author: o.author.clone(),
                    sources: o.sources.clone(),
                    subject: vec![(Record::Entity(n), chip(e.kind, &e.label))],
                })
            }
            Record::Rel { edge, from, to } => {
                let r = self.g.edge(edge).ok_or(gone("relation"))?.payload();
                let from_chip = self.entity_chip(from)?;
                let to_chip = self.entity_chip(to)?;
                Ok(RecordView {
                    origin: r.origin.clone(),
                    author: r.author.clone(),
                    sources: r.sources.clone(),
                    subject: vec![
                        (Record::Entity(from), from_chip),
                        (Record::Entity(to), to_chip),
                    ],
                })
            }
        }
    }

    fn entity_chip(&self, n: NodeIndex) -> Result<String, Error> {
        let e = self.g.node(n).ok_or(gone("entity"))?.payload();
        Ok(chip(e.kind, &e.label))
    }
}

fn gone(what: &str) -> Error {
    Error::Timeline {
        what: format!("the {what} behind this mark is no longer in the case"),
    }
}

fn chip(kind: EntityKind, label: &str) -> String {
    for m in KIND_META.iter() {
        if m.kind == kind {
            return format!("{} {label}", m.icon);
        }
    }
    label.to_owned()
}

fn heading(ui: &mut egui::Ui, ev: &TimelineEvent) -> Result<(), Error> {
    ui.horizontal_wrapped(|ui| -> Result<(), Error> {
        let lane = ev.key.lane.label()?.to_uppercase();
        ui.label(RichText::new(lane).monospace().small().strong());
        ui.label(RichText::new(ev.key.lane.caption()?).weak().small());
        if ev.sealed {
            ui.label(RichText::new(format!("{EYE_SLASH} sealed")).weak().small())
                .on_hover_text("redacted from exports — the working copy keeps it");
        }
        Ok(())
    })
    .inner?;
    ui.label(RichText::new(format!("{}{}", ev.footing.tag()?, ev.title)).strong());
    ui.label(RichText::new(ev.when.label()?).size(17.0).strong());
    ui.label(RichText::new(ev.when.note()?).weak().small());
    if ev.read_from_text {
        let note = "read verbatim from a text value — no precision was added";
        ui.label(RichText::new(note).weak().small());
    }
    Ok(())
}

fn standing(ui: &mut egui::Ui, ev: &TimelineEvent, rec: &RecordView) -> Result<(), Error> {
    style::section(ui, SEAL_CHECK, "standing")?;
    let color = status_color(ev.status)?;
    ui.horizontal_wrapped(|ui| -> Result<(), Error> {
        sample(ui, Sample::Dot, ev.footing, color)?;
        ui.label(RichText::new(ev.footing.label()?).color(color).strong());
        let detail = format!(
            "· {} · {}% · {}",
            status_label(ev.status)?,
            ev.confidence,
            origin_text(&rec.origin)
        );
        ui.label(RichText::new(detail).small());
        Ok(())
    })
    .inner?;
    ui.label(RichText::new(ev.footing.hint()?).weak().small());
    let by = format!("recorded by {}", who(&rec.author));
    ui.label(RichText::new(by).weak().small());
    Ok(())
}

fn conflicts(
    ui: &mut egui::Ui,
    digest: &Digest,
    ev: &TimelineEvent,
    act: &mut CardAct,
) -> Result<(), Error> {
    if ev.conflicts.is_empty() {
        return Ok(());
    }
    style::section(ui, NOT_EQUALS, "conflicting accounts")?;
    let note = "the same fact, stated with dates that cannot both hold — both stay on the record";
    ui.label(RichText::new(note).weak().small());
    for &c in &ev.conflicts {
        let other = &digest.events[c];
        let text = format!(
            "{} · {} · {}",
            other.when.label()?,
            other.footing.label()?,
            status_label(other.status)?
        );
        if ui
            .button(RichText::new(text).small())
            .on_hover_text(other.title.clone())
            .clicked()
        {
            *act = CardAct::Select(other.key);
        }
    }
    Ok(())
}

fn across_time(
    ui: &mut egui::Ui,
    digest: &Digest,
    ev: &TimelineEvent,
    act: &mut CardAct,
) -> Result<(), Error> {
    style::section(ui, CLOCK_COUNTER_CLOCKWISE, "this record across time")?;
    let mine = digest.of_record(ev.key.record)?;
    let mut happened: Vec<i64> = Vec::new();
    for &j in &mine {
        if digest.events[j].key.lane == Lane::Event {
            happened.push(digest.events[j].span.start);
        }
    }
    egui::Grid::new(("tl_across", ev.key.lane.slot()?))
        .num_columns(3)
        .spacing([8.0, 3.0])
        .show(ui, |ui| -> Result<(), Error> {
            for lane in Lane::ALL {
                ui.label(RichText::new(lane.label()?).small().strong());
                let marks: Vec<usize> = mine
                    .iter()
                    .copied()
                    .filter(|j| digest.events[*j].key.lane == lane)
                    .collect();
                if marks.is_empty() {
                    ui.label(RichText::new(missing(lane)?).weak().small());
                    ui.end_row();
                    continue;
                }
                for (k, &j) in marks.iter().enumerate() {
                    if k > 0 {
                        ui.label("");
                    }
                    time_row(ui, &digest.events[j], ev.key, &happened, act)?;
                    ui.end_row();
                }
            }
            Ok(())
        })
        .inner
}

fn time_row(
    ui: &mut egui::Ui,
    other: &TimelineEvent,
    here: EventKey,
    happened: &[i64],
    act: &mut CardAct,
) -> Result<(), Error> {
    let is_here = other.key == here;
    let text = RichText::new(other.when.label()?).small();
    let resp = ui
        .selectable_label(is_here, text)
        .on_hover_text(other.title.clone());
    if resp.clicked() && !is_here {
        *act = CardAct::Select(other.key);
    }
    ui.label(RichText::new(gap_note(other, happened)?).weak().small())
        .on_hover_text("time since the event began, as far as its precision allows");
    Ok(())
}

fn gap_note(other: &TimelineEvent, happened: &[i64]) -> Result<String, Error> {
    if other.key.lane == Lane::Event {
        return Ok(String::new());
    }
    let Some(first) = happened.iter().min() else {
        return Ok(String::new());
    };
    gap(other.span.start - first)
}

fn missing(lane: Lane) -> Result<&'static str, Error> {
    Ok(match lane {
        Lane::Event => "not stated — no event date on this record",
        Lane::Published => "no publication date on its sources",
        Lane::Captured => "no capture time on its sources",
        Lane::Entered => "unknown — the record predates entry stamps",
    })
}

fn gap(secs: i64) -> Result<String, Error> {
    let days = secs.div_euclid(When::DAY);
    let sign = if days < 0 { "-" } else { "+" };
    let d = days.abs();
    Ok(if d < 60 {
        format!("{sign}{d} d")
    } else if d < 730 {
        format!("{sign}{} mo", d / 30)
    } else {
        format!("{sign}{} y {} mo", d / 365, (d % 365) / 30)
    })
}

fn sources(
    ui: &mut egui::Ui,
    rec: &RecordView,
    ev: &TimelineEvent,
    act: &mut CardAct,
) -> Result<(), Error> {
    style::section(ui, LINK, &format!("sources ({})", rec.sources.len()))?;
    if rec.sources.is_empty() {
        ui.colored_label(
            style::DANGER,
            "no source cited — an unproven assertion, not a fact",
        );
        return Ok(());
    }
    for (i, s) in rec.sources.iter().enumerate() {
        source_box(ui, s, ev.key.via == Via::Source(i), act)?;
    }
    Ok(())
}

fn source_box(
    ui: &mut egui::Ui,
    s: &Source,
    current: bool,
    act: &mut CardAct,
) -> Result<(), Error> {
    let stroke = if current {
        ui.visuals().selection.stroke
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke
    };
    egui::Frame::group(ui.style())
        .stroke(stroke)
        .show(ui, |ui| -> Result<(), Error> {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(kind_label(s.kind)).small().strong());
                if opens(s) {
                    let text = RichText::new(format!("{ARROW_SQUARE_OUT} {}", s.reference)).small();
                    let link = ui.add(egui::Label::new(text).truncate().sense(Sense::click()));
                    if link.on_hover_text("open the cited material").clicked() {
                        *act = CardAct::Open(s.clone());
                    }
                } else {
                    ui.add(egui::Label::new(RichText::new(&s.reference).small()).truncate());
                }
            });
            if !s.excerpt.trim().is_empty() {
                let quote = format!("“{}”", clip(s.excerpt.trim(), 240));
                ui.label(RichText::new(quote).italics().weak().small());
            }
            ui.label(RichText::new(provenance(s)?).weak().small());
            Ok(())
        })
        .inner
}

fn links(
    ui: &mut egui::Ui,
    ev: &TimelineEvent,
    rec: &RecordView,
    act: &mut CardAct,
) -> Result<(), Error> {
    style::section(ui, CROSSHAIR, "in the case")?;
    ui.horizontal_wrapped(|ui| {
        let what = match ev.key.record {
            Record::Rel { .. } => "show relation on board",
            Record::Obs(..) | Record::Entity(..) => "show fact on board",
        };
        if ui.button(format!("{CROSSHAIR} {what}")).clicked() {
            *act = CardAct::Show(ev.key.record);
        }
        for (record, label) in &rec.subject {
            if ui
                .button(label)
                .on_hover_text("select this entity on the board")
                .clicked()
            {
                *act = CardAct::Show(*record);
            }
        }
    });
    Ok(())
}

struct Legend;

impl Legend {
    const ROWS: [(Sample, Footing, Verification, &'static str); 9] = [
        (
            Sample::Dot,
            Footing::Corroborated,
            Verification::Corroborated,
            "corroborated — 2+ independent origins",
        ),
        (
            Sample::Dot,
            Footing::Confirmed,
            Verification::Confirmed,
            "confirmed from one primary source — not corroboration",
        ),
        (
            Sample::Dot,
            Footing::Sourced,
            Verification::AnalystAsserted,
            "sourced claim, not corroborated",
        ),
        (
            Sample::Dot,
            Footing::Unproven,
            Verification::Unreviewed,
            "? unproven — no source, or inferred",
        ),
        (
            Sample::Dot,
            Footing::Disputed,
            Verification::DeniedByTarget,
            "! disputed — denied, or no public record",
        ),
        (
            Sample::Dot,
            Footing::Rejected,
            Verification::Rejected,
            "× rejected — kept, never as fact",
        ),
        (
            Sample::Band,
            Footing::Sourced,
            Verification::AnalystAsserted,
            "band — date known only to month / year",
        ),
        (
            Sample::Bar,
            Footing::Corroborated,
            Verification::Corroborated,
            "bar — holds over a stated period",
        ),
        (
            Sample::OpenBar,
            Footing::Sourced,
            Verification::AnalystAsserted,
            "… — an end the record leaves open",
        ),
    ];
    const NOTES: [&'static str; 3] = [
        "≠ — two accounts of one fact that cannot both hold",
        "red string — one record pinned across its times",
        "published / captured marks take the source tier colour",
    ];
}

fn overview(ui: &mut egui::Ui, digest: &Digest) -> Result<CardAct, Error> {
    let mut act = CardAct::Nothing;
    let hint = "pick a mark to see its record, its sources and its other times";
    ui.label(RichText::new(hint).weak());
    style::section(ui, QUESTION, "how to read it")?;
    for (shape, footing, status, text) in Legend::ROWS {
        legend(ui, shape, footing, status, text)?;
    }
    for n in Legend::NOTES {
        ui.label(RichText::new(n).small());
    }
    style::section(ui, CLOCK_COUNTER_CLOCKWISE, "not on the timeline")?;
    if digest.unplaced.is_empty() {
        ui.label(
            RichText::new("every record with a date is placed")
                .weak()
                .small(),
        );
    } else {
        let why = "no date on the record or its sources — nothing was guessed";
        ui.label(RichText::new(why).weak().small());
        for u in &digest.unplaced {
            if ui
                .small_button(&u.title)
                .on_hover_text("select it on the board")
                .clicked()
            {
                act = CardAct::Show(u.record);
            }
        }
    }
    if digest.unknown_entry > 0 {
        let n = digest.unknown_entry;
        let text = format!("{n} placed record(s) carry no entry stamp — left off ENTERED");
        ui.label(RichText::new(text).weak().small());
    }
    Ok(act)
}

fn legend(
    ui: &mut egui::Ui,
    shape: Sample,
    footing: Footing,
    status: Verification,
    text: &str,
) -> Result<(), Error> {
    let color = status_color(status)?;
    ui.horizontal(|ui| -> Result<(), Error> {
        sample(ui, shape, footing, color)?;
        ui.label(RichText::new(text).small());
        Ok(())
    })
    .inner
}

fn provenance(s: &Source) -> Result<String, Error> {
    let mut parts = vec![tier_label(s.tier).to_owned()];
    for org in s.source_org.iter() {
        parts.push(org.clone());
    }
    for p in s.published.iter() {
        parts.push(format!("published {}", When::Date(*p).label()?));
    }
    for f in s.fetched_at.iter() {
        let at = When::Instant {
            secs: *f,
            zone: Zone::Utc,
        }
        .label()?;
        parts.push(format!("captured {at}"));
    }
    for h in s.sha256.iter() {
        parts.push(format!("sha256 {}…", clip(h, 12)));
    }
    Ok(parts.join(" · "))
}

fn opens(s: &Source) -> bool {
    let r = s.reference.trim();
    if r.starts_with("https://") || r.starts_with("http://") {
        return true;
    }
    for m in SOURCE_META.iter() {
        if m.kind == s.kind {
            return m.file_backed && !r.is_empty();
        }
    }
    false
}

fn kind_label(kind: SourceKind) -> &'static str {
    for m in SOURCE_META.iter() {
        if m.kind == kind {
            return m.label;
        }
    }
    "source"
}

fn tier_label(tier: SourceTier) -> &'static str {
    for m in SOURCE_TIER_META.iter() {
        if m.tier == tier {
            return m.label;
        }
    }
    "unrated"
}

fn origin_text(o: &Origin) -> String {
    match o {
        Origin::Manual => "entered by hand".to_owned(),
        Origin::Lookup(m) => format!("via lookup {m}"),
        Origin::Import(s) => format!("imported from {s}"),
        Origin::Inference => "inferred — a hypothesis".to_owned(),
    }
}

fn who(a: &Author) -> String {
    if a.name.trim().is_empty() {
        return "an unnamed analyst".to_owned();
    }
    a.name.clone()
}

fn clip(s: &str, n: usize) -> String {
    let clipped: String = s.chars().take(n).collect();
    if clipped.len() < s.len() {
        return format!("{clipped}…");
    }
    clipped
}
