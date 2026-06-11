use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use egui::{Pos2, RichText, Vec2};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::{EntityKind, RELATION_META, Relation, Source, SourceKind, slugify};
use crate::modules::{Fact, ModuleRun, ModuleStatus, RunInput};

use super::app::RedstringApp;
use super::layout::LayoutAnim;
use super::style;


pub struct Job {
    pub run_id: u64,
    pub module_id: String,
    pub module_label: String,
    pub target_slug: String,
    pub target_label: String,
    pub started: Instant,
    pub cancel: Arc<AtomicBool>,

    pub found: usize,

    pub streams: bool,
}

impl RedstringApp {

    pub fn lookup_buttons(
        &self,
        ui: &mut egui::Ui,
        kind: EntityKind,
        module_ok: bool,
        slug: &str,
    ) -> Result<Option<usize>, Error> {
        let mut run_module = None;
        let mods = self.registry.for_kind(kind)?;
        if mods.is_empty() {
            ui.label(
                RichText::new("no modules for this kind — see settings → modules")
                    .weak()
                    .small(),
            );
            return Ok(None);
        }
        for mi in mods {
            let entry = &self.registry.entries[mi];
            let in_flight = self.job_running(&entry.spec.id, slug);
            let resp = ui.add_enabled(
                module_ok && !in_flight,
                egui::Button::new(RichText::new(format!(
                    "{} {}",
                    egui_phosphor::regular::PLAY,
                    entry.spec.label
                ))),
            );
            let resp = if in_flight {
                resp.on_disabled_hover_text("already running on this entity")
            } else {
                resp
            };
            if resp.clicked() {
                run_module = Some(mi);
            }
        }
        self.job_rows(ui)?;
        Ok(run_module)
    }


