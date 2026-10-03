use egui::{Color32, Context, Pos2, Rect, Vec2};
use egui_graphs::DrawContext;

use crate::error::Error;

const CLAIMS_ID: &str = "redstring_label_claims";
const RELATION_LABELS_ID: &str = "redstring_relation_labels";

pub const PINNED_EDGE_LABEL_MIN_ZOOM: f32 = 0.75;

pub const LABEL_SIZE: f32 = 5.5;
pub const LABEL_GAP: f32 = 2.0;
pub const LABEL_WRAP: f32 = 60.0;
pub const LABEL_MAX_ROWS: usize = 2;
pub const LABEL_MIN_PX: f32 = 8.5;
pub const LABEL_ICON_SCALE: f32 = 1.15;

pub const MAX_FONT_PX: f32 = 256.0;

pub const BADGE_CENTER_FRAC: f32 = 0.72;

const BACKDROP_PAD_FRAC: f32 = 0.12;
const BACKDROP_PAD_MIN_PX: f32 = 1.0;

const BADGE_RADIUS_FRAC: f32 = 0.5;
const IMG_BADGE_RADIUS_FRAC: f32 = 0.24;
const BADGE_R_MIN: f32 = 3.0;
const BADGE_R_MAX: f32 = 40.0;

pub fn label_pad_px(font_px: f32) -> Result<f32, Error> {
    Ok((font_px * BACKDROP_PAD_FRAC).max(BACKDROP_PAD_MIN_PX))
}

pub fn badge_radius(half: f32) -> Result<f32, Error> {
    Ok((half * BADGE_RADIUS_FRAC).clamp(BADGE_R_MIN, BADGE_R_MAX))
}

pub fn badge_radius_img(min_side: f32) -> Result<f32, Error> {
    Ok((min_side * IMG_BADGE_RADIUS_FRAC).clamp(BADGE_R_MIN, BADGE_R_MAX))
}

pub fn theme_shift(bg: Color32, dark: bool, mix: f32) -> Result<Color32, Error> {
    let toward = if dark { Color32::WHITE } else { Color32::BLACK };
    Ok(bg.lerp_to_gamma(toward, mix))
}

#[derive(Clone, Copy)]
pub enum LabelAnchor {
    Disc,
    Tile,
}

pub fn label_box(
    anchor: LabelAnchor,
    center: Pos2,
    half: f32,
    galley_size: Vec2,
    gap: f32,
    font_px: f32,
) -> Result<Rect, Error> {
    let pad = label_pad_px(font_px)?;
    let clear_y = match anchor {
        LabelAnchor::Disc => center.y + half * BADGE_CENTER_FRAC + badge_radius(half)?,
        LabelAnchor::Tile => center.y + half + badge_radius_img(half * 2.0)?,
    };
    let text_top = clear_y + gap + pad;
    let min = Pos2::new(center.x - galley_size.x / 2.0, text_top);
    Ok(Rect::from_min_size(min, galley_size).expand(pad))
}

#[derive(Clone, Copy)]
struct LabelClaim {
    rect: Rect,
    priority: f32,
    id: u64,
}

#[derive(Clone, Default)]
struct LabelClaims {
    frame: u64,
    live: Vec<LabelClaim>,
    prev: Vec<LabelClaim>,
}

pub fn claim_label(ctx: &Context, id: u64, rect: Rect, priority: f32) -> Result<bool, Error> {
    let frame = ctx.cumulative_pass_nr();
    let win = ctx.data_mut(|d| {
        let claims = d.get_temp_mut_or_default::<LabelClaims>(egui::Id::new(CLAIMS_ID));
        if claims.frame != frame {
            claims.frame = frame;
            claims.prev = std::mem::take(&mut claims.live);
        }
        claims.live.push(LabelClaim { rect, priority, id });
        for other in claims.prev.iter() {
            if other.id == id || !other.rect.intersects(rect) {
                continue;
            }
            if other.priority > priority || (other.priority == priority && other.id < id) {
                return false;
            }
        }
        true
    });
    Ok(win)
}

pub fn set_relation_labels(ctx: &Context, on: bool) -> Result<(), Error> {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(RELATION_LABELS_ID), on));
    Ok(())
}

