use egui::{RichText, Vec2};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::{
    Author, CATEGORY_META, DATE_PRECISION_META, DateVal, EntityKind, KIND_META, MAX_CASE_NAME,
    ObsValue, Observation, Origin, VERIFICATION_META, ValueFormat, Verification, format_hint,
    pretty_key, slugify, status_color, status_label,
};
use crate::modules::{ObsKind, parse_obs_value};

use super::app::{ALARM_RED, OK_GREEN, RedstringApp};
use super::style;

impl RedstringApp {
    pub fn inspector_strip(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        ui.vertical_centered(|ui| -> Result<(), Error> {
            if style::square_btn(ui, egui_phosphor::regular::CARET_LEFT, "expand inspector")? {
                self.prefs.inspector_collapsed = false;
                self.prefs.save()?;
            }
            Ok(())
        })
        .inner
    }

    pub fn inspector(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        ui.horizontal(|ui| -> Result<(), Error> {

            if style::square_btn(ui, egui_phosphor::regular::CARET_RIGHT, "collapse inspector")? {
                self.prefs.inspector_collapsed = true;
                self.prefs.save()?;
                self.clear_selection()?;
            }
            ui.heading("inspector");
            Ok(())
        })
        .inner?;
        ui.separator();

        let sel = self.g.selected_nodes().to_vec();
        if sel.len() > 1 {
            return self.multi_inspector(ui, sel);
        }
        let Some(nidx) = sel.first().copied() else {
            let sel_edge = self.g.selected_edges().first().copied();
            let Some(eidx) = sel_edge else {
                return self.case_inspector(ui);
            };
            return self.edge_inspector(ui, eidx);
        };
        self.node_inspector(ui, nidx)
    }

    fn case_inspector(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        style::section(ui, egui_phosphor::regular::FOLDER, "case")?;
        let name = ui.add(
            egui::TextEdit::singleline(&mut self.case_name)
                .desired_width(f32::INFINITY)
                .char_limit(MAX_CASE_NAME),
        );
        if name.changed() {
            self.status = "case renamed".into();
        }

        style::section(ui, egui_phosphor::regular::GRAPH, "graph settings")?;
        ui.checkbox(&mut self.size_by_degree, "scale nodes by connections")
            .on_hover_text("well-connected entities draw bigger — saved with the case");

        ui.add_space(style::GAP);
        ui.separator();
        ui.label(RichText::new("select an entity or relation").weak());
        ui.add_space(style::GAP);
        ui.label("• left-drag empty = pan · right-drag empty = select");
        ui.label("• left-drag node = move · left-click = select");
        ui.label("• right-drag node → node = link · right-click node = menu");
        ui.label("• middle-drag = pan · wheel = zoom · double-click = fold");
        ui.label("• dashed link = unsourced · plain link = sourced claim");
        ui.label("• red yarn = confirmed from a primary source, or corroborated");
        Ok(())
    }

