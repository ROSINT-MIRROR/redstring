use std::time::{SystemTime, UNIX_EPOCH};

use egui::{Pos2, RichText, Vec2};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::{
    EntityKind, KIND_META, RELATION_META, Relation, Source, SourceKind, pretty_key,
    slugify,
};
use crate::modules::{Fact, ModuleRun, ModuleStatus, RunInput};

use super::app::{ALARM_RED, RedstringApp};
use super::style;

impl RedstringApp {

    pub fn lookup_buttons(
        &self,
        ui: &mut egui::Ui,
        kind: EntityKind,
    ) -> Result<Option<usize>, Error> {
        let mut run_module = None;
        let mods = self.registry.for_kind(kind)?;
        if mods.is_empty() {
            ui.label(
                RichText::new("no modules for this kind — see settings → modules")
                    .weak()
                    .small(),
            );
        }
        for mi in mods {
            let label = self.registry.entries[mi].spec.label.clone();
            if style::action_btn(ui, egui_phosphor::regular::PLAY, &label)? {
                run_module = Some(mi);
            }
        }
        if self.module_busy > 0 {
            ui.label(
                RichText::new(format!("{} lookup(s) running…", self.module_busy))
                    .weak()
                    .small(),
            );
        }
        Ok(run_module)
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
            self.status = "module unavailable — see settings → modules".into();
            return Ok(());
        };
        self.module_busy += 1;
        self.status = format!("{} running on {}…", input.module_label, input.target_label);
        crate::modules::spawn_module(input, self.module_tx.clone(), ui.ctx().clone())?;
        Ok(())
    }

    fn run_input(&self, mi: usize, nidx: NodeIndex) -> Result<Option<RunInput>, Error> {
        let entry = &self.registry.entries[mi];
        let ModuleStatus::Active(bin) = &entry.status else {
            return Ok(None);
        };
        let Some(node) = self.g.g().node_weight(nidx) else {
            return Ok(None);
        };
        let e = node.payload();
        Ok(Some(RunInput {
            module_id: entry.spec.id.clone(),
            module_label: entry.spec.label.clone(),
            bin: bin.clone(),
            args: entry.spec.render_args(&e.label, &e.slug, &e.attrs)?,
            env: key_env(&entry.spec.keys, &self.prefs.api_keys),
            timeout_secs: entry.spec.timeout_secs,
            shapes: entry.spec.shapes.clone(),
            target_slug: e.slug.clone(),
            target_label: e.label.clone(),
        }))
    }


    pub fn module_results_window(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        self.drain_module_runs();
        if self.module_runs.is_empty() {
            return Ok(());
        }
        let queued = self.module_runs.len();
        let mut keep = false;
        let mut discard = false;
        let run = &mut self.module_runs[0];
        egui::Window::new("lookup results")
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ui.ctx().content_rect().center())
            .min_size(Vec2::new(480.0, 160.0))
            .show(ui.ctx(), |ui| -> Result<(), Error> {
                ui.label(
                    RichText::new(format!("{} on {}", run.module_label, run.target_label))
                        .strong(),
                );
                if queued > 1 {
                    ui.label(RichText::new(format!("{} more queued", queued - 1)).weak().small());
                }
                if !run.ok {
                    ui.colored_label(ALARM_RED, format!("run failed:\n{}", run.stderr_tail));
                }
                if run.ok && run.shape_id.is_empty() {
                    ui.colored_label(
                        ALARM_RED,
                        "no known output shape — nothing parsed, raw output can still be kept",
                    );
                }
                if !run.shape_id.is_empty() {
                    ui.label(RichText::new(format!("matched shape: {}", run.shape_id)).weak().small());
                }
                ui.add_space(style::GAP_SM);
                for (fact, keep) in &mut run.facts {
                    ui.checkbox(keep, fact_label(fact)?);
                }
                ui.add_space(style::GAP_SM);
                egui::CollapsingHeader::new("raw output")
                    .default_open(run.facts.is_empty())
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                            ui.label(RichText::new(run.raw.as_str()).monospace().small());
                        });
                    });
                ui.add_space(style::GAP_SM);
                ui.horizontal(|ui| -> Result<(), Error> {
                    keep = style::action_btn(ui, egui_phosphor::regular::CHECK, "keep")?;
                    discard =
                        style::danger_btn(ui, egui_phosphor::regular::TRASH, "discard", "drop this run — nothing enters the case")?;
                    ui.label(
                        RichText::new("keep = checked facts enter the board, raw output is archived as evidence")
                            .weak()
                            .small(),
                    );
                    Ok(())
                })
                .inner
            });
        if keep {
            let run = self.module_runs.remove(0);
            self.apply_run(run)?;
        }
        if discard {
            self.module_runs.remove(0);
            self.status = "lookup discarded — nothing entered the case".into();
        }
        Ok(())
    }

    fn drain_module_runs(&mut self) {
        loop {
            let Ok(run) = self.module_rx.try_recv() else {
                break;
            };
            self.module_busy = self.module_busy.saturating_sub(1);
            self.module_runs.push(run);
        }
    }

    fn apply_run(&mut self, run: ModuleRun) -> Result<(), Error> {
        let Some(tidx) = self.node_by_slug(&run.target_slug) else {
            self.status = "lookup target no longer on the board — discarded".into();
            return Ok(());
        };
        let source = self.evidence_source(&run)?;
        let origin = self.node_pos(tidx);
        let mut attrs_set = 0usize;
        let mut linked = 0usize;
        let mut skipped = 0usize;
        let mut spawned = 0usize;
        for (fact, keep) in &run.facts {
            if !keep {
                continue;
            }
            match fact {
                Fact::Attr { key, value } => {
                    self.apply_attr_fact(tidx, key, value, &mut attrs_set, &mut skipped)?;
                }
                Fact::Entity {
                    kind,
                    label,
                    rel,
                    outbound,
                } => {
                    let nidx = self.upsert_entity(label, *kind, origin, &mut spawned)?;
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
        self.status = format!(
            "{}: {attrs_set} attrs set, {linked} linked, {skipped} skipped (already present)",
            run.module_id
        );
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
        let e = node.payload_mut();
        let pos = e.attrs.iter().position(|(k, _)| k == key);
        let Some(p) = pos else {
            e.attrs.push((key.to_owned(), value.to_owned()));
            *attrs_set += 1;
            return Ok(());
        };
        if e.attrs[p].1.trim().is_empty() {
            e.attrs[p].1 = value.to_owned();
            *attrs_set += 1;
            return Ok(());
        }
        *skipped += 1;
        Ok(())
    }

    fn upsert_entity(
        &mut self,
        label: &str,
        kind: EntityKind,
        origin: Pos2,
        spawned: &mut usize,
    ) -> Result<NodeIndex, Error> {
        let slug = slugify(label)?;
        let Some(existing) = self.node_by_slug(&slug) else {
            let angle = 0.45 + *spawned as f32 * 0.9;
            let pos = origin + Vec2::new(170.0 * angle.cos(), 170.0 * angle.sin());
            *spawned += 1;
            return self.add_entity(kind, label.to_owned(), pos);
        };
        Ok(existing)
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

    fn evidence_source(&mut self, run: &ModuleRun) -> Result<Source, Error> {
        let Some(project) = &self.project else {
            return Ok(Source {
                kind: SourceKind::Note,
                reference: format!(
                    "{} on {} — raw output not persisted (case was unsaved)",
                    run.module_id, run.target_label
                ),
                excerpt: clip(&run.raw, 600),
            });
        };
        let rel = format!(
            "evidence/docs/{}-{}-{}.txt",
            run.module_id,
            run.target_slug,
            epoch_secs()?
        );
        let abs = project.resolve(&rel)?;
        let Some(dir) = abs.parent() else {
            return Err(Error::NotAProject { path: abs.clone() });
        };
        std::fs::create_dir_all(dir).map_err(|source| Error::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        std::fs::write(&abs, &run.raw).map_err(|source| Error::Io {
            path: abs.clone(),
            source,
        })?;
        Ok(Source {
            kind: SourceKind::Document,
            reference: rel,
            excerpt: clip(&run.raw, 280),
        })
    }
}

fn fact_label(fact: &Fact) -> Result<String, Error> {
    match fact {
        Fact::Attr { key, value } => Ok(format!("set {} = {}", pretty_key(key)?, value)),
        Fact::Entity {
            kind,
            label,
            rel,
            outbound,
        } => {
            let k = KIND_META[*kind as usize].label;
            let r = RELATION_META[*rel as usize].label;
            if *outbound {
                return Ok(format!("link: this —{r}→ {k} “{label}”"));
            }
            Ok(format!("link: {k} “{label}” —{r}→ this"))
        }
    }
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
    fn clip_caps_length() {
        assert_eq!(clip("abcdef", 3), "abc");
        assert_eq!(clip("ab", 9), "ab");
    }
}
