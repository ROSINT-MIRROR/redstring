use egui::epaint::TextShape;
use egui::text::LayoutJob;
use egui::{Align, FontFamily, FontId, Id, LayerId, Order, Pos2, Shape};
use egui_graphs::DrawContext;

use crate::error::Error;

use super::badge::{backdrop, badge_fg};
use super::label::{
    LABEL_GAP, LABEL_MIN_PX, LABEL_SIZE, LabelAnchor, MAX_FONT_PX, label_box, label_pad_px,
};

const REVEAL_LAYER: &str = "redstring_label_reveal";
const REVEAL_WRAP_EMS: f32 = 24.0;

const ELIDE_CHARS: usize = 18;
const HEAD_CHARS: usize = 8;
const TAIL_CHARS: usize = 6;

pub fn display_label(raw: &str) -> Result<String, Error> {
    let trimmed = raw.trim();
    let count = trimmed.chars().count();
    let spaced = trimmed.chars().any(|c| c.is_whitespace());
    if count > ELIDE_CHARS && !spaced {
        return middle_truncate(trimmed, HEAD_CHARS, TAIL_CHARS);
    }
    Ok(trimmed.to_owned())
}

fn middle_truncate(s: &str, head: usize, tail: usize) -> Result<String, Error> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= head + tail + 1 {
        return Ok(s.to_owned());
    }
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(chars[chars.len() - tail..].iter());
    Ok(out)
}

