use egui::{RichText, Vec2};
use petgraph::graph::EdgeIndex;

use crate::error::Error;
use crate::model::{
    Footing, RELATION_META, Relation, RelationKind, SOURCE_META, Source, VERIFICATION_META,
    Verification, status_color, status_label,
};
use crate::project::import_source_file;

use super::app::{ALARM_RED, OK_GREEN, RedstringApp};
use super::evidence;
use super::inspector::opt_date_field;
use super::style;

impl RedstringApp {
    pub(super) fn edge_inspector(
        &mut self,
        ui: &mut egui::Ui,
        eidx: EdgeIndex,
    ) -> Result<(), Error> {
        let (from_l, to_l) = self.edge_endpoints_labels(eidx)?;
        self.edge_form(ui, eidx, &from_l, &to_l)?;

        style::section(ui, egui_phosphor::regular::PAPERCLIP, "attach source")?;
        egui::ComboBox::from_id_salt("ns_kind")
            .selected_text(SOURCE_META[self.ns_kind as usize].label)
            .show_ui(ui, |ui| {
                for m in &SOURCE_META {
                    ui.selectable_value(&mut self.ns_kind, m.kind, m.label);
                }
            });
        let mut pick_source = false;
        ui.horizontal(|ui| {
            let fb = SOURCE_META[self.ns_kind as usize].file_backed;
            let w = if fb {
                (ui.available_width() - 34.0).max(40.0)
            } else {
                f32::INFINITY
            };
            ui.add(
                egui::TextEdit::singleline(&mut self.ns_ref)
                    .desired_width(w)
                    .hint_text(SOURCE_META[self.ns_kind as usize].hint),
            );
            if SOURCE_META[self.ns_kind as usize].file_backed
                && ui
                    .button(egui_phosphor::regular::PAPERCLIP)
                    .on_hover_text("copy a file into the case and cite it")
                    .clicked()
            {
                pick_source = true;
            }
        });
        if pick_source {
            self.pick_source_file()?;
        }
        ui.add(
            egui::TextEdit::multiline(&mut self.ns_excerpt)
                .hint_text("excerpt / quote (optional)")
                .desired_rows(2),
        );
        if style::action_btn(ui, egui_phosphor::regular::PLUS, "add source")?
            && !self.ns_ref.trim().is_empty()
        {
            self.attach_source(eidx);
        }

        ui.add_space(style::GAP);
        ui.separator();
        if style::danger_btn(
            ui,
            egui_phosphor::regular::TRASH,
            "delete relation",
            "remove this relation and its sources",
        )? {
            self.g.remove_edge(eidx);
        }
        Ok(())
    }

    fn edge_form(
        &mut self,
        ui: &mut egui::Ui,
        eidx: EdgeIndex,
        from_l: &str,
        to_l: &str,
    ) -> Result<(), Error> {
        let Some(edge) = self.g.edge_mut(eidx) else {
            return Ok(());
        };
        let r = edge.payload_mut();
        ui.label(
            RichText::new(format!(
                "{from_l}  —{}→  {to_l}",
                RELATION_META[r.kind as usize].label
            ))
            .strong(),
        );

        relation_kind_combo(ui, "insp_rel_kind", &mut r.kind);

        if r.sources.is_empty() {
            let g = egui_phosphor::regular::WARNING_CIRCLE;
            ui.colored_label(ALARM_RED, format!("{g} UNSOURCED — unproven claim"));
        } else {
            let g = egui_phosphor::regular::CHECK;
            ui.colored_label(OK_GREEN, format!("{g} {} source(s)", r.sources.len()));
        }

        standing(ui, eidx.index(), r)?;

        ui.add_space(style::GAP);
        ui.checkbox(
            &mut r.sealed,
            format!(
                "{} sealed — drop this relation from exports & generated files (local copy keeps it)",
                egui_phosphor::regular::EYE_SLASH
            ),
        )
        .on_hover_text(
            "redact this relation from the Export bundle (.redstring.zip) and the generated artifacts (graph.md / case.json): the edge, its verb and its sources are dropped. any edge touching a sealed entity is dropped automatically. the on-disk case directory keeps the full data as your working copy.",
        );

        style::section(ui, egui_phosphor::regular::CALENDAR, "valid during")?;
        ui.horizontal(|ui| -> Result<(), Error> {
            ui.add_sized(
                Vec2::new(40.0, ui.spacing().interact_size.y),
                egui::Label::new(RichText::new("from").weak().small()),
            );
            opt_date_field(
                ui,
                ("edge_valid_from", eidx.index()),
                &mut r.valid_from,
                110.0,
            )?;
            Ok(())
        })
        .inner?;
        ui.horizontal(|ui| -> Result<(), Error> {
            ui.add_sized(
                Vec2::new(40.0, ui.spacing().interact_size.y),
                egui::Label::new(RichText::new("to").weak().small()),
            );
            opt_date_field(ui, ("edge_valid_to", eidx.index()), &mut r.valid_to, 110.0)?;
            Ok(())
        })
        .inner?;
        ui.label(
            RichText::new("leave a side empty for an open-ended window")
                .weak()
                .small(),
        );

        style::section(ui, egui_phosphor::regular::LINK, "sources")?;
        let mut remove: Option<usize> = None;
        for (i, src) in r.sources.iter_mut().enumerate() {
            evidence::source_group(ui, i, src, &mut remove)?;
        }
        let Some(i) = remove else {
            return Ok(());
        };
        r.sources.remove(i);
        Ok(())
    }