    fn job_rows(&self, ui: &mut egui::Ui) -> Result<(), Error> {
        for job in &self.jobs {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(style::ICON_SECTION));
                ui.label(
                    RichText::new(format!(
                        "{} on {} · {}s",
                        job.module_label,
                        job.target_label,
                        job.started.elapsed().as_secs()
                    ))
                    .weak()
                    .small(),
                );
                let hint = if job.streams {
                    "stop the sweep — the hits already on the board stay"
                } else {
                    "cancel this job — nothing will be applied"
                };
                if ui
                    .small_button(egui_phosphor::regular::X)
                    .on_hover_text(hint)
                    .clicked()
                {
                    job.cancel.store(true, Ordering::Relaxed);
                }
            });
        }
        if !self.jobs.is_empty() {
            ui.ctx().request_repaint_after(Duration::from_millis(500));
        }
        Ok(())
    }


    pub fn jobs_chip(&self, ui: &mut egui::Ui) -> Result<(), Error> {
        if self.jobs.is_empty() {
            return Ok(());
        }
        ui.separator();
        ui.add(egui::Spinner::new().size(14.0));
        let label = if self.jobs.len() == 1 {
            format!("{} on {}", self.jobs[0].module_label, self.jobs[0].target_label)
        } else {
            format!("{} lookup jobs", self.jobs.len())
        };
        ui.label(RichText::new(label).weak().small());
        ui.ctx().request_repaint_after(Duration::from_millis(500));
        Ok(())
    }

    fn job_running(&self, module_id: &str, slug: &str) -> bool {
        self.jobs
            .iter()
            .any(|j| j.module_id == module_id && j.target_slug == slug)
    }


    pub fn start_module_request(
        &mut self,
        ui: &egui::Ui,
        nidx: NodeIndex,
        req: Option<usize>,
    ) -> Result<(), Error> {
        let Some(mi) = req else {
            return Ok(());
        };
        let Some(input) = self.run_input(mi, nidx)? else {
            self.status = "lookup needs a valid value — see the value field".into();
            return Ok(());
        };
        self.jobs.push(Job {
            run_id: input.run_id,
            module_id: input.module_id.clone(),
            module_label: input.module_label.clone(),
            target_slug: input.target_slug.clone(),
            target_label: input.target_label.clone(),
            started: Instant::now(),
            cancel: input.cancel.clone(),
            found: 0,
            streams: crate::modules::streaming(&input.shapes)?,
        });
        self.status = format!("{} running on {}…", input.module_label, input.target_label);
        crate::modules::spawn_module(input, self.module_tx.clone(), ui.ctx().clone())?;
        Ok(())
    }

    fn run_input(&mut self, mi: usize, nidx: NodeIndex) -> Result<Option<RunInput>, Error> {
        let entry = &self.registry.entries[mi];
        let ModuleStatus::Active(bin) = &entry.status else {
            return Ok(None);
        };
        let Some(node) = self.g.g().node_weight(nidx) else {
            return Ok(None);
        };
        let e = node.payload();
        let Some(value) = e.module_value()? else {
            return Ok(None);
        };
        let run_id = self.next_run_id;
        self.next_run_id += 1;
        let project_root = self.project.as_ref().map(|p| p.root.clone());
        let evidence_rel = if project_root.is_some() {
            format!("evidence/docs/{}-{}-{}.txt", entry.spec.id, e.slug, epoch_secs()?)
        } else {
            String::new()
        };
        Ok(Some(RunInput {
            run_id,
            module_id: entry.spec.id.clone(),
            module_label: entry.spec.label.clone(),
            bin: bin.clone(),
            args: entry.spec.render_args(&value, &e.label, &e.slug, &e.attrs)?,
            env: key_env(&entry.spec.keys, &self.prefs.api_keys),
            timeout_secs: entry.spec.timeout_secs,
            shapes: entry.spec.shapes.clone(),
            target_slug: e.slug.clone(),
            target_label: e.label.clone(),
            project_root,
            evidence_rel,
            cancel: Arc::new(AtomicBool::new(false)),
        }))
    }


    pub fn poll_module_runs(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        loop {
            let Ok(run) = self.module_rx.try_recv() else {
                return Ok(());
            };
            let run_id = run.run_id;
            let last = run.last;
            if run.cancelled {
                self.jobs.retain(|j| j.run_id != run_id);
                self.status = format!(
                    "{} on {} cancelled — {} kept",
                    run.module_label, run.target_label, self.job_found(run_id)
                );
                continue;
            }
            if !run.ok {
                self.jobs.retain(|j| j.run_id != run_id);
                self.status = format!("{} failed: {}", run.module_label, run.stderr_tail);
                continue;
            }
            if !run.facts.is_empty() {
                self.apply_run(ui, run)?;
            } else if last {
                self.status = format!(
                    "{} done — {} found",
                    self.job_label(run_id),
                    self.job_found(run_id)
                );
            }
            if last {
                self.jobs.retain(|j| j.run_id != run_id);
            }
        }
    }

    fn job_found(&self, run_id: u64) -> usize {
        let Some(job) = self.jobs.iter().find(|j| j.run_id == run_id) else {
            return 0;
        };
        job.found
    }

    fn job_label(&self, run_id: u64) -> String {
        let Some(job) = self.jobs.iter().find(|j| j.run_id == run_id) else {
            return "lookup".to_owned();
        };
        format!("{} on {}", job.module_label, job.target_label)
    }

    fn apply_run(&mut self, ui: &egui::Ui, run: ModuleRun) -> Result<(), Error> {
        let Some(tidx) = self.node_by_slug(&run.target_slug) else {
            self.status = "lookup target no longer on the board — result dropped".into();
            return Ok(());
        };
        let source = self.evidence_source(&run)?;
        if run.shape_id.is_empty() {
            self.status = format!(
                "{}: no known output shape — raw output archived, nothing parsed",
                run.module_id
            );
            return Ok(());
        }
        let origin = self.node_pos(tidx);
        let base = self.g.g().neighbors(tidx).count();
        let mut attrs_set = 0usize;
        let mut linked = 0usize;
        let mut skipped = 0usize;
        let mut spawned = 0usize;
        let mut steps: Vec<(NodeIndex, Pos2, Pos2)> = Vec::new();
        for fact in &run.facts {
            match fact {
                Fact::Attr { key, value } => {
                    self.apply_attr_fact(tidx, key, value, &mut attrs_set, &mut skipped)?;
                }
                Fact::Entity {
                    kind,
                    label,
                    value,
                    rel,
                    outbound,
                    attrs,
                } => {
                    let nidx = self.upsert_entity(
                        label, value, *kind, attrs, origin, base, &mut spawned, &mut steps,
                    )?;
                    let (a, b) = if *outbound { (tidx, nidx) } else { (nidx, tidx) };
                    let dup = self
                        .g
                        .g()
                        .edges_connecting(a, b)
                        .any(|er| er.weight().payload().kind == *rel);
                    if dup {
                        skipped += 1;
                        continue;
                    }
                    self.g.add_edge_with_label(
                        a,
                        b,
                        Relation {
                            kind: *rel,
                            sources: vec![source.clone()],
                        },
                        RELATION_META[*rel as usize].label.into(),
                    );
                    linked += 1;
                }
            }
        }
        self.kick_spawn_anim(ui, steps)?;
        let total = self.bump_job_found(run.run_id, linked);
        self.status = if run.last {
            format!(
                "{}: {attrs_set} attrs set, {total} linked, {skipped} skipped — prune what's irrelevant",
                run.module_id
            )
        } else {
            format!("{}: {total} linked so far…", run.module_id)
        };
        Ok(())
    }

    fn bump_job_found(&mut self, run_id: u64, linked: usize) -> usize {
        let Some(job) = self.jobs.iter_mut().find(|j| j.run_id == run_id) else {
            return linked;
        };
        job.found += linked;
        job.found
    }

    fn kick_spawn_anim(
        &mut self,
        ui: &egui::Ui,
        steps: Vec<(NodeIndex, Pos2, Pos2)>,
    ) -> Result<(), Error> {
        if steps.is_empty() {
            return Ok(());
        }
        let start = ui.input(|i| i.time);
        ui.ctx().request_repaint();
        let Some(anim) = self.layout_anim.as_mut() else {
            self.layout_anim = Some(LayoutAnim { steps, start });
            return Ok(());
        };
        anim.steps.extend(steps);
        anim.start = start;
        Ok(())
    }

    fn apply_attr_fact(
        &mut self,
        tidx: NodeIndex,
        key: &str,
        value: &str,
        attrs_set: &mut usize,
        skipped: &mut usize,
    ) -> Result<(), Error> {
        let Some(node) = self.g.node_mut(tidx) else {
            return Ok(());
        };
        fill_attr(node.payload_mut().attrs.as_mut(), key, value, attrs_set, skipped);
        Ok(())
    }

    fn upsert_entity(
        &mut self,
        label: &str,
        value: &str,
        kind: EntityKind,
        attrs: &[(String, String)],
        origin: Pos2,
        base: usize,
        spawned: &mut usize,
        steps: &mut Vec<(NodeIndex, Pos2, Pos2)>,
    ) -> Result<NodeIndex, Error> {
        let slug = slugify(value)?;
        let existing = self.node_by_slug(&slug);
        let nidx = self.ensure_node(existing, label, value, kind, origin, base, spawned, steps)?;
        let Some(node) = self.g.node_mut(nidx) else {
            return Ok(nidx);
        };
        let e = node.payload_mut();
        if e.value.trim().is_empty() {
            e.value = value.to_owned();
        }
        let mut set = 0usize;
        let mut skip = 0usize;
        for (k, v) in attrs {
            fill_attr(&mut e.attrs, k, v, &mut set, &mut skip);
        }
        Ok(nidx)
    }

    fn ensure_node(
        &mut self,
        existing: Option<NodeIndex>,
        label: &str,
        value: &str,
        kind: EntityKind,
        origin: Pos2,
        base: usize,
        spawned: &mut usize,
        steps: &mut Vec<(NodeIndex, Pos2, Pos2)>,
    ) -> Result<NodeIndex, Error> {
        let Some(idx) = existing else {
            let angle = 0.45 + (base + *spawned) as f32 * 0.9;
            let target = origin + Vec2::new(170.0 * angle.cos(), 170.0 * angle.sin());
            *spawned += 1;
            let idx = self.add_entity(kind, label.to_owned(), origin)?;
            self.reslug(idx, value)?;
            steps.push((idx, origin, target));
            return Ok(idx);
        };
        Ok(idx)
    }

    fn reslug(&mut self, idx: NodeIndex, value: &str) -> Result<(), Error> {
        let slug = slugify(value)?;
        let Some(n) = self.g.node_mut(idx) else {
            return Ok(());
        };
        n.payload_mut().slug = slug;
        Ok(())
    }

    fn node_by_slug(&self, slug: &str) -> Option<NodeIndex> {
        self.g.g().node_indices().find(|i| {
            let Some(n) = self.g.g().node_weight(*i) else {
                return false;
            };
            n.payload().slug == slug
        })
    }

    fn node_pos(&self, idx: NodeIndex) -> Pos2 {
        let Some(n) = self.g.g().node_weight(idx) else {
            return Pos2::ZERO;
        };
        n.location()
    }

    fn evidence_source(&self, run: &ModuleRun) -> Result<Source, Error> {
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
        })
    }
}

