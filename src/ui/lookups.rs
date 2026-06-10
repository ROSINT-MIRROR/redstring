use std::time::{SystemTime, UNIX_EPOCH};

use egui::{Pos2, RichText, Vec2};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::{EntityKind, RELATION_META, Relation, Source, SourceKind, slugify};
use crate::modules::{Fact, ModuleRun, ModuleStatus, RunInput};

use super::app::RedstringApp;
use super::layout::LayoutAnim;

impl RedstringApp {

    pub fn lookup_buttons(
        &self,
        ui: &mut egui::Ui,
        kind: EntityKind,
        module_ok: bool,
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
            let label = self.registry.entries[mi].spec.label.clone();
            let resp = ui.add_enabled(
                module_ok,
                egui::Button::new(RichText::new(format!(
                    "{} {label}",
                    egui_phosphor::regular::PLAY
                ))),
            );
            if resp.clicked() {
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
            self.status = "lookup needs a valid value — see the value field".into();
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
        let Some(value) = e.module_value()? else {
            return Ok(None);
        };
        Ok(Some(RunInput {
            module_id: entry.spec.id.clone(),
            module_label: entry.spec.label.clone(),
            bin: bin.clone(),
            args: entry.spec.render_args(&value, &e.label, &e.slug, &e.attrs)?,
            env: key_env(&entry.spec.keys, &self.prefs.api_keys),
            timeout_secs: entry.spec.timeout_secs,
            shapes: entry.spec.shapes.clone(),
            target_slug: e.slug.clone(),
            target_label: e.label.clone(),
        }))
    }


    pub fn poll_module_runs(&mut self, ui: &egui::Ui) -> Result<(), Error> {
        loop {
            let Ok(run) = self.module_rx.try_recv() else {
                return Ok(());
            };
            self.module_busy = self.module_busy.saturating_sub(1);
            if !run.ok {
                self.status = format!("{} failed: {}", run.module_label, run.stderr_tail);
                continue;
            }
            self.apply_run(ui, run)?;
        }
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
                    rel,
                    outbound,
                    attrs,
                } => {
                    let nidx =
                        self.upsert_entity(label, *kind, attrs, origin, &mut spawned, &mut steps)?;
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
        self.status = format!(
            "{}: {attrs_set} attrs set, {linked} linked, {skipped} skipped — prune what's irrelevant",
            run.module_id
        );
        Ok(())
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
        kind: EntityKind,
        attrs: &[(String, String)],
        origin: Pos2,
        spawned: &mut usize,
        steps: &mut Vec<(NodeIndex, Pos2, Pos2)>,
    ) -> Result<NodeIndex, Error> {
        let slug = slugify(label)?;
        let existing = self.node_by_slug(&slug);
        let nidx = self.ensure_node(existing, label, kind, origin, spawned, steps)?;
        let Some(node) = self.g.node_mut(nidx) else {
            return Ok(nidx);
        };
        let e = node.payload_mut();
        if e.value.trim().is_empty() {
            e.value = label.to_owned();
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
        kind: EntityKind,
        origin: Pos2,
        spawned: &mut usize,
        steps: &mut Vec<(NodeIndex, Pos2, Pos2)>,
    ) -> Result<NodeIndex, Error> {
        let Some(idx) = existing else {
            let angle = 0.45 + *spawned as f32 * 0.9;
            let target = origin + Vec2::new(170.0 * angle.cos(), 170.0 * angle.sin());
            *spawned += 1;
            let idx = self.add_entity(kind, label.to_owned(), origin)?;
            steps.push((idx, origin, target));
            return Ok(idx);
        };
        Ok(idx)
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