    fn pick_source_file(&mut self) -> Result<(), Error> {
        let Some(p) = rfd::FileDialog::new().pick_file() else {
            return Ok(());
        };
        let Some(project) = self.project.clone() else {
            self.status = "save the case first — evidence is copied into the case dir".into();
            return Ok(());
        };
        let kind = self.ns_kind;
        let imported =
            self.self_write(&project.root, |_app| import_source_file(&project, kind, &p));
        match imported {
            Ok(rel) => {
                self.ns_ref = rel;
                self.status = "file copied into the case — add the source".into();
            }
            Err(err) => self.status = format!("evidence import failed: {err}"),
        }
        Ok(())
    }

    fn attach_source(&mut self, eidx: EdgeIndex) {
        let src = Source {
            kind: self.ns_kind,
            reference: self.ns_ref.trim().to_owned(),
            excerpt: self.ns_excerpt.trim().to_owned(),
            ..Default::default()
        };
        let Some(edge) = self.g.edge_mut(eidx) else {
            return;
        };
        edge.payload_mut().sources.push(src);
        self.ns_ref.clear();
        self.ns_excerpt.clear();
        self.status = "source attached".into();
    }
}

fn standing(ui: &mut egui::Ui, id: usize, r: &mut Relation) -> Result<(), Error> {
    style::section(ui, egui_phosphor::regular::SEAL_CHECK, "standing")?;
    status_combo(ui, id, r)?;
    let footing = Footing::from(&*r);
    let line = if Footing::STRONG.contains(&footing) {
        format!(
            "{} {} — red string",
            egui_phosphor::regular::CHECK_CIRCLE,
            footing.label()?
        )
    } else {
        format!(
            "{} {}",
            egui_phosphor::regular::CIRCLE_DASHED,
            footing.label()?
        )
    };
    ui.label(RichText::new(line).strong());
    ui.label(RichText::new(footing.hint()?).weak().small());
    if !Footing::would_hold(r.status, &r.origin, &r.sources)? {
        ui.colored_label(
            ALARM_RED,
            format!(
                "{} marked {}, but that needs {}",
                egui_phosphor::regular::SEAL_WARNING,
                status_label(r.status)?,
                Footing::needs(r.status)?
            ),
        );
    }
    Ok(())
}

fn status_combo(ui: &mut egui::Ui, id: usize, r: &mut Relation) -> Result<(), Error> {
    let mut rows: Vec<(Verification, bool, &'static str)> =
        Vec::with_capacity(VERIFICATION_META.len());
    for m in &VERIFICATION_META {
        let holds = Footing::would_hold(m.status, &r.origin, &r.sources)?;
        rows.push((m.status, holds, Footing::needs(m.status)?));
    }
    let mut picked = r.status;
    egui::ComboBox::from_id_salt(("rel_status", id))
        .selected_text(RichText::new(status_label(r.status)?).color(status_color(r.status)?))
        .show_ui(ui, |ui| -> Result<(), Error> {
            for (status, holds, needs) in &rows {
                let text = RichText::new(status_label(*status)?).color(status_color(*status)?);
                ui.add_enabled_ui(*holds || *status == picked, |ui| {
                    ui.selectable_value(&mut picked, *status, text)
                })
                .inner
                .on_disabled_hover_text(format!("needs {needs}"));
            }
            Ok(())
        })
        .inner
        .transpose()?;
    r.status = picked;
    Ok(())
}

fn relation_kind_combo(ui: &mut egui::Ui, id: &str, kind: &mut RelationKind) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(RELATION_META[*kind as usize].label)
        .show_ui(ui, |ui| {
            let mut last_cat = "";
            for m in &RELATION_META {
                if m.category != last_cat {
                    last_cat = m.category;
                    ui.label(RichText::new(last_cat).weak().small());
                }
                ui.selectable_value(kind, m.kind, m.label);
            }
        });
}
