use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use egui::{Pos2, RichText, Vec2};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::{
    Author, EntityKind, Observation, Origin, RELATION_META, Relation, Verification, slugify,
};
use crate::modules::{Fact, ModuleRun, ModuleStatus, ObsKind, RunInput, parse_obs_value};

use super::app::{RedstringApp, RsGraph};
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
            } else if !module_ok {
                resp.on_disabled_hover_text("needs a valid value — fix the value field")
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
        crate::dlog_info!(
            "lookups",
            "job launch run_id={} module={} target={}",
            input.run_id,
            input.module_label,
            input.target_label
        );
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
            args: entry.spec.render_args(&value, &e.label, &e.slug, &e.attr_pairs()?)?,
            env: key_env(&entry.spec.keys, &self.prefs.api_keys),
            headers: entry.spec.headers.clone(),
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
                crate::dlog_warn!("lookups", "job cancelled run_id={run_id} module={}", run.module_label);
                self.jobs.retain(|j| j.run_id != run_id);
                self.status = format!(
                    "{} on {} cancelled — {} kept",
                    run.module_label, run.target_label, self.job_found(run_id)
                );
                continue;
            }
            if !run.ok {
                crate::dlog_error!("lookups", "job failed run_id={run_id} module={}: {}", run.module_label, run.stderr_tail);
                self.jobs.retain(|j| j.run_id != run_id);
                self.status = format!("{} failed: {}", run.module_label, run.stderr_tail);
                continue;
            }
            if !run.facts.is_empty() {
                self.apply_run(ui, run)?;
            } else if last {
                crate::dlog_info!("lookups", "job done run_id={run_id} found={}", self.job_found(run_id));
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
        let source = super::evidence::evidence_source(&run)?;
        if run.project_root.as_deref() != self.project.as_ref().map(|p| p.root.as_path()) {
            self.status =
                format!("{}: finished for another case — raw output archived there", run.module_id);
            return Ok(());
        }
        self.watch_current()?;
        let Some(tidx) = node_by_slug(&self.g, &run.target_slug) else {
            self.status = "lookup target no longer on the board — result dropped".into();
            return Ok(());
        };
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
                Fact::Attr { key, value, kind } => {
                    self.apply_attr_fact(
                        tidx,
                        key,
                        *kind,
                        value,
                        &run.module_id,
                        &mut attrs_set,
                        &mut skipped,
                    )?;
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
                        label, value, *kind, attrs, &run.module_id, origin, base, &mut spawned,
                        &mut steps,
                    )?;
                    if nidx == tidx {
                        skipped += 1;
                        continue;
                    }
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
                    let mut relation = Relation::stamp(
                        *rel,
                        Origin::Lookup(run.module_id.clone()),
                        Verification::MachineSuggested,
                        self.author.clone(),
                    )?;
                    relation.sources.push(source.clone());
                    self.g.add_edge_with_label(
                        a,
                        b,
                        relation,
                        RELATION_META[*rel as usize].label.into(),
                    );
                    linked += 1;
                }
            }
        }
        if self.relayout_on_add && spawned > 0 {
            self.relayout_all(ui, self.current_layout)?;
        } else {
            self.kick_spawn_anim(ui, steps)?;
        }
        let total = self.bump_job_found(run.run_id, linked);
        crate::dlog_info!(
            "lookups",
            "apply run_id={} module={} attrs={attrs_set} linked={total} skipped={skipped} spawned={spawned}",
            run.run_id,
            run.module_id
        );
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
        kind: ObsKind,
        value: &str,
        module_id: &str,
        attrs_set: &mut usize,
        skipped: &mut usize,
    ) -> Result<(), Error> {
        let author = self.author.clone();
        let Some(node) = self.g.node_mut(tidx) else {
            return Ok(());
        };
        fill_obs(
            &mut node.payload_mut().observations,
            key,
            kind,
            value,
            module_id,
            &author,
            attrs_set,
            skipped,
        )?;
        Ok(())
    }

    fn upsert_entity(
        &mut self,
        label: &str,
        value: &str,
        kind: EntityKind,
        attrs: &[(String, String, ObsKind)],
        module_id: &str,
        origin: Pos2,
        base: usize,
        spawned: &mut usize,
        steps: &mut Vec<(NodeIndex, Pos2, Pos2)>,
    ) -> Result<NodeIndex, Error> {
        let (nidx, created) = converge_or_create(&mut self.g, kind, label, value, origin)?;
        if created {
            let angle = 0.45 + (base + *spawned) as f32 * 0.9;
            let target = origin + Vec2::new(170.0 * angle.cos(), 170.0 * angle.sin());
            *spawned += 1;
            steps.push((nidx, origin, target));
        }
        let author = self.author.clone();
        let Some(node) = self.g.node_mut(nidx) else {
            return Ok(nidx);
        };
        let e = node.payload_mut();
        let mut set = 0usize;
        let mut skip = 0usize;
        for (k, v, akind) in attrs {
            fill_obs(&mut e.observations, k, *akind, v, module_id, &author, &mut set, &mut skip)?;
        }
        Ok(nidx)
    }

    fn node_pos(&self, idx: NodeIndex) -> Pos2 {
        let Some(n) = self.g.g().node_weight(idx) else {
            return Pos2::ZERO;
        };
        n.location()
    }
}