    fn node_inspector(&mut self, ui: &mut egui::Ui, nidx: NodeIndex) -> Result<(), Error> {
        let mut picked_image: Option<String> = None;
        let mut relabel: Option<String> = None;
        let mut delete = false;
        let author = self.author.clone();

        let Some(node) = self.g.node_mut(nidx) else {
            return Ok(());
        };
        let e = node.payload_mut();

        let kind_before = e.kind;
        kind_combo(ui, "insp_kind", &mut e.kind);
        if e.kind != kind_before {
            crate::model::retain_declared_facets(&mut e.facets, e.kind)?;
        }
        super::facet_ui::facet_rows(ui, e.kind, &mut e.facets)?;

        ui.horizontal(|ui| {
            ui.label("label");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut e.label).desired_width(f32::INFINITY),
            );
            if resp.lost_focus() {
                relabel = Some(e.label.clone());
            }
        });
        ui.horizontal(|ui| {
            ui.label("value")
                .on_hover_text("the machine-readable thing lookups run against — label is free prose");
            ui.add(
                egui::TextEdit::singleline(&mut e.value)
                    .desired_width(f32::INFINITY)
                    .hint_text(e.label.clone()),
            );
        });
        let vf = KIND_META[e.kind as usize].value_format;
        if vf != ValueFormat::Free {
            let hint = format_hint(vf)?;
            if e.module_value()?.is_some() {
                ui.colored_label(
                    OK_GREEN,
                    RichText::new(format!("{} valid {hint}", egui_phosphor::regular::CHECK)).small(),
                );
            } else {
                ui.colored_label(
                    ALARM_RED,
                    RichText::new(format!(
                        "{} needs a valid {hint} — lookups disabled",
                        egui_phosphor::regular::X
                    ))
                    .small(),
                );
            }
        }

        ui.add_space(style::GAP);
        ui.checkbox(
            &mut e.sealed,
            format!(
                "{} sealed — redact from exports & generated files (local copy keeps the data)",
                egui_phosphor::regular::EYE_SLASH
            ),
        )
        .on_hover_text(
            "redact this entity from the Export bundle (.redstring.zip) and the generated artifacts (graph.md / case.json): label, value, note and observations are masked and its id anonymized. the on-disk case directory keeps the full data as your working copy.",
        );

        style::section(ui, egui_phosphor::regular::IMAGE, "image")?;
        ui.horizontal(|ui| -> Result<(), Error> {
            if style::action_btn(ui, egui_phosphor::regular::IMAGE, "pick image…")?
                && let Some(p) = pick_image()
            {
                picked_image = Some(p);
            }
            if e.image.is_some()
                && style::danger_btn(ui, egui_phosphor::regular::TRASH, "", "remove image")?
            {
                e.image = None;
            }
            Ok(())
        })
        .inner?;
        image_path_label(ui, &e.image, self.project.as_ref())?;
        super::flag_ui::flag_row(ui, nidx.index(), &mut e.flag, e.image.is_some())?;

        style::section(ui, egui_phosphor::regular::TAG, "attributes")?;
        let mut remove: Option<usize> = None;
        for (i, o) in e.observations.iter_mut().enumerate() {
            observation_row(ui, i, o, &mut remove)?;
        }
        remove_marked(&mut e.observations, remove);

        ui.horizontal_wrapped(|ui| -> Result<(), Error> {
            for key in KIND_META[e.kind as usize].attrs {
                if e.observations.iter().any(|o| o.name == *key) {
                    continue;
                }
                if ui
                    .small_button(format!("+ {}", pretty_key(key)?))
                    .on_hover_text("add this attribute as an observation")
                    .clicked()
                {
                    e.observations.push(Observation::stamp(
                        (*key).to_owned(),
                        ObsValue::default(),
                        Origin::Manual,
                        Verification::Unreviewed,
                        author.clone(),
                    )?);
                }
            }
            Ok(())
        })
        .inner?;

        ui.horizontal(|ui| -> Result<(), Error> {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_attr_key)
                    .desired_width(150.0)
                    .hint_text("new attribute name"),
            );
            if style::action_btn(ui, egui_phosphor::regular::PLUS, "add attribute")?
                && !self.new_attr_key.trim().is_empty()
            {
                let key = slugify(&self.new_attr_key)?;
                if !e.observations.iter().any(|o| o.name == key) {
                    e.observations.push(Observation::stamp(
                        key,
                        ObsValue::default(),
                        Origin::Manual,
                        Verification::AnalystAsserted,
                        author.clone(),
                    )?);
                }
                self.new_attr_key.clear();
            }
            Ok(())
        })
        .inner?;

        if style::action_btn(ui, egui_phosphor::regular::CALENDAR, "add date-range attribute")?
            && !self.new_attr_key.trim().is_empty()
        {
            let key = slugify(&self.new_attr_key)?;
            if !e.observations.iter().any(|o| o.name == key) {
                e.observations.push(Observation::stamp(
                    key,
                    ObsValue::DateRange { start: None, end: None },
                    Origin::Manual,
                    Verification::AnalystAsserted,
                    author.clone(),
                )?);
            }
            self.new_attr_key.clear();
        }

        style::section(ui, egui_phosphor::regular::NOTE_PENCIL, "note")?;
        ui.text_edit_multiline(&mut e.note);
        let kind_now = e.kind;
        let slug_now = e.slug.clone();
        let module_ok = e.module_value()?.is_some();

        style::section(ui, egui_phosphor::regular::PLUGS, "lookups")?;
        let run_module = self.lookup_buttons(ui, kind_now, module_ok, &slug_now)?;

        ui.add_space(style::GAP);
        ui.separator();
        if style::danger_btn(
            ui,
            egui_phosphor::regular::TRASH,
            "delete entity",
            "remove this entity and its relations",
        )? {
            delete = true;
        }

        self.apply_picked_image(ui.ctx(), nidx, picked_image)?;
        self.apply_relabel(nidx, relabel)?;
        self.start_module_request(ui, nidx, run_module)?;
        if delete {
            self.g.remove_node(nidx);
        }
        Ok(())
    }

    fn apply_picked_image(
        &mut self,
        ctx: &egui::Context,
        nidx: NodeIndex,
        picked: Option<String>,
    ) -> Result<(), Error> {
        let Some(src) = picked else {
            return Ok(());
        };
        let resolved = self.canonical_image(nidx, &src)?;
        crate::shapes::invalidate_texture(ctx, &resolved)?;
        let Some(node) = self.g.node_mut(nidx) else {
            return Ok(());
        };
        node.payload_mut().image = Some(resolved);
        Ok(())
    }

    fn apply_relabel(&mut self, nidx: NodeIndex, relabel: Option<String>) -> Result<(), Error> {
        let Some(label) = relabel else {
            return Ok(());
        };
        self.apply_label(nidx, &label)
    }

    fn canonical_image(&mut self, nidx: NodeIndex, src: &str) -> Result<String, Error> {
        let slug = self.node_slug(nidx)?;
        let Some(project) = self.project.clone() else {
            self.status = "image linked in place — save the case first to copy it in".into();
            return Ok(src.to_owned());
        };
        let imported = self.self_write(&project.root, |_app| {
            crate::project::import_entity_image(&project, &slug, std::path::Path::new(src))
        });
        match imported {
            Ok(abs) => {
                let abs = abs.display().to_string();
                self.status = format!("image copied → {}", project.rel_or_self(&abs)?);
                Ok(abs)
            }
            Err(err) => {
                self.status = format!("image import failed — linked in place: {err}");
                Ok(src.to_owned())
            }
        }
    }

}