pub fn relation_labels(ctx: &Context) -> Result<bool, Error> {
    let Some(on) = ctx.data(|d| d.get_temp::<bool>(egui::Id::new(RELATION_LABELS_ID))) else {
        return Ok(false);
    };
    Ok(on)
}

pub fn pinned_labels(ctx: &DrawContext) -> Result<bool, Error> {
    Ok(relation_labels(ctx.ctx)? && ctx.meta.zoom >= PINNED_EDGE_LABEL_MIN_ZOOM)
}

pub fn claim_edge_label(
    ctx: &Context,
    id: u64,
    rect: Rect,
    priority: f32,
    force: bool,
) -> Result<bool, Error> {
    let win = claim_label(ctx, id, rect, priority)?;
    Ok(force || win)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::{IMG_RADIUS, NODE_RADIUS};
    use egui::{Color32, FontFamily, FontId};

    fn galley_size_wrapped(text: &str, font_px: f32, wrap: f32) -> Vec2 {
        let ctx = egui::Context::default();
        let mut sz = Vec2::ZERO;
        let _full = ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut job = egui::text::LayoutJob::simple(
                text.to_owned(),
                FontId::new(font_px, FontFamily::Monospace),
                Color32::WHITE,
                wrap,
            );
            job.halign = egui::Align::Center;
            sz = ui.ctx().fonts_mut(|f| f.layout_job(job)).size();
        });
        sz
    }

    #[test]
    fn disc_label_clears_disc_and_occupation_badge() {
        let font_px = LABEL_SIZE;
        let size = galley_size_wrapped("J. Doe", font_px, LABEL_WRAP);
        let center = Pos2::new(100.0, 100.0);
        let half = NODE_RADIUS;
        let gap = LABEL_GAP;
        let boxr =
            label_box(LabelAnchor::Disc, center, half, size, gap, font_px).expect("label box");

        let disc = Rect::from_center_size(center, Vec2::splat(half * 2.0));
        let badge_off = half * BADGE_CENTER_FRAC;
        let br = badge_radius(half).expect("badge radius");
        let occ =
            Rect::from_center_size(center + Vec2::new(-badge_off, badge_off), Vec2::splat(br * 2.0));

        assert!(
            boxr.top() >= disc.bottom(),
            "label box top {} must be at/below disc bottom {}",
            boxr.top(),
            disc.bottom()
        );
        assert!(
            boxr.top() >= occ.bottom(),
            "label box top {} must be at/below occupation badge bottom {}",
            boxr.top(),
            occ.bottom()
        );
        assert!(
            !boxr.intersects(occ),
            "label box {boxr:?} must not intersect occupation badge {occ:?}"
        );
        let breathing = boxr.top() - occ.bottom();
        assert!(
            (breathing - gap).abs() < 1e-3,
            "breathing room {breathing} below the badge must equal LABEL_GAP {gap}"
        );
    }

    fn disc_and_label_px(zoom: f32) -> (f32, f32) {
        use crate::model::{DEFAULT_NODE_SIZE, Entity, EntityKind, Relation};
        use crate::shapes::EntityNodeShape;
        use egui::Shape;
        use egui_graphs::{DisplayNode, DrawContext, MetadataFrame, Node, SettingsStyle};
        use petgraph::{Directed, stable_graph::DefaultIx};

        let mut node = Node::<Entity, Relation, Directed, DefaultIx, EntityNodeShape>::new(Entity {
            slug: "n".into(),
            kind: EntityKind::Person,
            label: "Node Label".into(),
            size: DEFAULT_NODE_SIZE,
            ..Default::default()
        });
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
        let (mut disc, mut font) = (0.0f32, 0.0f32);
        let _full = ctx.run_ui(input, |ui| {
            let c = ui.ctx();
            let painter = c.debug_painter();
            let style = SettingsStyle::default();
            let mut meta = MetadataFrame::default();
            meta.zoom = zoom;
            let dctx = DrawContext {
                ctx: c,
                painter: &painter,
                style: &style,
                is_directed: true,
                meta: &meta,
            };
            let shapes =
                <EntityNodeShape as DisplayNode<Entity, Relation, Directed, DefaultIx>>::shapes(
                    node.display_mut(),
                    &dctx,
                );
            for s in &shapes {
                match s {
                    Shape::Circle(cs) => disc = disc.max(cs.radius * 2.0),
                    Shape::Text(t) if t.galley.text() == "Node Label" => {
                        font = t.galley.job.sections[0].format.font_id.size;
                    }
                    _ => {}
                }
            }
        });
        (disc, font)
    }

    #[test]
    fn label_stays_about_a_quarter_of_its_disc_at_every_zoom() {
        for zoom in [0.8f32, 1.0, 1.5, 2.5] {
            let (disc, font) = disc_and_label_px(zoom);
            assert!(font > 0.0, "a default-size label must still draw at zoom {zoom}");
            let ratio = font / disc;
            assert!(
                (0.2..=0.26).contains(&ratio),
                "label {font}px against a {disc}px disc at zoom {zoom} is ratio {ratio}: the name must not outweigh its icon"
            );
        }
    }

    #[test]
    fn portraits_and_labels_keep_their_proportions() {
        let disc = NODE_RADIUS * 2.0;
        let tile = IMG_RADIUS * 2.0;
        assert!(tile >= disc, "a portrait must never render smaller than a plain icon disc");
        assert!(
            LABEL_SIZE / tile <= 0.2,
            "a portrait's name must not crowd the portrait: {LABEL_SIZE} on a {tile} tile"
        );
        assert!(
            LABEL_ICON_SCALE <= 1.25,
            "the kind glyph beside a portrait's name is an inline marker, not a second avatar"
        );
        let chars_per_line = LABEL_WRAP / (LABEL_SIZE * 0.6);
        assert!(
            (16.0..=20.0).contains(&chars_per_line),
            "the wrap width tracks the font so a line still holds ~18 monospace characters, got {chars_per_line}"
        );
        let lod_zoom = LABEL_MIN_PX / (LABEL_SIZE * crate::model::DEFAULT_NODE_SIZE);
        assert!(
            lod_zoom < 0.8,
            "default-size labels must stay drawn down to at least 0.8 zoom, they vanish below {lod_zoom}"
        );
    }

    #[test]
    fn label_padding_is_tight() {
        assert!((BACKDROP_PAD_FRAC - 0.12).abs() < 1e-6);
        assert!((BACKDROP_PAD_MIN_PX - 1.0).abs() < 1e-6);
        assert!((label_pad_px(LABEL_SIZE).unwrap() - 1.0).abs() < 1e-6);
        assert!((label_pad_px(40.0).unwrap() - 40.0 * BACKDROP_PAD_FRAC).abs() < 1e-6);
    }

    #[test]
    fn tile_label_clears_tile_badge() {
        let font_px = LABEL_SIZE;
        let size = galley_size_wrapped("Doe", font_px, LABEL_WRAP);
        let center = Pos2::new(50.0, 50.0);
        let half = IMG_RADIUS;
        let gap = LABEL_GAP;
        let boxr =
            label_box(LabelAnchor::Tile, center, half, size, gap, font_px).expect("label box");

        let tile = Rect::from_center_size(center, Vec2::splat(half * 2.0));
        let br = badge_radius_img(half * 2.0).expect("badge radius img");
        let occ = Rect::from_center_size(tile.left_bottom(), Vec2::splat(br * 2.0));

        assert!(boxr.top() >= tile.bottom(), "tile label must clear tile bottom");
        assert!(boxr.top() >= occ.bottom(), "tile label must clear image badge bottom");
        assert!(!boxr.intersects(occ), "tile label must not intersect image badge");
    }

    #[test]
    fn label_zone_and_draw_share_geometry() {
        let (size, zoom, boost) = (1.3f32, 1.7f32, 1.0f32);
        let font_px = (LABEL_SIZE * size * zoom).min(MAX_FONT_PX);
        let gsz = galley_size_wrapped("Jane Q. Public", font_px, LABEL_WRAP * size * zoom);
        let center = Pos2::new(200.0, 140.0);

        let node_half = NODE_RADIUS * size * zoom;
        let node_gap = LABEL_GAP * size * zoom;
        let node_fpx = (LABEL_SIZE * size * zoom).min(MAX_FONT_PX);
        let node_box =
            label_box(LabelAnchor::Disc, center, node_half, gsz, node_gap, node_fpx).expect("node");

        let vsize = size * boost;
        let zone_half = NODE_RADIUS * vsize * zoom;
        let zone_gap = LABEL_GAP * vsize * zoom;
        let zone_fpx = (LABEL_SIZE * vsize * zoom).min(MAX_FONT_PX);
        let zone_box =
            label_box(LabelAnchor::Disc, center, zone_half, gsz, zone_gap, zone_fpx).expect("zone");

        assert_eq!(
            node_box, zone_box,
            "node.rs draw and links.rs label_zone must derive an identical label box"
        );
    }

    #[test]
    fn edge_label_force_claims_and_gated_verb_yields() {
        let ctx = egui::Context::default();
        let box_rect = Rect::from_center_size(Pos2::new(100.0, 100.0), Vec2::splat(40.0));

        let _full = ctx.run_ui(egui::RawInput::default(), |ui| {
            claim_label(ui.ctx(), 1, box_rect, 100.0).expect("seed a higher-priority claim");
        });

        let mut warn_win = false;
        let mut verb_win = true;
        let _full = ctx.run_ui(egui::RawInput::default(), |ui| {
            let c = ui.ctx();
            claim_label(c, 1, box_rect, 100.0).expect("re-seed the higher-priority claim");
            warn_win = claim_edge_label(c, 2, box_rect, 100.0, true).expect("forced warning claim");
            verb_win = claim_edge_label(c, 3, box_rect, 0.0, false).expect("gated verb claim");
        });

        assert!(
            warn_win,
            "a forced unsourced-edge warning must always claim, even when overlapping a higher-priority box"
        );
        assert!(
            !verb_win,
            "a gated sourced-verb label overlapping a higher-priority box must be skipped"
        );

        let far = Rect::from_center_size(Pos2::new(400.0, 400.0), Vec2::splat(40.0));
        let edge = 2u64 | (1u64 << 63);
        let mut forced_alone = false;
        let _full = ctx.run_ui(egui::RawInput::default(), |ui| {
            forced_alone =
                claim_edge_label(ui.ctx(), edge, far, 1000.0, true).expect("forced claim, clear spot");
        });
        assert!(forced_alone, "a forced edge claim in clear space must draw");

        let mut node_win = true;
        let _full = ctx.run_ui(egui::RawInput::default(), |ui| {
            node_win =
                claim_label(ui.ctx(), 9, far, 5.0).expect("node label over the forced edge claim");
        });
        assert!(
            !node_win,
            "a lower-priority node label overlapping the previous frame's FORCED edge claim must yield: force overrides the draw decision, never the registration"
        );
    }

    #[test]
    fn moving_edge_claim_self_recognizes_across_frames() {
        use crate::model::{
            Author, Entity, EntityKind, Origin, Relation, RelationKind, Source, SourceKind,
            Verification,
        };
        use crate::shapes::{EntityNodeShape, SourcedEdgeShape};
        use crate::ui::testkit::nodes;
        use egui_graphs::{DisplayEdge, DrawContext, EdgeProps, MetadataFrame, Node, SettingsStyle};
        use egui_phosphor::regular::PAPERCLIP;
        use petgraph::{Directed, stable_graph::DefaultIx};

        SourcedEdgeShape::set_shot_labels(true).expect("enable always-labels");
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
        let mut es = SourcedEdgeShape::from(EdgeProps {
            payload: Relation {
                kind: RelationKind::Controls,
                origin: Origin::default(),
                status: Verification::Unreviewed,
                confidence: 100,
                author: Author::default(),
                created_at: 0,
                sources: vec![Source {
                    kind: SourceKind::Note,
                    reference: "n".into(),
                    ..Default::default()
                }],
                valid_from: None,
                valid_to: None,
                sealed: false,
                hidden: false,
            },
            order: 0,
            selected: false,
            label: String::new(),
        });
        let n1 = Node::<Entity, Relation, Directed, DefaultIx, EntityNodeShape>::new(
            nodes::entity(EntityKind::Person, "from_slug", "A", ""),
        );
        let mut n2 = Node::<Entity, Relation, Directed, DefaultIx, EntityNodeShape>::new(
            nodes::entity(EntityKind::Person, "to_slug", "B", ""),
        );
        for x in [200.0f32, 214.0] {
            n2.set_location(Pos2::new(x, 6.0));
            let mut drawn = false;
            let _full = ctx.run_ui(input.clone(), |ui| {
                let c = ui.ctx();
                let painter = c.debug_painter();
                let style = SettingsStyle::default();
                let meta = MetadataFrame::default();
                let dctx = DrawContext {
                    ctx: c,
                    painter: &painter,
                    style: &style,
                    is_directed: true,
                    meta: &meta,
                };
                for s in &es.shapes(&n1, &n2, &dctx) {
                    if let egui::Shape::Text(t) = s {
                        if t.galley.text().contains(PAPERCLIP) {
                            drawn = true;
                        }
                    }
                }
            });
            assert!(
                drawn,
                "a gated sourced label must stay drawn while its endpoint moves (x={x}): the slug-keyed claim id must self-recognize the previous frame's claim instead of losing to its own ghost"
            );
        }
        SourcedEdgeShape::set_shot_labels(false).expect("reset always-labels");
    }

    #[test]
    fn disjoint_edge_claim_does_not_shift_node_label_decisions() {
        let ctx = egui::Context::default();
        let r = Rect::from_center_size(Pos2::new(100.0, 100.0), Vec2::splat(40.0));
        let far = Rect::from_center_size(Pos2::new(1000.0, 1000.0), Vec2::splat(40.0));
        let edge = 1u64 << 63;

        let _full = ctx.run_ui(egui::RawInput::default(), |ui| {
            let c = ui.ctx();
            claim_label(c, 1, r, 5.0).expect("seed A");
            claim_label(c, 2, r, 5.0).expect("seed B");
        });

        let mut a_win = false;
        let mut b_win = true;
        let _full = ctx.run_ui(egui::RawInput::default(), |ui| {
            let c = ui.ctx();
            a_win = claim_label(c, 1, r, 5.0).expect("A baseline");
            b_win = claim_label(c, 2, r, 5.0).expect("B baseline");
        });
        assert!(a_win, "baseline: equal-priority lower-id A must win its overlap claim");
        assert!(!b_win, "baseline: equal-priority higher-id B must yield to A");

        let mut a_win2 = false;
        let mut b_win2 = true;
        let _full = ctx.run_ui(egui::RawInput::default(), |ui| {
            let c = ui.ctx();
            claim_label(c, edge, far, 1000.0).expect("a disjoint high-priority edge claim, pushed first");
            a_win2 = claim_label(c, 1, r, 5.0).expect("A with the edge claim present");
            b_win2 = claim_label(c, 2, r, 5.0).expect("B with the edge claim present");
        });
        assert!(
            a_win2,
            "A must STILL win: a disjoint hover-inserted edge claim shifts push indices but not stable ids"
        );
        assert!(
            !b_win2,
            "B must STILL yield: the A/B declutter outcome is invariant to the inserted-first disjoint claim"
        );
    }

    fn edge_texts(sourced: bool, pinned: bool, zoom: f32) -> Vec<String> {
        use crate::model::{
            Author, Entity, EntityKind, Origin, Relation, RelationKind, Source, SourceKind,
            Verification,
        };
        use crate::shapes::{EntityNodeShape, SourcedEdgeShape};
        use crate::ui::testkit::nodes;
        use egui_graphs::{DisplayEdge, DrawContext, EdgeProps, MetadataFrame, Node, SettingsStyle};
        use petgraph::{Directed, stable_graph::DefaultIx};

        let sources = if sourced {
            vec![Source { kind: SourceKind::Note, reference: "n".into(), ..Default::default() }]
        } else {
            Vec::new()
        };
        let mut es = SourcedEdgeShape::from(EdgeProps {
            payload: Relation {
                kind: RelationKind::Controls,
                origin: Origin::default(),
                status: Verification::AnalystAsserted,
                confidence: 100,
                author: Author::default(),
                created_at: 0,
                sources,
                valid_from: None,
                valid_to: None,
                sealed: false,
                hidden: false,
            },
            order: 0,
            selected: false,
            label: String::new(),
        });
        let mut n1 = Node::<Entity, Relation, Directed, DefaultIx, EntityNodeShape>::new(
            nodes::entity(EntityKind::Person, "from_slug", "A", ""),
        );
        let mut n2 = Node::<Entity, Relation, Directed, DefaultIx, EntityNodeShape>::new(
            nodes::entity(EntityKind::Person, "to_slug", "B", ""),
        );
        n1.set_location(Pos2::new(40.0, 200.0));
        n2.set_location(Pos2::new(400.0, 200.0));
        let ctx = egui::Context::default();
        set_relation_labels(&ctx, pinned).expect("set the view flag");
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
        let mut out = Vec::new();
        let _full = ctx.run_ui(input, |ui| {
            let c = ui.ctx();
            let painter = c.debug_painter();
            let style = SettingsStyle::default();
            let mut meta = MetadataFrame::default();
            meta.zoom = zoom;
            let dctx = DrawContext {
                ctx: c,
                painter: &painter,
                style: &style,
                is_directed: true,
                meta: &meta,
            };
            for s in &es.shapes(&n1, &n2, &dctx) {
                if let egui::Shape::Text(t) = s {
                    out.push(t.galley.text().to_owned());
                }
            }
        });
        out
    }

    fn shows(texts: &[String], needle: &str) -> bool {
        texts.iter().any(|t| t.contains(needle))
    }

    #[test]
    fn show_relation_labels_pins_sourced_names_at_readable_zooms() {
        use egui_phosphor::regular::PAPERCLIP;
        let off = edge_texts(true, false, 1.0);
        assert!(!shows(&off, "controls"), "toggle off: a sourced label waits for hover, got {off:?}");
        for zoom in [PINNED_EDGE_LABEL_MIN_ZOOM, 1.0, 2.0] {
            let on = edge_texts(true, true, zoom);
            assert!(
                shows(&on, "controls") && shows(&on, PAPERCLIP),
                "toggle on: the sourced verb and its source count stay visible at zoom {zoom}, got {on:?}"
            );
        }
        let far = edge_texts(true, true, 0.5);
        assert!(
            !shows(&far, "controls"),
            "zoomed far out the pinned labels step back instead of burying the board, got {far:?}"
        );
    }

    #[test]
    fn unsourced_warnings_ignore_the_relation_label_toggle() {
        use egui_phosphor::regular::WARNING;
        for pinned in [false, true] {
            for zoom in [0.5f32, 1.0] {
                let texts = edge_texts(false, pinned, zoom);
                assert!(
                    shows(&texts, WARNING),
                    "an unsourced edge always warns (pinned {pinned}, zoom {zoom}), got {texts:?}"
                );
            }
        }
    }

    #[test]
    fn screenshot_labels_and_the_view_toggle_stay_separate() {
        use crate::shapes::SourcedEdgeShape;
        SourcedEdgeShape::set_shot_labels(true).expect("enable shot labels");
        let shot = edge_texts(true, false, 1.0);
        SourcedEdgeShape::set_shot_labels(false).expect("reset shot labels");
        assert!(shows(&shot, "controls"), "the --shot flag still labels without the toggle, got {shot:?}");
        let ctx = egui::Context::default();
        assert!(!relation_labels(&ctx).expect("read"), "a fresh window starts with labels on hover only");
        set_relation_labels(&ctx, true).expect("set");
        assert!(relation_labels(&ctx).expect("read"), "the toggle lives on the window context");
        assert!(
            !relation_labels(&egui::Context::default()).expect("read"),
            "the flag never leaks into another context, such as the offscreen --shot renderer"
        );
    }

    #[test]
    fn the_board_publishes_the_relation_label_preference() {
        use crate::ui::testkit::harness;
        let mut app = harness::app().expect("app");
        app.prefs.relation_labels = true;
        let on = harness::frame(&mut app, |a, ui| {
            a.board(ui)?;
            relation_labels(ui.ctx())
        })
        .expect("board frame");
        assert!(on, "drawing the board must hand the preference to the edge renderer");
        app.prefs.relation_labels = false;
        let off = harness::frame(&mut app, |a, ui| {
            a.board(ui)?;
            relation_labels(ui.ctx())
        })
        .expect("board frame");
        assert!(!off, "turning the preference off must reach the renderer too");
    }
}