fn node_by_slug(g: &RsGraph, slug: &str) -> Option<NodeIndex> {
    g.g().node_indices().find(|i| {
        let Some(n) = g.g().node_weight(*i) else {
            return false;
        };
        n.payload().slug == slug
    })
}

fn node_by_value(g: &RsGraph, kind: EntityKind, value: &str) -> Result<Option<NodeIndex>, Error> {
    let target = slugify(value)?;
    if target.is_empty() {
        return Ok(None);
    }
    for i in g.g().node_indices() {
        let Some(n) = g.g().node_weight(i) else {
            continue;
        };
        let e = n.payload();
        if e.kind != kind {
            continue;
        }
        let cand = if e.value.trim().is_empty() {
            slugify(&e.label)?
        } else {
            slugify(&e.value)?
        };
        if cand == target {
            return Ok(Some(i));
        }
    }
    Ok(None)
}

fn converge_or_create(
    g: &mut RsGraph,
    kind: EntityKind,
    label: &str,
    value: &str,
    origin: Pos2,
) -> Result<(NodeIndex, bool), Error> {
    let Some(idx) = node_by_value(g, kind, value)? else {
        let idx = super::ops::spawn_node(g, kind, label, value, origin)?;
        return Ok((idx, true));
    };
    let Some(node) = g.node_mut(idx) else {
        return Ok((idx, false));
    };
    let e = node.payload_mut();
    if e.value.trim().is_empty() {
        e.value = value.to_owned();
    }
    Ok((idx, false))
}

fn fill_obs(
    observations: &mut Vec<Observation>,
    key: &str,
    kind: ObsKind,
    value: &str,
    module_id: &str,
    author: &Author,
    set: &mut usize,
    skipped: &mut usize,
) -> Result<(), Error> {
    let parsed = parse_obs_value(kind, value)?;
    if observations.iter().any(|o| o.name == key && o.value == parsed) {
        *skipped += 1;
        return Ok(());
    }
    if fill_empty_named_slot(observations, key, &parsed, module_id, set)? {
        return Ok(());
    }
    if curated_name_owner(observations, key)? {
        *skipped += 1;
        return Ok(());
    }
    observations.push(Observation::stamp(
        key.to_owned(),
        parsed,
        Origin::Lookup(module_id.to_owned()),
        Verification::MachineSuggested,
        author.clone(),
    )?);
    *set += 1;
    Ok(())
}

fn fill_empty_named_slot(
    observations: &mut [Observation],
    key: &str,
    parsed: &crate::model::ObsValue,
    module_id: &str,
    set: &mut usize,
) -> Result<bool, Error> {
    for i in 0..observations.len() {
        if observations[i].name != key || !observations[i].value.text()?.trim().is_empty() {
            continue;
        }
        observations[i].value = parsed.clone();
        observations[i].origin = Origin::Lookup(module_id.to_owned());
        observations[i].status = Verification::MachineSuggested;
        *set += 1;
        return Ok(true);
    }
    Ok(false)
}

