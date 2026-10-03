use std::io::Write;

use egui::RichText;
use sha2::{Digest, Sha256};

use crate::error::Error;
use crate::model::{
    DatePrecision, ObsValue, SOURCE_META, SOURCE_TIER_META, Source, SourceKind, SourceTier,
    fmt_civil_date, now_unix,
};
use crate::modules::{ModuleRun, ObsKind, parse_obs_value};

use super::archive::AlibiAsk;
use super::archive_view::{AlibiCtx, archive_rows, source_menu};
use super::style;

pub fn evidence_source(run: &ModuleRun) -> Result<Source, Error> {
    let Some(root) = run.project_root.as_ref() else {
        return Ok(note_source(run));
    };
    if run.evidence_rel.is_empty() {
        return Ok(note_source(run));
    }
    let abs = root.join(&run.evidence_rel);
    let Some(dir) = abs.parent() else {
        return Err(Error::NotAProject { path: abs.clone() });
    };
    std::fs::create_dir_all(dir).map_err(|source| Error::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&abs)
        .map_err(|source| Error::Io {
            path: abs.clone(),
            source,
        })?;
    file.write_all(run.raw.as_bytes()).map_err(|source| Error::Io {
        path: abs.clone(),
        source,
    })?;
    Ok(Source {
        kind: SourceKind::Document,
        reference: run.evidence_rel.clone(),
        excerpt: clip(&run.raw, 280),
        sha256: Some(sha256_hex(run.raw.as_bytes())),
        fetched_at: Some(now_unix()?),
        source_org: Some(run.module_id.clone()),
        ..Default::default()
    })
}

fn note_source(run: &ModuleRun) -> Source {
    Source {
        kind: SourceKind::Note,
        reference: format!(
            "{} on {} — raw output not persisted (case was unsaved)",
            run.module_id, run.target_label
        ),
        excerpt: clip(&run.raw, 600),
        sha256: Some(sha256_hex(run.raw.as_bytes())),
        source_org: Some(run.module_id.clone()),
        ..Default::default()
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for &byte in digest.iter() {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn fmt_unix_date(secs: i64) -> Result<String, Error> {
    fmt_civil_date(secs, DatePrecision::Day)
}

pub fn source_group(
    ui: &mut egui::Ui,
    i: usize,
    src: &mut Source,
    remove: &mut Option<usize>,
    alibi: &AlibiCtx,
    asks: &mut Vec<AlibiAsk>,
) -> Result<(), Error> {
    let framed = ui.scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
        ui.group(|ui| -> Result<egui::Response, Error> {
            egui::ComboBox::from_id_salt(("src_kind", i))
                .selected_text(SOURCE_META[src.kind as usize].label)
                .show_ui(ui, |ui| {
                    for m in &SOURCE_META {
                        ui.selectable_value(&mut src.kind, m.kind, m.label);
                    }
                });
            let reference = ui.add(
                egui::TextEdit::singleline(&mut src.reference)
                    .desired_width(f32::INFINITY)
                    .hint_text(SOURCE_META[src.kind as usize].hint),
            );
            ui.add(
                egui::TextEdit::multiline(&mut src.excerpt)
                    .hint_text("excerpt / quote (optional)")
                    .desired_rows(2),
            );
            source_provenance(ui, i, src)?;
            archive_rows(ui, src, alibi, asks)?;
            if style::danger_btn(
                ui,
                egui_phosphor::regular::TRASH,
                "remove source",
                "detach this source",
            )? {
                *remove = Some(i);
            }
            Ok(reference)
        })
        .inner
    });
    let reference = framed.inner?;
    source_menu(&framed.response, i, src, alibi, asks)?;
    source_menu(&reference, i, src, alibi, asks)
}

fn source_provenance(ui: &mut egui::Ui, i: usize, src: &mut Source) -> Result<(), Error> {
    ui.horizontal_wrapped(|ui| -> Result<(), Error> {
        tier_combo(ui, i, &mut src.tier)?;
        for org in src.source_org.iter() {
            ui.label(RichText::new(format!("· {org}")).weak().small())
                .on_hover_text("source organisation");
        }
        for ts in src.fetched_at.iter() {
            let when = fmt_unix_date(*ts)?;
            ui.label(RichText::new(format!("· fetched {when}")).weak().small())
                .on_hover_text("capture time — when the evidence was fetched");
        }
        Ok(())
    })
    .inner?;
    published_field(ui, i, src)?;
    edit_opt(ui, &mut src.sha256, "sha256 (content hash)", true)?;
    Ok(())
}

fn published_field(ui: &mut egui::Ui, i: usize, src: &mut Source) -> Result<(), Error> {
    let id = egui::Id::new(("src_published", i));
    let mut buf = published_text(src)?;
    let draft: Option<String> = ui.data(|d| d.get_temp::<String>(id));
    for typed in draft.iter() {
        buf = typed.clone();
    }
    let resp = ui
        .horizontal(|ui| {
            ui.label(RichText::new("published").weak().small())
                .on_hover_text("when the cited material was made public — not when you fetched it");
            ui.add(
                egui::TextEdit::singleline(&mut buf)
                    .desired_width(120.0)
                    .hint_text("e.g. 2019-09-30"),
            )
        })
        .inner;
    if resp.changed() {
        ui.data_mut(|d| d.insert_temp(id, buf.clone()));
    }
    if resp.lost_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
        commit_published(src, &buf)?;
    }
    Ok(())
}

