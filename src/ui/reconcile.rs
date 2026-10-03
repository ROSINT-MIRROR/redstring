use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use egui::{Pos2, Vec2};
use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::dlog_info;
use crate::error::Error;
use crate::model::{DEFAULT_NODE_SIZE, Entity, KIND_META, RELATION_META, RelationRec, now_unix};
use crate::project::{
    CaseLock, CaseState, Claim, Merged, Project, Resolve, disk_sig, merge, note_on_disk,
    read_state, same_content, try_lock, wait_lock, write_changes,
};

use super::app::RedstringApp;
use super::board::camera_read;
use super::ops::free_spot;
use super::persist::{entity_of, rel_rec, relation_of};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Push {
    Absorb,
    Autosave,
    Explicit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Synced {
    Settled,
    Busy,
    Unreadable,
    Conflict,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Adopted {
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub added_slugs: Vec<String>,
    pub removed_slugs: Vec<String>,
}

type RelKey = (String, String, String, i64, usize);

const UNSAVED: &str = "\u{0}merged-but-unsaved";

impl RedstringApp {
    pub fn local_state(&self, ui: &egui::Ui) -> Result<CaseState, Error> {
        let file = self.to_case_file(ui)?;
        let mut notes = BTreeMap::new();
        for n in self.g.g().node_weights() {
            let e = n.payload();
            notes.insert(e.slug.clone(), note_on_disk(&e.note)?);
        }
        Ok(CaseState { file, notes })
    }

    pub fn sync_disk(&mut self, ui: &egui::Ui, push: Push, resolve: Resolve) -> Result<Synced, Error> {
        let Some(project) = self.project.clone() else {
            return Ok(Synced::Settled);
        };
        let Some(lock) = self.claim(&project, push)? else {
            self.status = "case busy — another redstring process is writing it; retrying".to_owned();
            return Ok(Synced::Busy);
        };
        let sig = disk_sig(&project)?;
        let disk = if self.watcher.in_sync(&project.root, &sig)? {
            self.watcher.base_or_empty()?
        } else {
            match read_state(&project) {
                Ok(state) => state,
                Err(err) => {
                    self.status = format!(
                        "⚠ case on disk is unreadable — your edits are held in memory, nothing written: {err}"
                    );
                    return Ok(Synced::Unreadable);
                }
            }
        };
        let base = self.ancestor(&disk)?;
        let local = self.local_state(ui)?;
        let merged = merge(&base, &local, &disk, resolve)?;
        if resolve == Resolve::Ask && !merged.clashes.is_empty() {
            self.raise_conflict(&merged)?;
            return Ok(Synced::Conflict);
        }
        self.autosave.conflict = None;
        let adopted = self.adopt(ui, &local, &merged.state)?;
        let mut new_base = disk;
        if push != Push::Absorb {
            match write_changes(&project, &new_base, &merged.state) {
                Ok(written) => dlog_info!(
                    "persist",
                    "wrote {} file(s), removed {} entit(ies) -> {}",
                    written.files,
                    written.removed,
                    project.root.display()
                ),
                Err(err) => {
                    self.watcher.forget()?;
                    return Err(err);
                }
            }
            new_base = merged.state.clone();
            self.last_saved_at = Some(now_unix()?);
        }
        self.autosave.last_camera = new_base.file.display.camera;
        let clean = same_content(&new_base, &self.local_state(ui)?)?;
        self.watcher.settle(&project.root, new_base, disk_sig(&project)?)?;
        drop(lock);
        self.saved_fp = if clean {
            self.fingerprint()?
        } else {
            UNSAVED.to_owned()
        };
        self.report_adoption(ui, &adopted, &merged)?;
        Ok(Synced::Settled)
    }

    fn ancestor(&self, disk: &CaseState) -> Result<CaseState, Error> {
        let Some(base) = &self.watcher.base else {
            let mut orphan = CaseState::default();
            orphan.file.meta = disk.file.meta.clone();
            orphan.file.display = disk.file.display.clone();
            return Ok(orphan);
        };
        Ok(base.clone())
    }

    fn claim(&self, project: &Project, push: Push) -> Result<Option<CaseLock>, Error> {
        if push == Push::Explicit {
            return Ok(Some(wait_lock(project, Duration::from_secs(3))?));
        }
        Ok(match try_lock(project)? {
            Claim::Held(lock) => Some(lock),
            Claim::Busy => None,
        })
    }

    fn report_adoption(&mut self, ui: &egui::Ui, adopted: &Adopted, merged: &Merged) -> Result<(), Error> {
        let touched = adopted.added + adopted.removed + adopted.changed;
        if touched == 0 && merged.dropped.is_empty() {
            return Ok(());
        }
        dlog_info!(
            "watch",
            "merged disk changes: +{} -{} ~{} dropped={}",
            adopted.added,
            adopted.removed,
            adopted.changed,
            merged.dropped.len()
        );
        self.fit_on_open = false;
        self.settle_arrivals(ui, &adopted.added_slugs, &adopted.removed_slugs)?;
        let mut status = format!(
            "external edit detected — merged from disk: {} added, {} removed, {} changed",
            adopted.added, adopted.removed, adopted.changed
        );
        if !merged.dropped.is_empty() {
            status += &format!(" · dropped links to missing entities: {}", merged.dropped.join(", "));
        }
        self.status = status;
        Ok(())
    }

    fn view_center(&self, ui: &egui::Ui) -> Result<Pos2, Error> {
        let cam = camera_read(ui)?;
        let zoom = if cam.zoom > 0.0 { cam.zoom } else { 1.0 };
        Ok(((self.board_size / 2.0 - Vec2::new(cam.x, cam.y)) / zoom).to_pos2())
    }

    pub fn adopt(&mut self, ui: &egui::Ui, local: &CaseState, target: &CaseState) -> Result<Adopted, Error> {
        let mut out = Adopted::default();
        if same_content(local, target)? && local.file.display.camera == target.file.display.camera {
            return Ok(out);
        }
        let Some(project) = self.project.clone() else {
            return Ok(out);
        };
        let wanted: HashSet<&str> = target.file.data.entities.iter().map(|e| e.slug.as_str()).collect();
        let mut by_slug: HashMap<String, NodeIndex> = HashMap::new();
        for idx in self.g.g().node_indices().collect::<Vec<_>>() {
            let Some(n) = self.g.node(idx) else { continue };
            let slug = n.payload().slug.clone();
            if wanted.contains(slug.as_str()) {
                by_slug.insert(slug, idx);
                continue;
            }
            self.g.remove_node(idx);
            out.removed += 1;
            out.removed_slugs.push(slug);
        }
        let mut taken: Vec<Pos2> = by_slug.values().filter_map(|i| self.g.node(*i)).map(|n| n.location()).collect();
        let anchor = self.view_center(ui)?;
        let d = &target.file.display;
        for er in &target.file.data.entities {
            let note = target.note(&er.slug)?;
            let Some(&idx) = by_slug.get(&er.slug) else {
                let pos = saved_or_free(d.positions.get(&er.slug), &taken, anchor)?;
                taken.push(pos);
                let label = format!("{} {}", KIND_META[er.kind as usize].icon, er.label);
                let idx = self.g.add_node_with_label_and_location(entity_of(&project, er, note, d)?, label, pos);
                by_slug.insert(er.slug.clone(), idx);
                out.added += 1;
                out.added_slugs.push(er.slug.clone());
                continue;
            };
            let unchanged = local.file.data.entities.iter().any(|l| l == er) && local.note(&er.slug)? == note;
            let Some(node) = self.g.node_mut(idx) else { continue };
            if !unchanged {
                let fresh = entity_of(&project, er, note, d)?;
                let e = node.payload_mut();
                *e = Entity {
                    boost: e.boost,
                    hidden: e.hidden,
                    ..fresh
                };
                node.set_label(format!("{} {}", KIND_META[er.kind as usize].icon, er.label));
                out.changed += 1;
            }
            let e = node.payload_mut();
            e.size = DEFAULT_NODE_SIZE;
            for s in d.sizes.get(&er.slug).into_iter() {
                e.size = *s;
            }
            e.collapsed = d.collapsed.contains(&er.slug);
            for &(x, y) in d.positions.get(&er.slug).into_iter() {
                if local.file.display.positions.get(&er.slug) != Some(&(x, y)) {
                    node.set_location(Pos2::new(x, y));
                }
            }
        }
        self.adopt_relations(&by_slug, local, target, &mut out)?;
        self.case_name = target.file.meta.name.clone();
        self.current_layout = d.layout;
        self.size_by_degree = d.size_by_degree;
        if d.camera != local.file.display.camera {
            self.pending_camera = Some(d.camera);
        }
        if out.removed > 0 {
            self.forget_node_refs()?;
        }
        Ok(out)
    }

    fn adopt_relations(
        &mut self,
        by_slug: &HashMap<String, NodeIndex>,
        local: &CaseState,
        target: &CaseState,
        out: &mut Adopted,
    ) -> Result<(), Error> {
        let want = keyed(&target.file.data.relations);
        let want_map: HashMap<&RelKey, &RelationRec> = want.iter().map(|(k, r)| (k, *r)).collect();
        let have = keyed(&local.file.data.relations);
        let have_keys: HashSet<&RelKey> = have.iter().map(|(k, _)| k).collect();
        let edges = self.edge_keys()?;
        for (k, current, eidx) in &edges {
            let Some(rr) = want_map.get(k) else {
                self.g.remove_edge(*eidx);
                out.changed += 1;
                continue;
            };
            if current == *rr {
                continue;
            }
            let Some(edge) = self.g.edge_mut(*eidx) else { continue };
            *edge.payload_mut() = relation_of(rr)?;
            out.changed += 1;
        }
        for (k, rr) in &want {
            if have_keys.contains(k) {
                continue;
            }
            let (Some(&a), Some(&b)) = (by_slug.get(&rr.from), by_slug.get(&rr.to)) else {
                continue;
            };
            self.g.add_edge_with_label(a, b, relation_of(rr)?, RELATION_META[rr.kind as usize].label.into());
            out.changed += 1;
        }
        Ok(())
    }

    fn edge_keys(&self) -> Result<Vec<(RelKey, RelationRec, EdgeIndex)>, Error> {
        let g = self.g.g();
        let mut recs: Vec<RelationRec> = Vec::new();
        let mut idxs: Vec<EdgeIndex> = Vec::new();
        for idx in g.edge_indices() {
            let Some((a, b)) = g.edge_endpoints(idx) else { continue };
            let (Some(from), Some(to), Some(r)) = (g.node_weight(a), g.node_weight(b), g.edge_weight(idx)) else {
                continue;
            };
            recs.push(rel_rec(&from.payload().slug, &to.payload().slug, r.payload())?);
            idxs.push(idx);
        }
        let mut out = Vec::new();
        for ((k, rec), idx) in keyed(&recs).into_iter().zip(idxs) {
            out.push((k, rec.clone(), idx));
        }
        Ok(out)
    }
}

fn saved_or_free(saved: Option<&(f32, f32)>, taken: &[Pos2], anchor: Pos2) -> Result<Pos2, Error> {
    let Some(&(x, y)) = saved else {
        return free_spot(taken, anchor);
    };
    Ok(Pos2::new(x, y))
}

fn keyed(v: &[RelationRec]) -> Vec<(RelKey, &RelationRec)> {
    let mut seen: HashMap<(String, String, String, i64), usize> = HashMap::new();
    let mut out = Vec::new();
    for r in v {
        let k = (r.from.clone(), r.to.clone(), format!("{:?}", r.kind), r.created_at);
        let nth = seen.entry(k.clone()).or_insert(0);
        out.push(((k.0, k.1, k.2, k.3, *nth), r));
        *nth += 1;
    }
    out
}
