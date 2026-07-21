use std::collections::HashSet;

use egui::RichText;
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::{RELATION_META, SOURCE_META, SOURCE_TIER_META, Source};

use super::app::{ALARM_RED, OK_GREEN, RedstringApp, RsGraph};
use super::graphalg::{PathHop, path_between};
use super::style;

pub fn section(app: &mut RedstringApp, ui: &mut egui::Ui, sel: &[NodeIndex]) -> Result<(), Error> {
    if path_matches_selection(app, sel) {
        return render(app, ui);
    }
    forget_path(app)?;
    if sel.len() != 2 {
        return Ok(());
    }
    ui.add_space(style::GAP);
    ui.separator();
    style::section(ui, egui_phosphor::regular::SHARE_NETWORK, "path finder")?;
    ui.label(
        RichText::new("trace how these two entities connect — with the source behind every hop")
            .weak()
            .small(),
    );
    if style::action_btn(ui, egui_phosphor::regular::PATH, "find path")? {
        find(app, sel[0], sel[1])?;
    }
    Ok(())
}

fn path_matches_selection(app: &RedstringApp, sel: &[NodeIndex]) -> bool {
    let Some(hops) = app.active_path.as_ref() else {
        return false;
    };
    let mut want: HashSet<NodeIndex> = HashSet::new();
    for h in hops {
        want.insert(h.from);
        want.insert(h.to);
    }
    let have: HashSet<NodeIndex> = sel.iter().copied().collect();
    want == have
}

fn forget_path(app: &mut RedstringApp) -> Result<(), Error> {
    let Some(hops) = app.active_path.take() else {
        return Ok(());
    };
    for h in &hops {
        let Some(e) = app.g.edge_mut(h.edge) else {
            continue;
        };
        e.set_selected(false);
    }
    Ok(())
}

fn find(app: &mut RedstringApp, a: NodeIndex, b: NodeIndex) -> Result<(), Error> {
    let Some(hops) = path_between(&app.g, a, b)? else {
        app.status = format!(
            "no path — {} and {} are not connected",
            app.node_label(a)?,
            app.node_label(b)?
        );
        return Ok(());
    };
    app.clear_selection()?;
    highlight(app, &hops)?;
    let weak = count_unsourced(&app.g, &hops)?;
    app.status = summary(hops.len(), weak);
    app.active_path = Some(hops);
    Ok(())
}

fn highlight(app: &mut RedstringApp, hops: &[PathHop]) -> Result<(), Error> {
    let mut nodes: HashSet<NodeIndex> = HashSet::new();
    for h in hops {
        nodes.insert(h.from);
        nodes.insert(h.to);
        let Some(e) = app.g.edge_mut(h.edge) else {
            continue;
        };
        e.set_selected(true);
    }
    for idx in nodes {
        let Some(n) = app.g.node_mut(idx) else {
            continue;
        };
        n.set_selected(true);
    }
    Ok(())
}

fn count_unsourced(g: &RsGraph, hops: &[PathHop]) -> Result<usize, Error> {
    let mut weak = 0usize;
    for h in hops {
        let Some(e) = g.edge(h.edge) else {
            continue;
        };
        if e.payload().sources.is_empty() {
            weak += 1;
        }
    }
    Ok(weak)
}

fn summary(hops: usize, weak: usize) -> String {
    if weak == 0 {
        return format!("path found — {hops} hop(s), every hop sourced");
    }
    format!("path found — {hops} hop(s), {weak} unsourced — unproven")
}

fn render(app: &mut RedstringApp, ui: &mut egui::Ui) -> Result<(), Error> {
    let Some(hops) = app.active_path.clone() else {
        return Ok(());
    };
    ui.add_space(style::GAP);
    ui.separator();
    style::section(ui, egui_phosphor::regular::SHARE_NETWORK, "path trail")?;

    let weak = count_unsourced(&app.g, &hops)?;
    weakest_banner(ui, hops.len(), weak)?;
    ui.add_space(style::GAP_SM);

    for (i, h) in hops.iter().enumerate() {
        hop_row(app, ui, i, h)?;
    }

    ui.add_space(style::GAP);
    if style::action_btn(ui, egui_phosphor::regular::X, "clear path")? {
        app.active_path = None;
        app.clear_selection()?;
    }
    Ok(())
}

fn weakest_banner(ui: &mut egui::Ui, hops: usize, weak: usize) -> Result<(), Error> {
    if weak == 0 {
        ui.colored_label(
            OK_GREEN,
            RichText::new(format!(
                "{} {hops} hop(s) · fully sourced",
                egui_phosphor::regular::CHECK
            ))
            .strong(),
        );
        return Ok(());
    }
    ui.colored_label(
        ALARM_RED,
        RichText::new(format!(
            "{} {hops} hop(s) · {weak} unsourced — weakest link unproven",
            egui_phosphor::regular::WARNING_CIRCLE
        ))
        .strong(),
    );
    Ok(())
}

