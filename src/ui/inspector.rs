use egui::RichText;
use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::error::Error;
use crate::model::{
    CATEGORY_META, EntityKind, KIND_META, MAX_CASE_NAME, RELATION_META, RelationKind, SOURCE_META,
    Source,
};

use super::app::{ALARM_RED, OK_GREEN, RedstringApp};

impl RedstringApp {
    pub fn inspector_strip(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        ui.vertical_centered(|ui| -> Result<(), Error> {
            if ui.button("⏴").on_hover_text("expand inspector").clicked() {
                self.prefs.inspector_collapsed = false;
                self.prefs.save()?;
            }
            Ok(())
        })
        .inner
    }

    pub fn inspector(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        ui.horizontal(|ui| -> Result<(), Error> {
            ui.heading("inspector");
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| -> Result<(), Error> {
                    if ui.button("⏵").on_hover_text("collapse inspector").clicked() {
                        self.prefs.inspector_collapsed = true;
                        self.prefs.save()?;
                    }
                    Ok(())
                },
            )
            .inner
        })
        .inner?;
        ui.separator();

        let sel_node = self.g.selected_nodes().first().copied();
        let Some(nidx) = sel_node else {
            let sel_edge = self.g.selected_edges().first().copied();
            let Some(eidx) = sel_edge else {
                ui.label(RichText::new("case").strong());
                let name = ui.add(
                    egui::TextEdit::singleline(&mut self.case_name).char_limit(MAX_CASE_NAME),
                );
                if name.changed() {
                    self.status = "case renamed — save to persist".into();
                }
                ui.separator();
                ui.label(RichText::new("select an entity or relation").weak());
                ui.add_space(8.0);
                ui.label("• drag to move · scroll to zoom");
                ui.label("• right-drag entity → entity to link");
                ui.label("• red dashed link = unsourced");
                ui.label("• green link = has ≥1 source");
                return Ok(());
            };
            return self.edge_inspector(ui, eidx);
        };
        self.node_inspector(ui, nidx)
    }

    fn node_inspector(&mut self, ui: &mut egui::Ui, nidx: NodeIndex) -> Result<(), Error> {
        let mut picked_image: Option<String> = None;
        let mut delete = false;

        let Some(node) = self.g.node_mut(nidx) else {
            return Ok(());
        };
        let e = node.payload_mut();

        let prev_kind = e.kind;
        kind_combo(ui, "insp_kind", &mut e.kind);
        if e.kind != prev_kind {
            merge_template_attrs(e);
        }

        ui.horizontal(|ui| {
            ui.label("label");
            ui.text_edit_singleline(&mut e.label);
        });

        ui.add_space(6.0);
        ui.label(RichText::new("image").strong());
        ui.horizontal(|ui| {
            if ui.button("🖼 image…").clicked()
                && let Some(p) = pick_image()
            {
                picked_image = Some(p);
            }
            if e.image.is_some()
                && ui
                    .button(egui_phosphor::regular::TRASH)
                    .on_hover_text("remove image")
                    .clicked()
            {
                e.image = None;
            }
        });
        image_path_label(ui, &e.image);

        ui.add_space(6.0);
        ui.label(RichText::new("attributes").strong());
        let mut remove: Option<usize> = None;
        for (i, (k, v)) in e.attrs.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(k).desired_width(80.0).hint_text("key"));
                ui.add(egui::TextEdit::singleline(v).hint_text("value"));
                if ui
                    .button(egui_phosphor::regular::TRASH)
                    .on_hover_text("remove attribute")
                    .clicked()
                {
                    remove = Some(i);
                }
            });
        }
        remove_marked(&mut e.attrs, remove);
        if ui.button("+ attr").clicked() {
            e.attrs.push((String::new(), String::new()));
        }

        ui.add_space(6.0);
        ui.label(RichText::new("note").strong());
        ui.text_edit_multiline(&mut e.note);

        ui.separator();
        if ui
            .button(format!("{} delete entity", egui_phosphor::regular::TRASH))
            .clicked()
        {
            delete = true;
        }

        self.apply_picked_image(ui.ctx(), nidx, picked_image)?;
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

    fn canonical_image(&mut self, nidx: NodeIndex, src: &str) -> Result<String, Error> {
        let slug = self.node_slug(nidx)?;
        let Some(project) = &self.project else {
            self.status = "image linked in place — save the case first to copy it in".into();
            return Ok(src.to_owned());
        };
        match crate::project::import_entity_image(project, &slug, std::path::Path::new(src)) {
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

    fn edge_inspector(&mut self, ui: &mut egui::Ui, eidx: EdgeIndex) -> Result<(), Error> {
        let (from_l, to_l) = self.edge_endpoints_labels(eidx)?;
        self.edge_form(ui, eidx, &from_l, &to_l);

        ui.separator();
        ui.label(RichText::new("attach source").strong());
        egui::ComboBox::from_id_salt("ns_kind")
            .selected_text(SOURCE_META[self.ns_kind as usize].label)
            .show_ui(ui, |ui| {
                for m in &SOURCE_META {
                    ui.selectable_value(&mut self.ns_kind, m.kind, m.label);
                }
            });
        let mut pick_source = false;
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.ns_ref)
                    .hint_text(SOURCE_META[self.ns_kind as usize].hint),
            );
            if SOURCE_META[self.ns_kind as usize].file_backed
                && ui
                    .button("📎")
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
        if ui.button("+ source").clicked() && !self.ns_ref.trim().is_empty() {
            self.attach_source(eidx);
        }

        ui.separator();
        if ui
            .button(format!("{} delete relation", egui_phosphor::regular::TRASH))
            .clicked()
        {
            self.g.remove_edge(eidx);
        }
        Ok(())
    }

    fn edge_form(&mut self, ui: &mut egui::Ui, eidx: EdgeIndex, from_l: &str, to_l: &str) {
        let Some(edge) = self.g.edge_mut(eidx) else { return };
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
            ui.colored_label(ALARM_RED, "⚠ UNSOURCED — unproven claim");
        } else {
            ui.colored_label(OK_GREEN, format!("✓ {} source(s)", r.sources.len()));
        }

        ui.separator();
        ui.label(RichText::new("sources").strong());
        let mut remove: Option<usize> = None;
        for (i, src) in r.sources.iter_mut().enumerate() {
            ui.group(|ui| {
                egui::ComboBox::from_id_salt(("src_kind", i))
                    .selected_text(SOURCE_META[src.kind as usize].label)
                    .show_ui(ui, |ui| {
                        for m in &SOURCE_META {
                            ui.selectable_value(&mut src.kind, m.kind, m.label);
                        }
                    });
                ui.add(
                    egui::TextEdit::singleline(&mut src.reference)
                        .hint_text(SOURCE_META[src.kind as usize].hint),
                );
                ui.add(
                    egui::TextEdit::multiline(&mut src.excerpt)
                        .hint_text("excerpt / quote (optional)")
                        .desired_rows(2),
                );
                if ui
                    .button(format!("{} remove source", egui_phosphor::regular::TRASH))
                    .clicked()
                {
                    remove = Some(i);
                }
            });
        }
        remove_marked(&mut r.sources, remove);
    }

    fn pick_source_file(&mut self) -> Result<(), Error> {
        let Some(p) = rfd::FileDialog::new().pick_file() else {
            return Ok(());
        };
        let Some(project) = &self.project else {
            self.status = "save the case first — evidence is copied into the case dir".into();
            return Ok(());
        };
        match crate::project::import_source_file(project, self.ns_kind, &p) {
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
        };
        let Some(edge) = self.g.edge_mut(eidx) else { return };
        edge.payload_mut().sources.push(src);
        self.ns_ref.clear();
        self.ns_excerpt.clear();
        self.status = "source attached".into();
    }
}

fn image_path_label(ui: &mut egui::Ui, image: &Option<String>) {
    let Some(p) = image else { return };
    ui.label(RichText::new(p.as_str()).weak().small());
}

fn remove_marked<T>(items: &mut Vec<T>, marked: Option<usize>) {
    let Some(i) = marked else { return };
    items.remove(i);
}


fn merge_template_attrs(e: &mut crate::model::Entity) {
    for key in KIND_META[e.kind as usize].attrs {
        if !e.attrs.iter().any(|(k, _)| k == key) {
            e.attrs.push(((*key).to_owned(), String::new()));
        }
    }
}

fn pick_image() -> Option<String> {
    let p = rfd::FileDialog::new()
        .add_filter("image", &["png", "jpg", "jpeg", "webp", "gif"])
        .pick_file()?;
    Some(p.to_string_lossy().into_owned())
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