fn curated_name_owner(observations: &[Observation], key: &str) -> Result<bool, Error> {
    for o in observations {
        if o.name != key || matches!(o.origin, Origin::Lookup(_)) {
            continue;
        }
        if !o.value.text()?.trim().is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
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

#[cfg(test)]
mod tests {
    use egui_graphs::Graph;
    use petgraph::stable_graph::StableGraph;

    use super::*;
    use crate::model::{DatePrecision, DateVal, Entity, ObsValue};
    use crate::ui::testkit::nodes;

    fn domain_node(slug: &str, label: &str, value: &str) -> Entity {
        nodes::entity(EntityKind::Domain, slug, label, value)
    }

    fn put(g: &mut RsGraph, e: Entity) -> NodeIndex {
        let label = e.label.clone();
        g.add_node_with_label_and_location(e, label, Pos2::ZERO)
    }

    #[test]
    fn converges_on_value_despite_stale_slug() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let e = put(&mut g, domain_node("domain", "evil.com", ""));
        assert_eq!(node_by_slug(&g, "domain"), Some(e));
        assert_eq!(node_by_value(&g, EntityKind::Domain, "evil.com").unwrap(), Some(e));
        let before = g.g().node_count();
        let (nidx, created) =
            converge_or_create(&mut g, EntityKind::Domain, "evil.com", "evil.com", Pos2::ZERO)
                .unwrap();
        assert_eq!(nidx, e);
        assert!(!created);
        assert_eq!(g.g().node_count(), before);
    }

    #[test]
    fn distinct_san_creates_exactly_one_node() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let e = put(&mut g, domain_node("domain", "evil.com", ""));
        let (a, created_a) =
            converge_or_create(&mut g, EntityKind::Domain, "evil.com", "evil.com", Pos2::ZERO)
                .unwrap();
        assert_eq!(a, e);
        assert!(!created_a);
        assert_eq!(g.g().node_count(), 1);
        let (b, created_b) = converge_or_create(
            &mut g,
            EntityKind::Domain,
            "www.evil.com",
            "www.evil.com",
            Pos2::ZERO,
        )
        .unwrap();
        assert_ne!(b, e);
        assert!(created_b);
        assert_eq!(g.g().node_count(), 2);
    }

    #[test]
    fn different_kind_same_value_does_not_converge() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        put(&mut g, domain_node("domain", "evil.com", ""));
        assert_eq!(node_by_value(&g, EntityKind::IpAddress, "evil.com").unwrap(), None);
    }

    #[test]
    fn created_nodes_that_slugify_alike_get_unique_slugs() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let (a, created_a) =
            converge_or_create(&mut g, EntityKind::Domain, "Evil Com", "a.example", Pos2::ZERO)
                .unwrap();
        let (b, created_b) =
            converge_or_create(&mut g, EntityKind::Domain, "evil-com", "b.example", Pos2::ZERO)
                .unwrap();
        assert!(created_a && created_b);
        assert_ne!(a, b);
        let sa = g.node(a).unwrap().payload().slug.clone();
        let sb = g.node(b).unwrap().payload().slug.clone();
        assert_eq!(sa, "evil_com");
        assert_eq!(sb, "evil_com_2");
    }

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
    fn fill_obs_never_clobbers() {
        let author = Author::default();
        let mut obs = vec![
            Observation::stamp(
                "country".to_owned(),
                ObsValue::Text("AU".to_owned()),
                Origin::Manual,
                Verification::AnalystAsserted,
                author.clone(),
            )
            .unwrap(),
            Observation::stamp(
                "city".to_owned(),
                ObsValue::Text(String::new()),
                Origin::Manual,
                Verification::AnalystAsserted,
                author.clone(),
            )
            .unwrap(),
        ];
        let (mut set, mut skip) = (0usize, 0usize);
        fill_obs(&mut obs, "country", ObsKind::Text, "US", "mod", &author, &mut set, &mut skip).unwrap();
        fill_obs(&mut obs, "city", ObsKind::Text, "Brisbane", "mod", &author, &mut set, &mut skip).unwrap();
        fill_obs(&mut obs, "region", ObsKind::Text, "QLD", "mod", &author, &mut set, &mut skip).unwrap();
        assert_eq!(obs[0].value.text().unwrap(), "AU", "existing value untouched");
        assert_eq!(obs[1].value.text().unwrap(), "Brisbane", "empty filled");
        assert_eq!(obs[2].name, "region", "missing appended");
        assert_eq!(obs[2].value.text().unwrap(), "QLD");
        assert_eq!((set, skip), (2, 1));
    }

    #[test]
    fn fill_obs_parses_declared_kind_and_degrades() {
        let author = Author::default();
        let mut obs = Vec::new();
        let (mut set, mut skip) = (0usize, 0usize);
        fill_obs(&mut obs, "expires", ObsKind::Date, "2000-01-01T00:00:00Z", "whois", &author, &mut set, &mut skip).unwrap();
        fill_obs(&mut obs, "revenue", ObsKind::Number, "180199672", "propublica", &author, &mut set, &mut skip).unwrap();
        fill_obs(&mut obs, "when", ObsKind::Date, "sometime last spring", "m", &author, &mut set, &mut skip).unwrap();
        assert_eq!(
            obs[0].value,
            ObsValue::Date(DateVal { secs: 946_684_800, precision: DatePrecision::Day }),
            "a parseable date stamps a typed Date"
        );
        assert_eq!(obs[1].value, ObsValue::Number(180199672.0), "revenue stamps a typed Number");
        assert_eq!(
            obs[2].value,
            ObsValue::Text("sometime last spring".to_owned()),
            "a bad date degrades to Text, never dropped"
        );
        assert_eq!((set, skip), (3, 0));
    }

    #[test]
    fn fill_obs_fans_multi_value_into_distinct_same_name_observations() {
        let author = Author::default();
        let mut obs = Vec::new();
        let (mut set, mut skip) = (0usize, 0usize);
        for tag in ["sanction", "role.pep", "crime"] {
            fill_obs(&mut obs, "topics", ObsKind::Text, tag, "opensanctions", &author, &mut set, &mut skip)
                .unwrap();
        }
        let topics: Vec<String> =
            obs.iter().filter(|o| o.name == "topics").map(|o| o.value.text().unwrap()).collect();
        assert_eq!(
            topics,
            vec!["sanction", "role.pep", "crime"],
            "every tag lands as its own observation under the shared name — none dropped"
        );
        assert_eq!((set, skip), (3, 0));
        for o in &obs {
            assert_eq!(o.origin, Origin::Lookup("opensanctions".to_owned()));
            assert_eq!(o.status, Verification::MachineSuggested);
        }
        for tag in ["sanction", "role.pep", "crime"] {
            fill_obs(&mut obs, "topics", ObsKind::Text, tag, "opensanctions", &author, &mut set, &mut skip)
                .unwrap();
        }
        assert_eq!(obs.len(), 3, "re-applying the same lookup spawns no duplicate observations");
        assert_eq!((set, skip), (3, 3), "the second pass is entirely deduped on name+value");
    }

    #[test]
    fn fill_obs_multi_value_defers_to_a_human_owned_name() {
        let author = Author::default();
        let mut obs = vec![
            Observation::stamp(
                "topics".to_owned(),
                ObsValue::Text("hand-curated".to_owned()),
                Origin::Manual,
                Verification::AnalystAsserted,
                author.clone(),
            )
            .unwrap(),
        ];
        let (mut set, mut skip) = (0usize, 0usize);
        for tag in ["sanction", "role.pep", "crime"] {
            fill_obs(&mut obs, "topics", ObsKind::Text, tag, "opensanctions", &author, &mut set, &mut skip)
                .unwrap();
        }
        assert_eq!(obs.len(), 1, "a human-curated attr name is never fanned over by machine values");
        assert_eq!(obs[0].value.text().unwrap(), "hand-curated");
        assert_eq!(obs[0].origin, Origin::Manual, "the human observation stays untouched");
        assert_eq!((set, skip), (0, 3), "all three machine tags defer to the human owner");
    }
}