pub fn reveal_label(
    ctx: &DrawContext,
    anchor: LabelAnchor,
    center: Pos2,
    half: f32,
    size: f32,
    full: &str,
) -> Result<(), Error> {
    if full.is_empty() {
        return Ok(());
    }
    let fpx = ctx
        .meta
        .canvas_to_screen_size(LABEL_SIZE * size)
        .clamp(LABEL_MIN_PX, MAX_FONT_PX);
    let bg = ctx.ctx.global_style().visuals.panel_fill;
    let dark = ctx.ctx.global_style().visuals.dark_mode;
    let ink = badge_fg(bg)?;
    let mut job = LayoutJob::simple(
        full.to_owned(),
        FontId::new(fpx, FontFamily::Monospace),
        ink,
        fpx * REVEAL_WRAP_EMS,
    );
    job.halign = Align::Center;
    let galley = ctx.ctx.fonts_mut(|f| f.layout_job(job));
    let gap = ctx
        .meta
        .canvas_to_screen_size(LABEL_GAP * size)
        .max(LABEL_GAP);
    let rect = label_box(anchor, center, half, galley.size(), gap, fpx)?;
    let pad = label_pad_px(fpx)?;
    let mut res: Vec<Shape> = Vec::with_capacity(2);
    backdrop(&mut res, rect, bg, dark)?;
    res.push(TextShape::new(Pos2::new(center.x, rect.min.y + pad), galley, ink).into());
    ctx.ctx
        .layer_painter(LayerId::new(Order::Foreground, Id::new(REVEAL_LAYER)))
        .with_clip_rect(ctx.painter.clip_rect())
        .extend(res);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DEFAULT_NODE_SIZE, Entity, EntityKind, Relation};
    use crate::shapes::EntityNodeShape;
    use egui::{Rect, Vec2};
    use egui_graphs::{DisplayNode, MetadataFrame, Node, SettingsStyle};
    use petgraph::{Directed, stable_graph::DefaultIx};

    const LONG_TOKEN: &str = "vodafone-romania-holdings.com";
    const SHORT_LABEL: &str = "Node Label";

    type BoardNode = Node<Entity, Relation, Directed, DefaultIx, EntityNodeShape>;

    fn node_at(slug: &str, label: &str, size: f32, pos: Pos2) -> BoardNode {
        let mut n = BoardNode::new(Entity {
            slug: slug.into(),
            kind: EntityKind::Person,
            label: label.into(),
            size,
            ..Default::default()
        });
        n.set_location(pos);
        n
    }

    struct Frame {
        node_texts: Vec<Vec<String>>,
        layer_texts: Vec<(String, Pos2)>,
    }

    impl Frame {
        fn drawn(&self, node: usize, text: &str) -> usize {
            self.node_texts[node].iter().filter(|t| t.as_str() == text).count()
        }

        fn elided(&self, node: usize) -> usize {
            self.node_texts[node].iter().filter(|t| t.contains('…')).count()
        }

        fn revealed(&self, text: &str) -> usize {
            self.layer_texts.iter().filter(|(t, _)| t.as_str() == text).count()
        }

        fn reveal_pos(&self, text: &str) -> Pos2 {
            let hit = self.layer_texts.iter().find(|(t, _)| t.as_str() == text);
            hit.map(|(_, p)| *p).expect("the reveal must have reached the overlay layer")
        }
    }

    fn draw(ctx: &egui::Context, nodes: &mut [BoardNode]) -> Frame {
        draw_with(ctx, nodes, MetadataFrame::default())
    }

    fn draw_with(ctx: &egui::Context, nodes: &mut [BoardNode], meta: MetadataFrame) -> Frame {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
        let mut node_texts: Vec<Vec<String>> = Vec::new();
        let full = ctx.run_ui(input, |ui| {
            let c = ui.ctx();
            let painter = c.debug_painter();
            let style = SettingsStyle::default();
            let dctx = DrawContext {
                ctx: c,
                painter: &painter,
                style: &style,
                is_directed: true,
                meta: &meta,
            };
            for n in nodes.iter_mut() {
                let props = n.props().clone();
                <EntityNodeShape as DisplayNode<Entity, Relation, Directed, DefaultIx>>::update(
                    n.display_mut(),
                    &props,
                );
                let shapes =
                    <EntityNodeShape as DisplayNode<Entity, Relation, Directed, DefaultIx>>::shapes(
                        n.display_mut(),
                        &dctx,
                    );
                let mut texts = Vec::new();
                for s in &shapes {
                    if let Shape::Text(t) = s {
                        texts.push(t.galley.text().to_owned());
                    }
                }
                node_texts.push(texts);
            }
        });
        let mut layer_texts = Vec::new();
        for clipped in &full.shapes {
            if let Shape::Text(t) = &clipped.shape {
                layer_texts.push((t.galley.text().to_owned(), t.pos));
            }
        }
        Frame {
            node_texts,
            layer_texts,
        }
    }

    #[test]
    fn long_token_label_is_middle_truncated() {
        let raw = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
        let shown = display_label(raw).expect("display label");
        assert!(
            shown.chars().count() < raw.chars().count(),
            "a long single-token label must be shortened, got {shown:?}"
        );
        assert!(shown.contains('…'), "middle truncation must insert an ellipsis, got {shown:?}");
        assert!(shown.starts_with("bc1qxy2"), "the head must stay visible, got {shown:?}");
        assert!(shown.ends_with("wlh"), "the tail must stay visible, got {shown:?}");
        assert!(
            shown.chars().count() <= HEAD_CHARS + TAIL_CHARS + 1,
            "the truncated label must fit roughly one line, got {shown:?}"
        );
        assert_eq!(
            display_label("Meridian Holdings Ltd").expect("display label"),
            "Meridian Holdings Ltd",
            "multi-word prose labels must not be middle-truncated"
        );
    }

    #[test]
    fn hovering_a_truncated_label_reveals_the_full_text_once() {
        let ctx = egui::Context::default();
        let mut nodes = [node_at("n", LONG_TOKEN, DEFAULT_NODE_SIZE, Pos2::new(60.0, 60.0))];
        let cold = draw(&ctx, &mut nodes);
        assert_eq!(
            cold.elided(0),
            1,
            "unhovered, a long token must draw its middle-truncated form, got {:?}",
            cold.node_texts
        );
        assert_eq!(
            cold.revealed(LONG_TOKEN),
            0,
            "unhovered, nothing may be revealed on the overlay layer, got {:?}",
            cold.layer_texts
        );

        nodes[0].set_hovered(true);
        let hot = draw(&ctx, &mut nodes);
        assert_eq!(
            hot.revealed(LONG_TOKEN),
            1,
            "hovering a truncated label must reveal the full text exactly once, got {:?}",
            hot.layer_texts
        );
        assert_eq!(
            hot.elided(0),
            0,
            "the truncated form must give way to the reveal, never draw both, got {:?}",
            hot.node_texts
        );
    }

    #[test]
    fn the_reveal_rides_zoom_and_pan() {
        let ctx = egui::Context::default();
        let spot = Pos2::new(40.0, 30.0);
        let mut nodes = [node_at("n", LONG_TOKEN, DEFAULT_NODE_SIZE, spot)];
        nodes[0].set_hovered(true);

        let home = draw(&ctx, &mut nodes);
        let mut moved = MetadataFrame::default();
        moved.zoom = 2.0;
        moved.pan = Vec2::new(120.0, -45.0);
        let screen = moved.canvas_to_screen_pos(spot);
        let away = draw_with(&ctx, &mut nodes, moved);

        let at_home = home.reveal_pos(LONG_TOKEN);
        let at_away = away.reveal_pos(LONG_TOKEN);
        assert!(
            (at_home.x - spot.x).abs() < 0.5,
            "unzoomed, the reveal must be centered on the node, got {at_home:?} for {spot:?}"
        );
        assert!(
            (at_away.x - screen.x).abs() < 0.5,
            "zoomed and panned, the reveal must follow the node to {screen:?}, got {at_away:?}"
        );
        assert!(
            at_away.y > screen.y && at_home.y > spot.y,
            "the reveal must sit below its node in both views, got {at_home:?} / {at_away:?}"
        );
    }

    #[test]
    fn hovering_an_untruncated_label_changes_nothing() {
        let ctx = egui::Context::default();
        let mut nodes = [node_at("n", SHORT_LABEL, DEFAULT_NODE_SIZE, Pos2::new(60.0, 60.0))];
        let cold = draw(&ctx, &mut nodes);
        nodes[0].set_hovered(true);
        let hot = draw(&ctx, &mut nodes);

        assert_eq!(
            hot.node_texts, cold.node_texts,
            "a label that already reads in full must draw identically when hovered"
        );
        assert_eq!(
            hot.drawn(0, SHORT_LABEL),
            1,
            "the label must be drawn exactly once, got {:?}",
            hot.node_texts
        );
        assert_eq!(
            hot.revealed(SHORT_LABEL),
            0,
            "no overlay reveal may be added for an untruncated, unsuppressed label, got {:?}",
            hot.layer_texts
        );
    }

    #[test]
    fn hovering_below_the_lod_floor_reveals_the_label() {
        let ctx = egui::Context::default();
        let tiny = (LABEL_MIN_PX * 0.5) / LABEL_SIZE;
        let mut nodes = [node_at("n", SHORT_LABEL, tiny, Pos2::new(60.0, 60.0))];
        let cold = draw(&ctx, &mut nodes);
        assert_eq!(
            cold.drawn(0, SHORT_LABEL),
            0,
            "below the LOD floor an unhovered label stays dropped, got {:?}",
            cold.node_texts
        );
        assert_eq!(cold.revealed(SHORT_LABEL), 0, "and nothing reaches the overlay layer");

        nodes[0].set_hovered(true);
        let hot = draw(&ctx, &mut nodes);
        assert_eq!(
            hot.revealed(SHORT_LABEL),
            1,
            "hovering a node whose label the LOD dropped must reveal it, got {:?}",
            hot.layer_texts
        );
    }

    #[test]
    fn hovering_a_decluttered_label_brings_it_back() {
        let ctx = egui::Context::default();
        let size = (LABEL_MIN_PX * 2.0) / LABEL_SIZE;
        let spot = Pos2::new(60.0, 60.0);
        let mut nodes = [
            node_at("n1", SHORT_LABEL, size, spot),
            node_at("n2", SHORT_LABEL, size, spot),
        ];
        draw(&ctx, &mut nodes);
        let settled = draw(&ctx, &mut nodes);
        let winner = settled.drawn(0, SHORT_LABEL);
        assert_eq!(
            winner + settled.drawn(1, SHORT_LABEL),
            1,
            "two stacked labels must declutter to one, got {:?}",
            settled.node_texts
        );
        let loser = if winner == 1 { 1 } else { 0 };

        nodes[loser].set_hovered(true);
        draw(&ctx, &mut nodes);
        let hovered = draw(&ctx, &mut nodes);
        assert_eq!(
            hovered.drawn(loser, SHORT_LABEL) + hovered.revealed(SHORT_LABEL),
            1,
            "the hovered node must win the claim registry and show its label, got {:?} / {:?}",
            hovered.node_texts,
            hovered.layer_texts
        );
        assert_eq!(
            hovered.drawn(1 - loser, SHORT_LABEL),
            0,
            "the label it used to lose to must yield to the hovered one, got {:?}",
            hovered.node_texts
        );
    }
}
