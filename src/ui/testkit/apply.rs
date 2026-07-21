use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::modules::{Fact, ModuleRun};
use crate::ui::RedstringApp;

use super::graph::slug_of;
use super::harness::frame;

pub struct Applied {
    pub shape_id: String,
    pub facts: Vec<Fact>,
    pub nodes_before: usize,
    pub nodes_after: usize,
    pub edges_before: usize,
    pub edges_after: usize,
    pub status: String,
}

pub fn parse(module_ron: &str, raw: &str) -> Result<(String, Vec<Fact>), Error> {
    crate::modules::testkit::parse(module_ron, raw)
}

pub fn apply(
    app: &mut RedstringApp,
    module_ron: &str,
    module_id: &str,
    target_slug: &str,
    raw: &str,
) -> Result<Applied, Error> {
    let (shape_id, facts) = crate::modules::testkit::parse(module_ron, raw)?;
    let nodes_before = app.g.g().node_count();
    let edges_before = app.g.g().edge_count();
    let run = ModuleRun {
        run_id: app.next_run_id,
        module_id: module_id.to_owned(),
        module_label: module_id.to_owned(),
        target_slug: target_slug.to_owned(),
        target_label: target_slug.to_owned(),
        project_root: app.project.as_ref().map(|p| p.root.clone()),
        shape_id: shape_id.clone(),
        facts: facts.clone(),
        raw: raw.to_owned(),
        stderr_tail: String::new(),
        ok: true,
        cancelled: false,
        evidence_rel: String::new(),
        last: true,
    };
    app.next_run_id += 1;
    app.module_tx.send(run).map_err(|e| Error::Testkit {
        what: format!("module channel send failed: {e}"),
    })?;
    frame(app, |a, ui| a.poll_module_runs(ui))?;
    Ok(Applied {
        shape_id,
        facts,
        nodes_before,
        nodes_after: app.g.g().node_count(),
        edges_before,
        edges_after: app.g.g().edge_count(),
        status: app.status.clone(),
    })
}

pub fn apply_to(
    app: &mut RedstringApp,
    module_ron: &str,
    module_id: &str,
    target: NodeIndex,
    raw: &str,
) -> Result<Applied, Error> {
    let slug = slug_of(app, target)?;
    apply(app, module_ron, module_id, &slug, raw)
}