fn image_path_label(
    ui: &mut egui::Ui,
    image: &Option<String>,
    project: Option<&crate::project::Project>,
) -> Result<(), Error> {
    let Some(p) = image else { return Ok(()) };
    let Some(proj) = project else {
        ui.add(egui::Label::new(RichText::new(p.as_str()).weak().small()).truncate());
        return Ok(());
    };
    let shown = proj.rel_or_self(p)?;
    ui.add(egui::Label::new(RichText::new(shown).weak().small()).truncate());
    Ok(())
}

fn remove_marked<T>(items: &mut Vec<T>, marked: Option<usize>) {
    let Some(i) = marked else { return };
    items.remove(i);
}

fn observation_row(
    ui: &mut egui::Ui,
    i: usize,
    o: &mut Observation,
    remove: &mut Option<usize>,
) -> Result<(), Error> {
    let range_summary = if matches!(o.value, ObsValue::DateRange { .. }) {
        o.value.text()?
    } else {
        String::new()
    };
    ui.horizontal(|ui| -> Result<(), Error> {
        ui.add_sized(
            Vec2::new(90.0, ui.spacing().interact_size.y),
            egui::Label::new(pretty_key(&o.name)?).truncate(),
        );
        let w = (ui.available_width() - 34.0).max(40.0);
        match &mut o.value {
            ObsValue::Text(s) => {
                ui.add(egui::TextEdit::singleline(s).desired_width(w).hint_text("value"));
            }
            ObsValue::DateRange { .. } => {
                ui.add_sized(
                    Vec2::new(w, ui.spacing().interact_size.y),
                    egui::Label::new(RichText::new(range_summary.as_str()).small()).truncate(),
                );
            }
            other => {
                let is_date = matches!(other, ObsValue::Date(_));
                let mut buf = other.text()?;
                let tw = if is_date { (w - 82.0).max(40.0) } else { w };
                let resp =
                    ui.add(egui::TextEdit::singleline(&mut buf).desired_width(tw).hint_text("value"));
                if resp.changed() {
                    *other = if is_date {
                        parse_obs_value(ObsKind::Date, &buf)?
                    } else {
                        ObsValue::Text(buf)
                    };
                }
                date_precision_combo(ui, i, other);
            }
        }
        if style::danger_btn(ui, egui_phosphor::regular::TRASH, "", "remove attribute")? {
            *remove = Some(i);
        }
        Ok(())
    })
    .inner?;

    if let ObsValue::DateRange { start, end } = &mut o.value {
        ui.horizontal(|ui| -> Result<(), Error> {
            ui.add_sized(
                Vec2::new(40.0, ui.spacing().interact_size.y),
                egui::Label::new(RichText::new("from").weak().small()),
            );
            opt_date_field(ui, ("obs_from", i), start, 100.0)?;
            Ok(())
        })
        .inner?;
        ui.horizontal(|ui| -> Result<(), Error> {
            ui.add_sized(
                Vec2::new(40.0, ui.spacing().interact_size.y),
                egui::Label::new(RichText::new("to").weak().small()),
            );
            opt_date_field(ui, ("obs_to", i), end, 100.0)?;
            Ok(())
        })
        .inner?;
    }

    ui.horizontal_wrapped(|ui| -> Result<(), Error> {
        verification_combo(ui, i, &mut o.status)?;
        ui.add(egui::DragValue::new(&mut o.confidence).range(0..=100u8).suffix("%"))
            .on_hover_text("confidence");
        ui.label(RichText::new(origin_label(&o.origin)).weak().small())
            .on_hover_text(format!("who: {}", who(&o.author)));
        if !o.sources.is_empty() {
            ui.label(
                RichText::new(format!("{} {}", egui_phosphor::regular::LINK, o.sources.len()))
                    .weak()
                    .small(),
            )
            .on_hover_text("cited sources");
        }
        Ok(())
    })
    .inner?;
    Ok(())
}


