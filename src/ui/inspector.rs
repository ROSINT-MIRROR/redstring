//! Right panel: the inspector for whatever is selected — entity form with
//! attributes/image/note, or relation form with the source list. This is
//! where evidence gets attached.

use egui::RichText;
use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::model::{Category, EntityKind, RelationKind, Source, SourceKind};

use super::app::{ALARM_RED, OK_GREEN, RedstringApp};

impl RedstringApp {
    pub(super) fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("inspector");
        ui.separator();

        let sel_node = self.g.selected_nodes().first().copied();
        let Some(nidx) = sel_node else {
            let sel_edge = self.g.selected_edges().first().copied();
            let Some(eidx) = sel_edge else {
                ui.label(RichText::new("select an entity or relation").weak());
                ui.add_space(8.0);
                ui.label("• drag to move · scroll to zoom");
                ui.label("• right-drag entity → entity to link");
                ui.label("• red dashed link = unsourced");
                ui.label("• green link = has ≥1 source");
                return;
            };
            self.edge_inspector(ui, eidx);
            return;
        };
        self.node_inspector(ui, nidx);
    }

    fn node_inspector(&mut self, ui: &mut egui::Ui, nidx: NodeIndex) {
        // slug edits collect into a local and apply after the borrow ends,
        // because dedupe needs to read the whole graph
        let mut slug_edit: Option<String> = None;
        let mut delete = false;

        let Some(node) = self.g.node_mut(nidx) else { return };
        let e = node.payload_mut();
        ui.label(RichText::new(format!("{} {}", e.kind.icon(), e.kind.label())).strong());
        ui.label(RichText::new(e.kind.category().label()).weak().small());

        kind_combo(ui, "insp_kind", &mut e.kind);

        ui.horizontal(|ui| {
            ui.label("label");
            ui.text_edit_singleline(&mut e.label);
        });

        let mut slug = e.slug.clone();
        ui.horizontal(|ui| {
            ui.label("slug");
            if ui.text_edit_singleline(&mut slug).changed() {
                slug_edit = Some(slug.clone());
            }
        });
        ui.label(
            RichText::new("the handle relations & external edits reference")
                .weak()
                .small(),
        );

        ui.add_space(6.0);
        ui.label(RichText::new("image").strong());
        ui.horizontal(|ui| {
            if ui.button("🖼 image…").clicked()
                && let Some(p) = pick_image()
            {
                // re-picking a path must reflect the file as it is now
                crate::shapes::invalidate_texture(ui.ctx(), &p);
                e.image = Some(p);
            }
            if e.image.is_some() && ui.button("✕").clicked() {
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
                if ui.button("✕").clicked() {
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
        if ui.button("🗑 delete entity").clicked() {
            delete = true;
        }

        self.apply_slug_edit(nidx, slug_edit);
        if delete {
            self.g.remove_node(nidx);
        }
    }

    /// Apply an edited slug when it is non-empty and unique in the graph.
    fn apply_slug_edit(&mut self, nidx: NodeIndex, edit: Option<String>) {
        let Some(new_slug) = edit else { return };
        let clean: String = new_slug
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        if clean.is_empty() || self.slug_taken_by_other(&clean, nidx) {
            return;
        }
        let Some(node) = self.g.node_mut(nidx) else { return };
        node.payload_mut().slug = clean;
    }

    fn edge_inspector(&mut self, ui: &mut egui::Ui, eidx: EdgeIndex) {
        let (from_l, to_l) = self.edge_endpoints_labels(eidx);
        self.edge_form(ui, eidx, &from_l, &to_l);

        // add-source form, kept out of the edge borrow above
        ui.separator();
        ui.label(RichText::new("attach source").strong());
        egui::ComboBox::from_id_salt("ns_kind")
            .selected_text(self.ns_kind.label())
            .show_ui(ui, |ui| {
                for k in SourceKind::ALL {
                    ui.selectable_value(&mut self.ns_kind, k, k.label());
                }
            });
        ui.add(egui::TextEdit::singleline(&mut self.ns_ref).hint_text(self.ns_kind.hint()));
        ui.add(
            egui::TextEdit::multiline(&mut self.ns_excerpt)
                .hint_text("excerpt / quote (optional)")
                .desired_rows(2),
        );
        if ui.button("+ source").clicked() && !self.ns_ref.trim().is_empty() {
            self.attach_source(eidx);
        }

        ui.separator();
        if ui.button("🗑 delete relation").clicked() {
            self.g.remove_edge(eidx);
        }
    }

    /// The kind selector, sourced/unsourced banner and editable source list.
    fn edge_form(&mut self, ui: &mut egui::Ui, eidx: EdgeIndex, from_l: &str, to_l: &str) {
        let Some(edge) = self.g.edge_mut(eidx) else { return };
        let r = edge.payload_mut();
        ui.label(RichText::new(format!("{from_l}  —{}→  {to_l}", r.kind.label())).strong());

        relation_kind_combo(ui, "insp_rel_kind", &mut r.kind);

        if r.is_sourced() {
            ui.colored_label(OK_GREEN, format!("✓ {} source(s)", r.sources.len()));
        } else {
            ui.colored_label(ALARM_RED, "⚠ UNSOURCED — unproven claim");
        }

        ui.separator();
        ui.label(RichText::new("sources").strong());
        let mut remove: Option<usize> = None;
        for (i, src) in r.sources.iter_mut().enumerate() {
            ui.group(|ui| {
                egui::ComboBox::from_id_salt(("src_kind", i))
                    .selected_text(src.kind.label())
                    .show_ui(ui, |ui| {
                        for k in SourceKind::ALL {
                            ui.selectable_value(&mut src.kind, k, k.label());
                        }
                    });
                ui.add(egui::TextEdit::singleline(&mut src.reference).hint_text(src.kind.hint()));
                ui.add(
                    egui::TextEdit::multiline(&mut src.excerpt)
                        .hint_text("excerpt / quote (optional)")
                        .desired_rows(2),
                );
                if ui.button("✕ remove source").clicked() {
                    remove = Some(i);
                }
            });
        }
        remove_marked(&mut r.sources, remove);
    }

    /// Push the add-source form contents onto the relation and reset the form.
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

/// Drop the row a ✕ button marked this frame, when there is one.
fn remove_marked<T>(items: &mut Vec<T>, marked: Option<usize>) {
    let Some(i) = marked else { return };
    items.remove(i);
}

/// Modal image picker; formats match the decoder features in Cargo.toml.
fn pick_image() -> Option<String> {
    let p = rfd::FileDialog::new()
        .add_filter("image", &["png", "jpg", "jpeg", "webp", "gif"])
        .pick_file()?;
    Some(p.to_string_lossy().into_owned())
}

/// Verb dropdown sectioned by category — `ALL` is contiguous per family, so
/// a weak header drops in whenever the category changes and 78 verbs stay
/// scannable.
fn relation_kind_combo(ui: &mut egui::Ui, id: &str, kind: &mut RelationKind) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(kind.label())
        .show_ui(ui, |ui| {
            let mut last_cat = "";
            for k in RelationKind::ALL {
                if k.category() != last_cat {
                    last_cat = k.category();
                    ui.label(RichText::new(last_cat).weak().small());
                }
                ui.selectable_value(kind, k, k.label());
            }
        });
}

/// Kind dropdown sectioned by ontology category — used by the inspector; the
/// add-entity flow uses the palette instead.
fn kind_combo(ui: &mut egui::Ui, id: &str, kind: &mut EntityKind) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(format!("{} {}", kind.icon(), kind.label()))
        .show_ui(ui, |ui| {
            for cat in Category::ALL {
                ui.add_space(2.0);
                ui.label(RichText::new(cat.label()).weak().small());
                for k in EntityKind::ALL.into_iter().filter(|k| k.category() == cat) {
                    ui.selectable_value(kind, k, format!("{} {}", k.icon(), k.label()));
                }
            }
        });
}
