use egui::{RichText, Vec2};
use egui_phosphor::regular::{ARROW_RIGHT, CLOCK_COUNTER_CLOCKWISE, CORNERS_OUT, CROSSHAIR, X};
use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::error::Error;
use crate::model::{SOURCE_META, Source};

use super::super::app::RedstringApp;
use super::super::board::camera_read;
use super::super::center::center_on_node;
use super::canvas::{Pick, paint};
use super::card::CardAct;
use super::derive::Digest;
use super::event::{Lane, Record};
use super::state::{Fit, Focus};

impl RedstringApp {
    pub fn timeline_panel(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        let digest = Digest::of(&self.g)?;
        let focus = Focus::of(&self.g)?;
        self.follow_board(&digest, &focus)?;
        self.timeline_header(ui, &digest)?;
        ui.separator();
        let card_w = (ui.available_width() * 0.3).clamp(260.0, 380.0);
        let (pick, act) = ui
            .horizontal_top(|ui| -> Result<(Pick, CardAct), Error> {
                let h = ui.available_height();
                let canvas_w = (ui.available_width() - card_w - 12.0).max(240.0);
                let pick = ui
                    .allocate_ui(Vec2::new(canvas_w, h), |ui| {
                        paint(ui, &digest, &focus, &mut self.timeline)
                    })
                    .inner?;
                ui.separator();
                let top_down = egui::Layout::top_down(egui::Align::Min);
                let act = ui
                    .allocate_ui_with_layout(Vec2::new(card_w, h), top_down, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("timeline_card")
                            .auto_shrink([false, false])
                            .show(ui, |ui| self.timeline_card(ui, &digest))
                            .inner
                    })
                    .inner?;
                Ok((pick, act))
            })
            .inner?;
        self.apply_pick(ui, &digest, pick)?;
        self.apply_card(ui, act)
    }

    pub fn show_record(&mut self, ui: &egui::Ui, record: Record) -> Result<(), Error> {
        match record {
            Record::Entity(n) | Record::Obs(n, ..) => self.focus_node(ui, n)?,
            Record::Rel { edge, from, to } => self.focus_edge(ui, edge, from, to)?,
        }
        self.timeline.seen = Focus::of(&self.g)?;
        Ok(())
    }

    fn timeline_header(&mut self, ui: &mut egui::Ui, digest: &Digest) -> Result<(), Error> {
        ui.horizontal_wrapped(|ui| -> Result<(), Error> {
            if ui
                .small_button(X)
                .on_hover_text("close the timeline (Ctrl+T)")
                .clicked()
            {
                self.timeline.open = false;
            }
            ui.label(RichText::new(format!("{CLOCK_COUNTER_CLOCKWISE} timeline")).strong())
                .on_hover_text("the same case, ordered by time — nothing here is a copy");
            ui.separator();
            for lane in Lane::ALL {
                let on = &mut self.timeline.lanes[lane.slot()?];
                ui.toggle_value(on, lane.label()?)
                    .on_hover_text(lane.caption()?);
            }
            ui.separator();
            ui.toggle_value(&mut self.timeline.follow, "follow board")
                .on_hover_text("selecting on the board pins and frames its marks here");
            if ui.button(format!("{CORNERS_OUT} fit all")).clicked() {
                self.timeline.pending_fit = Some(Fit::All);
            }
            if ui.button(format!("{CROSSHAIR} fit selection")).clicked() {
                self.timeline.pending_fit = Some(Fit::Focus);
            }
            ui.separator();
            ui.label(RichText::new(summary(digest)?).weak().small());
            Ok(())
        })
        .inner
    }

    fn follow_board(&mut self, digest: &Digest, focus: &Focus) -> Result<(), Error> {
        if *focus == self.timeline.seen {
            return Ok(());
        }
        self.timeline.seen = focus.clone();
        if !self.timeline.follow || focus.is_empty()? {
            return Ok(());
        }
        for ev in &digest.events {
            if focus.holds(ev.key.record)? {
                self.timeline.selected = Some(ev.key);
                self.timeline.pending_fit = Some(Fit::Focus);
                return Ok(());
            }
        }
        Ok(())
    }

    fn apply_pick(&mut self, ui: &egui::Ui, digest: &Digest, pick: Pick) -> Result<(), Error> {
        match pick {
            Pick::Event(i) => {
                let key = digest.events[i].key;
                self.timeline.selected = Some(key);
                self.show_record(ui, key.record)
            }
            Pick::Background => {
                self.timeline.selected = None;
                Ok(())
            }
            Pick::Nothing => Ok(()),
        }
    }

    fn apply_card(&mut self, ui: &egui::Ui, act: CardAct) -> Result<(), Error> {
        match act {
            CardAct::Select(key) => {
                self.timeline.selected = Some(key);
                self.show_record(ui, key.record)?;
                self.timeline.pending_fit = Some(Fit::Focus);
                Ok(())
            }
            CardAct::Show(record) => self.show_record(ui, record),
            CardAct::Open(source) => self.open_source(ui, &source),
            CardAct::Nothing => Ok(()),
        }
    }

    fn focus_edge(
        &mut self,
        ui: &egui::Ui,
        edge: EdgeIndex,
        from: NodeIndex,
        to: NodeIndex,
    ) -> Result<(), Error> {
        let a = self.g.node(from).ok_or(lost(from))?.location();
        let b = self.g.node(to).ok_or(lost(to))?.location();
        let nodes: Vec<NodeIndex> = self.g.g().node_indices().collect();
        for n in nodes {
            let Some(node) = self.g.node_mut(n) else {
                continue;
            };
            node.set_selected(false);
        }
        let edges: Vec<EdgeIndex> = self.g.g().edge_indices().collect();
        for e in edges {
            let Some(item) = self.g.edge_mut(e) else {
                continue;
            };
            item.set_selected(e == edge);
        }
        self.g.set_selected_nodes(Vec::new());
        self.g.set_selected_edges(vec![edge]);
        let zoom = camera_read(ui)?.zoom.max(0.9);
        self.pending_camera = Some(center_on_node(a + (b - a) * 0.5, self.board_size, zoom)?);
        let (from_l, to_l) = self.edge_endpoints_labels(edge)?;
        self.status = format!("timeline: relation {from_l} {ARROW_RIGHT} {to_l}");
        Ok(())
    }

    fn open_source(&mut self, ui: &egui::Ui, src: &Source) -> Result<(), Error> {
        let reference = src.reference.trim();
        if reference.starts_with("https://") || reference.starts_with("http://") {
            ui.ctx().open_url(egui::OpenUrl::new_tab(reference));
            self.status = format!("opened {reference}");
            return Ok(());
        }
        if !file_backed(src) {
            self.status = "this source is a note — there is nothing to open".to_owned();
            return Ok(());
        }
        let Some(project) = self.project.clone() else {
            self.status = "save the case first — cited files live in the case directory".to_owned();
            return Ok(());
        };
        let Some(rel) = reference.split(['#', '·']).next() else {
            return Ok(());
        };
        let target = project.root.join(rel.trim());
        let root = project.root.canonicalize().map_err(|source| Error::Io {
            path: project.root.clone(),
            source,
        })?;
        let file = target.canonicalize().map_err(|source| Error::Io {
            path: target.clone(),
            source,
        })?;
        if !file.starts_with(&root) {
            self.status = format!("not opened — {} lies outside the case", file.display());
            return Ok(());
        }
        ui.ctx()
            .open_url(egui::OpenUrl::new_tab(format!("file://{}", file.display())));
        self.status = format!("opened {}", rel.trim());
        Ok(())
    }
}

fn lost(n: NodeIndex) -> Error {
    Error::Timeline {
        what: format!("entity {} left the board", n.index()),
    }
}

fn file_backed(src: &Source) -> bool {
    for m in SOURCE_META.iter() {
        if m.kind == src.kind {
            return m.file_backed;
        }
    }
    false
}

fn summary(digest: &Digest) -> Result<String, Error> {
    Ok(format!(
        "{} marks · {} conflicting pair(s) · {} unplaced",
        digest.events.len(),
        digest.conflict_count()?,
        digest.unplaced.len()
    ))
}
