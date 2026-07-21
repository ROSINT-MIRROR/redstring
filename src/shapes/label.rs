use egui::{Context, Pos2, Rect, Vec2};

use crate::error::Error;

const CLAIMS_ID: &str = "redstring_label_claims";

pub const LABEL_SIZE: f32 = 7.0;
pub const LABEL_GAP: f32 = 2.0;
pub const LABEL_WRAP: f32 = 75.0;
pub const LABEL_MAX_ROWS: usize = 2;

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
}

#[derive(Clone, Default)]
struct LabelClaims {
    frame: u64,
    live: Vec<LabelClaim>,
    prev: Vec<LabelClaim>,
}

pub fn claim_label(ctx: &Context, rect: Rect, priority: f32) -> Result<bool, Error> {
    let frame = ctx.cumulative_pass_nr();
    let win = ctx.data_mut(|d| {
        let claims = d.get_temp_mut_or_default::<LabelClaims>(egui::Id::new(CLAIMS_ID));
        if claims.frame != frame {
            claims.frame = frame;
            claims.prev = std::mem::take(&mut claims.live);
        }
        let my_idx = claims.live.len();
        claims.live.push(LabelClaim { rect, priority });
        for (k, other) in claims.prev.iter().enumerate() {
            if k == my_idx || !other.rect.intersects(rect) {
                continue;
            }
            if other.priority > priority || (other.priority == priority && k < my_idx) {
                return false;
            }
        }
        true
    });
    Ok(win)
}

pub fn claim_edge_label(
    ctx: &Context,
    rect: Rect,
    priority: f32,
    force: bool,
) -> Result<bool, Error> {
    let win = claim_label(ctx, rect, priority)?;
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
        ctx.run(egui::RawInput::default(), |ctx| {
            let mut job = egui::text::LayoutJob::simple(
                text.to_owned(),
                FontId::new(font_px, FontFamily::Monospace),
                Color32::WHITE,
                wrap,
            );
            job.halign = egui::Align::Center;
            sz = ctx.fonts_mut(|f| f.layout_job(job)).size();
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

        ctx.run(egui::RawInput::default(), |c| {
            claim_label(c, box_rect, 100.0).expect("seed a higher-priority claim");
        });

        let mut warn_win = false;
        let mut verb_win = true;
        ctx.run(egui::RawInput::default(), |c| {
            claim_label(c, box_rect, 100.0).expect("re-seed the higher-priority claim");
            warn_win = claim_edge_label(c, box_rect, 100.0, true).expect("forced warning claim");
            verb_win = claim_edge_label(c, box_rect, 0.0, false).expect("gated verb claim");
        });

        assert!(
            warn_win,
            "a forced unsourced-edge warning must always claim, even when overlapping a higher-priority box"
        );
        assert!(
            !verb_win,
            "a gated sourced-verb label overlapping a higher-priority box must be skipped"
        );
    }
}
