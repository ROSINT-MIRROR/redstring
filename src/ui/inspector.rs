use egui::RichText;
use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::error::Error;
use crate::model::{
    CATEGORY_META, EntityKind, KIND_META, MAX_CASE_NAME, RELATION_META, RelationKind, SOURCE_META,
    Source, ValueFormat, format_hint, pretty_key, slugify,
};

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

    fn multi_inspector(&mut self, ui: &mut egui::Ui, sel: Vec<NodeIndex>) -> Result<(), Error> {
        style::section(ui, egui_phosphor::regular::USERS_THREE, "selection")?;
        ui.label(RichText::new(format!("{} entities selected", sel.len())).strong());
        ui.label(
            RichText::new("Delete key asks to remove them — Enter confirms")
                .weak()
                .small(),
        );
        ui.add_space(style::GAP);
        if style::danger_btn(
            ui,
            egui_phosphor::regular::TRASH,
            &format!("delete {} entities", sel.len()),
            "remove the selected entities and their relations",
        )? {
            self.delete_confirm = Some(sel);
        }
        Ok(())
    }

    fn case_inspector(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        style::section(ui, egui_phosphor::regular::FOLDER, "case")?;
        let name = ui.add(
            egui::TextEdit::singleline(&mut self.case_name)
                .desired_width(f32::INFINITY)
                .char_limit(MAX_CASE_NAME),
        );
        if name.changed() {
            self.status = "case renamed — save to persist".into();
        }

        style::section(ui, egui_phosphor::regular::GRAPH, "graph settings")?;
        ui.checkbox(&mut self.size_by_degree, "scale nodes by connections")
            .on_hover_text("well-connected entities draw bigger — saved with the case");

        ui.add_space(style::GAP);
        ui.separator();
        ui.label(RichText::new("select an entity or relation").weak());
        ui.add_space(style::GAP);
        ui.label("• drag to move · scroll to zoom");
        ui.label("• right-drag entity → entity to link");
        ui.label("• red dashed link = unsourced");
        ui.label("• green link = has ≥1 source");
        Ok(())
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
            e.merge_template_attrs()?;
        }

        ui.horizontal(|ui| {
            ui.label("label");
            ui.add(
                egui::TextEdit::singleline(&mut e.label).desired_width(f32::INFINITY),
            );
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
        image_path_label(ui, &e.image);

        style::section(ui, egui_phosphor::regular::TAG, "attributes")?;
        let mut remove: Option<usize> = None;
        for (i, (k, v)) in e.attrs.iter_mut().enumerate() {
            ui.horizontal(|ui| -> Result<(), Error> {
                ui.add_sized(
                    egui::Vec2::new(90.0, ui.spacing().interact_size.y),
                    egui::Label::new(pretty_key(k)?).truncate(),
                );
                let w = (ui.available_width() - 34.0).max(40.0);
                ui.add(
                    egui::TextEdit::singleline(v)
                        .desired_width(w)
                        .hint_text("value"),
                );
                if style::danger_btn(ui, egui_phosphor::regular::TRASH, "", "remove attribute")? {
                    remove = Some(i);
                }
                Ok(())
            })
            .inner?;
        }
        remove_marked(&mut e.attrs, remove);
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
                if !e.attrs.iter().any(|(k, _)| *k == key) {
                    e.attrs.push((key, String::new()));
                }
                self.new_attr_key.clear();
            }
            Ok(())
        })
        .inner?;

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
            ui.colored_label(ALARM_RED, "⚠ UNSOURCED — unproven claim");
        } else {
            ui.colored_label(OK_GREEN, format!("✓ {} source(s)", r.sources.len()));
        }

        style::section(ui, egui_phosphor::regular::LINK, "sources")?;
        let mut remove: Option<usize> = None;
        for (i, src) in r.sources.iter_mut().enumerate() {
            ui.group(|ui| -> Result<(), Error> {
                egui::ComboBox::from_id_salt(("src_kind", i))
                    .selected_text(SOURCE_META[src.kind as usize].label)
                    .show_ui(ui, |ui| {
                        for m in &SOURCE_META {
                            ui.selectable_value(&mut src.kind, m.kind, m.label);
                        }
                    });
                ui.add(
                    egui::TextEdit::singleline(&mut src.reference)
                        .desired_width(f32::INFINITY)
                        .hint_text(SOURCE_META[src.kind as usize].hint),
                );
                ui.add(
                    egui::TextEdit::multiline(&mut src.excerpt)
                        .hint_text("excerpt / quote (optional)")
                        .desired_rows(2),
                );
                if style::danger_btn(
                    ui,
                    egui_phosphor::regular::TRASH,
                    "remove source",
                    "detach this source",
                )? {
                    remove = Some(i);
                }
                Ok(())
            })
            .inner?;
        }
        remove_marked(&mut r.sources, remove);
        Ok(())
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