fn published_text(src: &Source) -> Result<String, Error> {
    let Some(dv) = src.published else {
        return Ok(String::new());
    };
    ObsValue::Date(dv).text()
}

fn commit_published(src: &mut Source, typed: &str) -> Result<(), Error> {
    if typed.trim().is_empty() {
        src.published = None;
        return Ok(());
    }
    if let ObsValue::Date(dv) = parse_obs_value(ObsKind::Date, typed)? {
        src.published = Some(dv);
    }
    Ok(())
}

fn tier_combo(ui: &mut egui::Ui, id: usize, tier: &mut SourceTier) -> Result<(), Error> {
    let cur = &SOURCE_TIER_META[*tier as usize];
    egui::ComboBox::from_id_salt(("src_tier", id))
        .selected_text(RichText::new(cur.label).color(cur.color).small())
        .show_ui(ui, |ui| {
            for m in &SOURCE_TIER_META {
                ui.selectable_value(tier, m.tier, RichText::new(m.label).color(m.color));
            }
        })
        .response
        .on_hover_text("credibility tier");
    Ok(())
}

fn edit_opt(ui: &mut egui::Ui, field: &mut Option<String>, hint: &str, mono: bool) -> Result<(), Error> {
    let mut buf = String::new();
    for existing in field.iter() {
        buf = existing.clone();
    }
    let editor = egui::TextEdit::singleline(&mut buf)
        .desired_width(f32::INFINITY)
        .hint_text(hint);
    let editor = if mono {
        editor.font(egui::TextStyle::Monospace)
    } else {
        editor
    };
    if ui.add(editor).changed() {
        let trimmed = buf.trim().to_owned();
        *field = (!trimmed.is_empty()).then_some(trimmed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_with(raw: &str, root: Option<std::path::PathBuf>, rel: &str) -> ModuleRun {
        ModuleRun {
            run_id: 1,
            module_id: "whois".to_owned(),
            module_label: "whois".to_owned(),
            target_slug: "evil_example".to_owned(),
            target_label: "evil.example".to_owned(),
            project_root: root,
            shape_id: "icann".to_owned(),
            facts: Vec::new(),
            raw: raw.to_owned(),
            stderr_tail: String::new(),
            ok: true,
            cancelled: false,
            evidence_rel: rel.to_owned(),
            last: true,
        }
    }

    #[test]
    fn sha256_matches_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn evidence_source_hashes_and_stamps_the_run() {
        let dir = std::env::temp_dir().join(format!(
            "rs_ev_{}_{}",
            std::process::id(),
            now_unix().unwrap()
        ));
        let raw = "Registrar: Example Registrar\nCreated: 2001-01-01\n";
        let run = run_with(raw, Some(dir.clone()), "evidence/docs/whois-x.txt");
        let src = evidence_source(&run).unwrap();
        assert_eq!(src.kind, SourceKind::Document);
        assert_eq!(src.reference, "evidence/docs/whois-x.txt");
        assert_eq!(
            src.sha256.as_deref(),
            Some(sha256_hex(raw.as_bytes()).as_str()),
            "sha256 is the hash of the raw stdout"
        );
        assert_eq!(src.source_org.as_deref(), Some("whois"), "source_org is the module id");
        assert!(src.fetched_at.is_some_and(|t| t > 0), "capture time stamped");
        assert_eq!(
            src.tier,
            SourceTier::Unrated,
            "machine source starts unrated for the analyst to grade"
        );
        let written = std::fs::read_to_string(dir.join("evidence/docs/whois-x.txt")).unwrap();
        assert_eq!(written, raw, "raw stdout persisted verbatim as evidence");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unsaved_run_degrades_to_note_but_keeps_the_hash() {
        let raw = "some registry output";
        let run = run_with(raw, None, "");
        let src = evidence_source(&run).unwrap();
        assert_eq!(src.kind, SourceKind::Note);
        assert_eq!(src.sha256.as_deref(), Some(sha256_hex(raw.as_bytes()).as_str()));
        assert_eq!(src.source_org.as_deref(), Some("whois"));
        assert!(src.fetched_at.is_none(), "no persisted file, no capture stamp");
    }

    #[test]
    fn date_formats_epoch() {
        assert_eq!(fmt_unix_date(0).unwrap(), "1970-01-01");
        assert_eq!(fmt_unix_date(1_700_000_000).unwrap(), "2023-11-14");
    }
}