fn verification_combo(ui: &mut egui::Ui, id: usize, status: &mut Verification) -> Result<(), Error> {
    let cur_color = status_color(*status)?;
    let cur_label = status_label(*status)?;
    egui::ComboBox::from_id_salt(("obs_status", id))
        .selected_text(RichText::new(cur_label).color(cur_color).small())
        .show_ui(ui, |ui| {
            for m in &VERIFICATION_META {
                ui.selectable_value(status, m.status, RichText::new(m.label).color(m.color));
            }
        });
    Ok(())
}


fn date_precision_combo(ui: &mut egui::Ui, id: usize, v: &mut ObsValue) {
    let ObsValue::Date(dv) = v else { return };
    egui::ComboBox::from_id_salt(("obs_prec", id))
        .selected_text(RichText::new(DATE_PRECISION_META[dv.precision as usize].label).small())
        .show_ui(ui, |ui| {
            for m in &DATE_PRECISION_META {
                ui.selectable_value(&mut dv.precision, m.precision, m.label);
            }
        });
}


fn date_slot_text(slot: Option<DateVal>) -> Result<String, Error> {
    let Some(dv) = slot else {
        return Ok(String::new());
    };
    ObsValue::Date(dv).text()
}

pub(super) fn opt_date_field(
    ui: &mut egui::Ui,
    salt: (&'static str, usize),
    slot: &mut Option<DateVal>,
    width: f32,
) -> Result<(), Error> {
    let mut buf = date_slot_text(*slot)?;
    let resp = ui.add(
        egui::TextEdit::singleline(&mut buf)
            .desired_width(width)
            .hint_text("e.g. 2012"),
    );
    if resp.changed() {
        if buf.trim().is_empty() {
            *slot = None;
        } else if let ObsValue::Date(dv) = parse_obs_value(ObsKind::Date, &buf)? {
            *slot = Some(dv);
        }
    }
    let Some(dv) = slot.as_mut() else {
        return Ok(());
    };
    date_precision_combo_dv(ui, salt, dv);
    Ok(())
}


fn date_precision_combo_dv(ui: &mut egui::Ui, salt: (&'static str, usize), dv: &mut DateVal) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(RichText::new(DATE_PRECISION_META[dv.precision as usize].label).small())
        .show_ui(ui, |ui| {
            for m in &DATE_PRECISION_META {
                ui.selectable_value(&mut dv.precision, m.precision, m.label);
            }
        });
}


fn origin_label(o: &Origin) -> String {
    match o {
        Origin::Manual => "manual".to_owned(),
        Origin::Lookup(m) => format!("via {m}"),
        Origin::Import(s) => format!("import {s}"),
        Origin::Inference => "inferred".to_owned(),
    }
}


fn who(a: &Author) -> String {
    if a.name.trim().is_empty() {
        return "unknown".to_owned();
    }
    a.name.clone()
}

fn pick_image() -> Option<String> {
    let p = rfd::FileDialog::new()
        .add_filter("image", &["png", "jpg", "jpeg", "webp", "gif"])
        .pick_file()?;
    Some(p.to_string_lossy().into_owned())
}

fn kind_combo(ui: &mut egui::Ui, id: &str, kind: &mut EntityKind) {
    let cur = &KIND_META[*kind as usize];
    egui::ComboBox::from_id_salt(id)
        .selected_text(format!("{} {}", cur.icon, cur.label))
        .show_ui(ui, |ui| {
            for cat in &CATEGORY_META {
                ui.add_space(2.0);
                ui.label(RichText::new(cat.label).weak().small());
                for m in KIND_META.iter().filter(|m| m.category == cat.category) {
                    ui.selectable_value(kind, m.kind, format!("{} {}", m.icon, m.label));
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_row_fits_default_inspector_width() {
        let width = 248.0;
        let ctx = egui::Context::default();
        let mut o = Observation::stamp(
            "registrar_abuse_contact_email".to_owned(),
            ObsValue::Text("abuse@example.com".to_owned()),
            Origin::Lookup("whois".to_owned()),
            Verification::MachineSuggested,
            Author::default(),
        )
        .expect("stamp observation");
        let mut remove = None;
        let mut input = egui::RawInput::default();
        input.screen_rect =
            Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(width + 80.0, 400.0)));
        let out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(ui, |ui| {
                ui.allocate_ui(Vec2::new(width, 300.0), |ui| {
                    observation_row(ui, 0, &mut o, &mut remove).expect("row renders");
                });
            });
        });
        let mut right = f32::MIN;
        for cs in &out.shapes {
            let b = cs.shape.visual_bounding_rect();
            if b.is_finite() {
                right = right.max(b.right());
            }
        }
        assert!(right > 0.0 && right <= width + 3.0, "obs row clips at {right} > {width}");
    }
}