fn hop_row(app: &RedstringApp, ui: &mut egui::Ui, i: usize, h: &PathHop) -> Result<(), Error> {
    let Some(edge) = app.g.edge(h.edge) else {
        return Ok(());
    };
    let Some((src, _dst)) = app.g.g().edge_endpoints(h.edge) else {
        return Ok(());
    };
    let r = edge.payload();
    let verb = RELATION_META[r.kind as usize].label;
    let forward = src == h.from;
    let from_l = app.node_label(h.from)?;
    let to_l = app.node_label(h.to)?;
    let arrow = if forward {
        format!("—{verb}→")
    } else {
        format!("←{verb}—")
    };
    ui.group(|ui| -> Result<(), Error> {
        ui.label(RichText::new(format!("{}.  {from_l}", i + 1)).small().weak());
        ui.label(RichText::new(format!("{arrow}  {to_l}")).strong());
        hop_sources(ui, &r.sources)?;
        Ok(())
    })
    .inner
}

fn hop_sources(ui: &mut egui::Ui, sources: &[Source]) -> Result<(), Error> {
    if sources.is_empty() {
        ui.colored_label(
            ALARM_RED,
            RichText::new(format!(
                "{} UNSOURCED — unproven hop",
                egui_phosphor::regular::WARNING_CIRCLE
            ))
            .small(),
        );
        return Ok(());
    }
    ui.colored_label(
        OK_GREEN,
        RichText::new(format!(
            "{} {} source(s)",
            egui_phosphor::regular::PAPERCLIP,
            sources.len()
        ))
        .small(),
    );
    for s in sources {
        source_line(ui, s)?;
    }
    Ok(())
}

fn source_line(ui: &mut egui::Ui, s: &Source) -> Result<(), Error> {
    let kind = SOURCE_META[s.kind as usize].label;
    let tier = &SOURCE_TIER_META[s.tier as usize];
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("• {kind}")).weak().small());
        ui.label(RichText::new(tier.label).color(tier.color).small())
            .on_hover_text("credibility tier");
        if !s.reference.is_empty() {
            ui.label(RichText::new(s.reference.as_str()).monospace().small())
                .on_hover_text(s.reference.as_str());
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use egui::Pos2;
    use egui_graphs::Graph;
    use petgraph::graph::NodeIndex;
    use petgraph::stable_graph::StableGraph;

    use super::super::app::RsGraph;
    use super::super::graphalg::path_between;
    use super::{count_unsourced, summary};
    use crate::model::{Author, Origin, Relation, RelationKind, Source, Verification};
    use crate::ui::testkit::nodes::person;

    fn spawn(g: &mut RsGraph, name: &str) -> NodeIndex {
        g.add_node_with_label_and_location(person(name), name.into(), Pos2::ZERO)
    }

    fn link(g: &mut RsGraph, a: NodeIndex, b: NodeIndex, sourced: bool) {
        let mut r = Relation::stamp(
            RelationKind::Controls,
            Origin::Manual,
            Verification::Unreviewed,
            Author::default(),
        )
        .unwrap();
        if sourced {
            r.sources.push(Source {
                reference: "notes/p.md".to_owned(),
                ..Default::default()
            });
        }
        g.add_edge_with_label(a, b, r, "controls".into());
    }

    #[test]
    fn count_unsourced_counts_the_weak_hops() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let b = spawn(&mut g, "b");
        let c = spawn(&mut g, "c");
        link(&mut g, a, b, false);
        link(&mut g, b, c, true);
        let hops = path_between(&g, a, c).unwrap().expect("a reaches c");
        assert_eq!(count_unsourced(&g, &hops).unwrap(), 1);
    }

    #[test]
    fn count_unsourced_is_zero_for_a_fully_sourced_path() {
        let mut g: RsGraph = Graph::from(&StableGraph::default());
        let a = spawn(&mut g, "a");
        let b = spawn(&mut g, "b");
        link(&mut g, a, b, true);
        let hops = path_between(&g, a, b).unwrap().expect("a reaches b");
        assert_eq!(count_unsourced(&g, &hops).unwrap(), 0);
    }

    #[test]
    fn summary_marks_unproven_when_a_hop_is_weak() {
        assert!(summary(3, 0).contains("every hop sourced"));
        assert!(summary(3, 1).contains("unproven"));
        assert!(summary(4, 2).contains("2 unsourced"));
    }
}