fn fill_attr(
    attrs: &mut Vec<(String, String)>,
    key: &str,
    value: &str,
    set: &mut usize,
    skipped: &mut usize,
) {
    let pos = attrs.iter().position(|(k, _)| k == key);
    let Some(p) = pos else {
        attrs.push((key.to_owned(), value.to_owned()));
        *set += 1;
        return;
    };
    if attrs[p].1.trim().is_empty() {
        attrs[p].1 = value.to_owned();
        *set += 1;
        return;
    }
    *skipped += 1;
}

fn key_env(keys: &[String], api_keys: &[(String, String)]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for id in keys {
        let Some((_, v)) = api_keys.iter().find(|(k, _)| k == id) else {
            continue;
        };
        if v.is_empty() {
            continue;
        }
        out.push((format!("REDSTRING_KEY_{}", id.to_uppercase()), v.clone()));
    }
    out
}

fn epoch_secs() -> Result<u64, Error> {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|source| Error::Clock { source })?;
    Ok(d.as_secs())
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn note_source(run: &ModuleRun) -> Source {
    Source {
        kind: SourceKind::Note,
        reference: format!(
            "{} on {} — raw output not persisted (case was unsaved)",
            run.module_id, run.target_label
        ),
        excerpt: clip(&run.raw, 600),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_env_builds_only_configured_nonempty_keys() {
        let keys = vec!["web_archive".to_owned(), "missing".to_owned(), "empty".to_owned()];
        let api = vec![
            ("web_archive".to_owned(), "secret".to_owned()),
            ("empty".to_owned(), String::new()),
        ];
        let env = key_env(&keys, &api);
        assert_eq!(env, vec![("REDSTRING_KEY_WEB_ARCHIVE".to_owned(), "secret".to_owned())]);
    }

    #[test]
    fn fill_attr_never_clobbers() {
        let mut attrs = vec![("country".to_owned(), "AU".to_owned()), ("city".to_owned(), String::new())];
        let (mut set, mut skip) = (0usize, 0usize);
        fill_attr(&mut attrs, "country", "US", &mut set, &mut skip);
        fill_attr(&mut attrs, "city", "Brisbane", &mut set, &mut skip);
        fill_attr(&mut attrs, "region", "QLD", &mut set, &mut skip);
        assert_eq!(attrs[0].1, "AU", "existing value untouched");
        assert_eq!(attrs[1].1, "Brisbane", "empty filled");
        assert_eq!(attrs[2], ("region".to_owned(), "QLD".to_owned()), "missing appended");
        assert_eq!((set, skip), (2, 1));
    }

    #[test]
    fn clip_caps_length() {
        assert_eq!(clip("abcdef", 3), "abc");
        assert_eq!(clip("ab", 9), "ab");
    }
}
