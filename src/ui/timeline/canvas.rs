use std::sync::Arc;

use egui::epaint::CubicBezierShape;
use egui::{
    Align2, Color32, FontId, Galley, Painter, Pos2, Rect, Response, RichText, Sense, Shape, Stroke,
    StrokeKind, Vec2, Visuals,
};

use crate::error::Error;
use crate::model::{SOURCE_TIER_META, SourceTier, now_unix, status_color};

use super::derive::{Digest, Slot};
use super::event::{Lane, TimelineEvent, Via};
use super::glyph::Glyph;
use super::scale::Ruler;
use super::state::{Fit, Focus, TimelineState};
use super::when::When;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pick {
    Event(usize),
    Background,
    Nothing,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Under {
    Mark(usize),
    Nothing,
}

struct LaneBox {
    lane: Lane,
    rect: Rect,
}

struct Row {
    glyph_end: f32,
    label_end: f32,
    last: Option<usize>,
    y: f32,
}

struct Placed {
    idx: usize,
    glyph: Glyph,
    y: f32,
    galley: Arc<Galley>,
    labelled: bool,
    clip: f32,
}

struct Span {
    start: f32,
    glyph_end: f32,
    label_end: f32,
}

struct Geo;

impl Geo {
    const GUTTER: f32 = 116.0;
    const RULER: f32 = 22.0;
    const SMALL_LANE: f32 = 30.0;
    const ROW: f32 = 20.0;
    const MIN_BAND_PX: f32 = 9.0;
    const FONT: f32 = 11.5;
    const LABEL_GAP: f32 = 6.0;
    const MIN_LABEL_PX: f32 = 28.0;
    const DIM: f32 = 0.22;
    const ZOOM_RATE: f64 = 0.0022;
    const STRING_RED: Color32 = Color32::from_rgb(214, 46, 46);
}

pub fn paint(
    ui: &mut egui::Ui,
    digest: &Digest,
    focus: &Focus,
    state: &mut TimelineState,
) -> Result<Pick, Error> {
    let rect = ui.available_rect_before_wrap();
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let plot = Rect::from_min_max(
        Pos2::new(rect.left() + Geo::GUTTER, rect.top() + Geo::RULER),
        rect.max,
    );
    fit(state, digest, focus, plot.width())?;
    navigate(ui, &resp, plot, state)?;
    let ruler = Ruler::new(plot.left(), plot.width(), state.view)?;
    let lanes = lane_boxes(state, plot)?;
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals().clone();
    backdrop(&painter, &visuals, rect, plot, &lanes, &ruler)?;
    let placed = place_all(&painter, digest, &lanes, &ruler, plot)?;
    let selected = selected_mark(digest, state)?;
    let hovered = under(&placed, resp.hover_pos())?;
    strings(&painter, digest, &placed, hovered, 0.45)?;
    strings(&painter, digest, &placed, selected, 1.0)?;
    for p in &placed {
        mark(&painter, &visuals, digest, p, focus, selected)?;
    }
    let resp = with_tooltip(resp, digest, hovered)?;
    if resp.double_clicked() {
        state.pending_fit = Some(Fit::All);
        return Ok(Pick::Nothing);
    }
    if !resp.clicked() {
        return Ok(Pick::Nothing);
    }
    Ok(match hovered {
        Under::Mark(i) => Pick::Event(i),
        Under::Nothing => Pick::Background,
    })
}

fn fit(state: &mut TimelineState, digest: &Digest, focus: &Focus, width: f32) -> Result<(), Error> {
    let Some(kind) = state.pending_fit else {
        return Ok(());
    };
    state.pending_fit = None;
    let mut lo = i64::MAX;
    let mut hi = i64::MIN;
    for ev in &digest.events {
        if !state.shows(ev.key.lane)? {
            continue;
        }
        if kind == Fit::Focus && !focus.holds(ev.key.record)? {
            continue;
        }
        lo = lo.min(ev.span.start);
        hi = hi.max(ev.span.end);
    }
    if lo > hi {
        return Ok(());
    }
    let pad = ((hi - lo) / 14).max(When::DAY * 30);
    state.view = Ruler::spanning(lo - pad, hi + pad, width)?;
    Ok(())
}

fn navigate(
    ui: &egui::Ui,
    resp: &Response,
    plot: Rect,
    state: &mut TimelineState,
) -> Result<(), Error> {
    if resp.dragged() {
        state.view.t0 -= f64::from(resp.drag_delta().x) * state.view.spp;
    }
    let Some(pointer) = resp.hover_pos() else {
        return Ok(());
    };
    let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
    state.view.t0 -= f64::from(scroll.x) * state.view.spp;
    let factor = (-f64::from(scroll.y) * Geo::ZOOM_RATE).exp() / f64::from(pinch);
    if (factor - 1.0).abs() < 1e-9 {
        return Ok(());
    }
    let at = Ruler::new(plot.left(), plot.width(), state.view)?.secs_at(pointer.x)?;
    let spp = (state.view.spp * factor).clamp(TimelineState::MIN_SPP, TimelineState::MAX_SPP);
    state.view.t0 = at - f64::from(pointer.x - plot.left()) * spp;
    state.view.spp = spp;
    Ok(())
}

fn lane_boxes(state: &TimelineState, plot: Rect) -> Result<Vec<LaneBox>, Error> {
    let mut shown = Vec::new();
    let mut small_total = 0.0;
    for lane in Lane::ALL {
        if !state.shows(lane)? {
            continue;
        }
        if lane != Lane::Event {
            small_total += Geo::SMALL_LANE;
        }
        shown.push(lane);
    }
    let event_h = (plot.height() - small_total).max(Geo::ROW + 8.0);
    let mut y = plot.top();
    let mut out = Vec::new();
    for lane in shown {
        let h = if lane == Lane::Event {
            event_h
        } else {
            Geo::SMALL_LANE
        };
        let rect = Rect::from_min_size(Pos2::new(plot.left(), y), Vec2::new(plot.width(), h));
        out.push(LaneBox { lane, rect });
        y += h;
    }
    Ok(out)
}

fn backdrop(
    painter: &Painter,
    v: &Visuals,
    rect: Rect,
    plot: Rect,
    lanes: &[LaneBox],
    ruler: &Ruler,
) -> Result<(), Error> {
    painter.rect_filled(rect, 0.0, v.extreme_bg_color);
    let rule = v.widgets.noninteractive.bg_stroke.color;
    for (i, lb) in lanes.iter().enumerate() {
        let band = Rect::from_min_max(Pos2::new(rect.left(), lb.rect.top()), lb.rect.max);
        if i % 2 == 1 {
            painter.rect_filled(band, 0.0, v.faint_bg_color);
        }
        painter.line_segment(
            [band.left_top(), band.right_top()],
            Stroke::new(1.0_f32, rule),
        );
        let at = Pos2::new(rect.left() + 8.0, lb.rect.top() + 4.0);
        let title = lb.lane.label()?.to_uppercase();
        let strong = v.strong_text_color();
        painter.text(at, Align2::LEFT_TOP, title, FontId::monospace(10.5), strong);
        let caption_at = at + Vec2::new(0.0, 13.0);
        let caption = lb.lane.caption()?;
        let weak = v.weak_text_color();
        painter.text(
            caption_at,
            Align2::LEFT_TOP,
            caption,
            FontId::proportional(10.0),
            weak,
        );
    }
    ruler_marks(painter, v, rect, plot, ruler)?;
    let edge = [
        Pos2::new(plot.left(), rect.top()),
        Pos2::new(plot.left(), rect.bottom()),
    ];
    painter.line_segment(edge, Stroke::new(1.0_f32, rule));
    Ok(())
}

fn ruler_marks(
    painter: &Painter,
    v: &Visuals,
    rect: Rect,
    plot: Rect,
    ruler: &Ruler,
) -> Result<(), Error> {
    let grid = v
        .widgets
        .noninteractive
        .bg_stroke
        .color
        .gamma_multiply(0.55);
    for t in ruler.ticks()? {
        let top = Pos2::new(t.x, rect.top() + Geo::RULER - 5.0);
        painter.line_segment(
            [top, Pos2::new(t.x, plot.bottom())],
            Stroke::new(1.0_f32, grid),
        );
        let ink = if t.major {
            v.text_color()
        } else {
            v.weak_text_color()
        };
        let at = Pos2::new(t.x + 3.0, rect.top() + 3.0);
        painter.text(
            at,
            Align2::LEFT_TOP,
            t.label,
            FontId::proportional(10.5),
            ink,
        );
    }
    let today = ruler.x(now_unix()?)?;
    if today > plot.left() && today < plot.right() {
        let path = [
            Pos2::new(today, plot.top()),
            Pos2::new(today, plot.bottom()),
        ];
        let stroke = Stroke::new(1.0_f32, v.weak_text_color());
        painter.extend(Shape::dashed_line(&path, stroke, 3.0, 3.0));
        let at = Pos2::new(today - 3.0, plot.top() + 2.0);
        let weak = v.weak_text_color();
        painter.text(
            at,
            Align2::RIGHT_TOP,
            "today",
            FontId::proportional(10.0),
            weak,
        );
    }
    Ok(())
}

fn place_all(
    painter: &Painter,
    digest: &Digest,
    lanes: &[LaneBox],
    ruler: &Ruler,
    plot: Rect,
) -> Result<Vec<Placed>, Error> {
    let mut out = Vec::new();
    for lb in lanes {
        let mut rows = rows_for(lb);
        for (idx, ev) in digest.events.iter().enumerate() {
            if ev.key.lane != lb.lane {
                continue;
            }
            let glyph = glyph_of(ev, ruler)?;
            let (gx0, gx1) = glyph.extent()?;
            if gx1 < plot.left() - 4.0 || gx0 > plot.right() + 4.0 {
                continue;
            }
            let font = FontId::proportional(Geo::FONT);
            let galley = painter.layout_no_wrap(label_text(ev)?, font, Color32::PLACEHOLDER);
            let wants_label = lb.lane == Lane::Event;
            let label_end = if wants_label {
                gx1 + Geo::LABEL_GAP + galley.size().x
            } else {
                gx1
            };
            let span = Span {
                start: gx0 - Geo::LABEL_GAP,
                glyph_end: gx1,
                label_end,
            };
            let (y, room) = pick_row(&mut rows, &mut out, &span);
            let labelled = wants_label && room;
            out.push(Placed {
                idx,
                glyph,
                y,
                galley,
                labelled,
                clip: f32::INFINITY,
            });
        }
    }
    Ok(out)
}

fn rows_for(lb: &LaneBox) -> Vec<Row> {
    let empty = |y: f32| Row {
        glyph_end: f32::NEG_INFINITY,
        label_end: f32::NEG_INFINITY,
        last: None,
        y,
    };
    if lb.lane != Lane::Event {
        return vec![empty(lb.rect.center().y + 4.0)];
    }
    let mut rows = Vec::new();
    let mut y = lb.rect.top() + Geo::ROW * 0.5 + 6.0;
    while y + Geo::ROW * 0.5 <= lb.rect.bottom() || rows.is_empty() {
        rows.push(empty(y));
        y += Geo::ROW;
    }
    rows
}

fn pick_row(rows: &mut [Row], placed: &mut [Placed], span: &Span) -> (f32, bool) {
    let next = placed.len();
    for row in rows.iter_mut() {
        if row.label_end < span.start {
            claim(row, span, next);
            return (row.y, true);
        }
    }
    for row in rows.iter_mut() {
        if row.glyph_end < span.start {
            for last in row.last.iter() {
                placed[*last].clip = span.start;
            }
            claim(row, span, next);
            return (row.y, true);
        }
    }
    let mut best = 0usize;
    for (i, row) in rows.iter().enumerate() {
        if row.glyph_end < rows[best].glyph_end {
            best = i;
        }
    }
    rows[best].glyph_end = rows[best].glyph_end.max(span.glyph_end);
    (rows[best].y, false)
}

fn claim(row: &mut Row, span: &Span, next: usize) {
    row.glyph_end = span.glyph_end;
    row.label_end = span.label_end;
    row.last = Some(next);
}

fn glyph_of(ev: &TimelineEvent, ruler: &Ruler) -> Result<Glyph, Error> {
    let x0 = ruler.x(ev.span.start)?;
    let x1 = ruler.x(ev.span.end)?;
    if ev.when.is_duration()? {
        return Ok(Glyph::Bar {
            x0,
            x1: x1.max(x0 + 2.0),
            open_start: ev.span.open_start,
            open_end: ev.span.open_end,
        });
    }
    if x1 - x0 >= Geo::MIN_BAND_PX {
        return Ok(Glyph::Band { x0, x1 });
    }
    Ok(Glyph::Dot { x: (x0 + x1) * 0.5 })
}

fn hit_rect(p: &Placed) -> Result<Rect, Error> {
    let (x0, x1) = p.glyph.extent()?;
    Ok(Rect::from_min_max(
        Pos2::new(x0 - 2.0, p.y - 8.0),
        Pos2::new(x1 + 2.0, p.y + 8.0),
    ))
}

fn anchor(p: &Placed) -> Pos2 {
    match p.glyph {
        Glyph::Dot { x } => Pos2::new(x, p.y),
        Glyph::Band { x0, x1 } | Glyph::Bar { x0, x1, .. } => Pos2::new((x0 + x1) * 0.5, p.y),
    }
}

fn label_text(ev: &TimelineEvent) -> Result<String, Error> {
    let clash = if ev.conflicts.is_empty() { "" } else { "≠ " };
    let tag = ev.footing.tag()?;
    Ok(format!("{clash}{tag}{} · {}", ev.title, ev.when.label()?))
}

fn selected_mark(digest: &Digest, state: &TimelineState) -> Result<Under, Error> {
    let Some(key) = state.selected else {
        return Ok(Under::Nothing);
    };
    Ok(match digest.slot(key)? {
        Slot::At(i) => Under::Mark(i),
        Slot::Gone => Under::Nothing,
    })
}

fn under(placed: &[Placed], pos: Option<Pos2>) -> Result<Under, Error> {
    let Some(at) = pos else {
        return Ok(Under::Nothing);
    };
    let mut best = Under::Nothing;
    let mut best_area = f32::INFINITY;
    for p in placed {
        let r = hit_rect(p)?;
        if r.contains(at) && r.area() < best_area {
            best = Under::Mark(p.idx);
            best_area = r.area();
        }
    }
    Ok(best)
}

fn mark(
    painter: &Painter,
    v: &Visuals,
    digest: &Digest,
    p: &Placed,
    focus: &Focus,
    selected: Under,
) -> Result<(), Error> {
    let ev = &digest.events[p.idx];
    let dim = !focus.is_empty()? && !focus.holds(ev.key.record)?;
    let base = ink(ev, v)?;
    let color = if dim {
        base.gamma_multiply(Geo::DIM)
    } else {
        base
    };
    p.glyph
        .draw(painter, p.y, color, ev.footing, Glyph::TAIL_PX)?;
    let (x0, x1) = p.glyph.extent()?;
    if selected == Under::Mark(p.idx) {
        let ring = Stroke::new(1.5_f32, v.selection.stroke.color);
        painter.rect_stroke(hit_rect(p)?.expand(2.0), 4.0, ring, StrokeKind::Outside);
    }
    if !ev.conflicts.is_empty() {
        let at = Pos2::new(x0 - 3.0, p.y);
        let font = FontId::proportional(12.0);
        painter.text(at, Align2::RIGHT_CENTER, "≠", font, Geo::STRING_RED);
    }
    if !p.labelled {
        return Ok(());
    }
    let text_ink = if dim {
        v.weak_text_color().gamma_multiply(0.6)
    } else {
        v.text_color()
    };
    let at = Pos2::new(x1 + Geo::LABEL_GAP, p.y - p.galley.size().y * 0.5);
    if p.clip - at.x < Geo::MIN_LABEL_PX {
        return Ok(());
    }
    let room = Rect::from_min_max(
        Pos2::new(at.x, p.y - Geo::ROW),
        Pos2::new(p.clip, p.y + Geo::ROW),
    );
    let clipped = painter.with_clip_rect(room.intersect(painter.clip_rect()));
    clipped.galley(at, p.galley.clone(), text_ink);
    Ok(())
}

fn ink(ev: &TimelineEvent, v: &Visuals) -> Result<Color32, Error> {
    if ev.key.lane == Lane::Entered {
        return Ok(v.text_color());
    }
    if let Via::Source(..) = ev.key.via {
        return tier_color(ev.tier);
    }
    status_color(ev.status)
}

fn tier_color(tier: SourceTier) -> Result<Color32, Error> {
    for m in SOURCE_TIER_META.iter() {
        if m.tier == tier {
            return Ok(m.color);
        }
    }
    Err(Error::Timeline {
        what: format!("no colour for source tier {tier:?}"),
    })
}

fn strings(
    painter: &Painter,
    digest: &Digest,
    placed: &[Placed],
    who: Under,
    strength: f32,
) -> Result<(), Error> {
    let Under::Mark(i) = who else {
        return Ok(());
    };
    let ev = &digest.events[i];
    let mut pins: Vec<(usize, f32, Pos2)> = Vec::new();
    for p in placed {
        let other = &digest.events[p.idx];
        if other.key.record == ev.key.record {
            let at = anchor(p);
            pins.push((other.key.lane.slot()?, at.x, at));
        }
    }
    pins.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let red = Geo::STRING_RED.gamma_multiply(strength);
    for pair in pins.windows(2) {
        yarn(painter, pair[0].2, pair[1].2, red);
    }
    for pin in &pins {
        painter.circle_filled(pin.2, 2.4, red);
    }
    for &c in &ev.conflicts {
        clash(painter, placed, i, c, red);
    }
    Ok(())
}

fn yarn(painter: &Painter, a: Pos2, b: Pos2, color: Color32) {
    let dy = (b.y - a.y) * 0.5;
    let sag = if dy.abs() < 4.0 {
        ((b.x - a.x).abs() * 0.18).clamp(6.0, 28.0)
    } else {
        0.0
    };
    let c1 = Pos2::new(a.x, a.y + dy + sag);
    let c2 = Pos2::new(b.x, b.y - dy + sag);
    let stroke = Stroke::new(1.6_f32, color);
    let shape =
        CubicBezierShape::from_points_stroke([a, c1, c2, b], false, Color32::TRANSPARENT, stroke);
    painter.add(shape);
}

fn clash(painter: &Painter, placed: &[Placed], a: usize, b: usize, color: Color32) {
    let mut ends: Vec<Pos2> = Vec::new();
    for p in placed {
        if p.idx == a || p.idx == b {
            ends.push(anchor(p));
        }
    }
    if ends.len() != 2 {
        return;
    }
    let path = [ends[0], ends[1]];
    painter.extend(Shape::dashed_line(
        &path,
        Stroke::new(1.3_f32, color),
        5.0,
        4.0,
    ));
    let mid = ends[0] + (ends[1] - ends[0]) * 0.5;
    let font = FontId::proportional(13.0);
    painter.text(mid, Align2::CENTER_BOTTOM, "≠", font, color);
}

fn with_tooltip(resp: Response, digest: &Digest, hovered: Under) -> Result<Response, Error> {
    let Under::Mark(i) = hovered else {
        return Ok(resp);
    };
    let ev = &digest.events[i];
    let title = format!("{}{}", ev.footing.tag()?, ev.title);
    let when = format!("{} — {}", ev.when.label()?, ev.when.note()?);
    let lane = format!("{} lane · {}", ev.key.lane.label()?, ev.key.lane.caption()?);
    let footing = format!("{} — {}", ev.footing.label()?, ev.footing.hint()?);
    let from_text = ev.read_from_text;
    let clashes = ev.conflicts.len();
    Ok(resp.on_hover_ui_at_pointer(|ui| {
        ui.strong(title);
        ui.label(when);
        ui.label(RichText::new(lane).weak());
        ui.label(footing);
        if from_text {
            ui.label(RichText::new("date read verbatim from a text value").weak());
        }
        if clashes > 0 {
            let note = format!("≠ {clashes} conflicting account(s) of this fact");
            ui.colored_label(Geo::STRING_RED, note);
        }
        let help = "click to pin · drag to pan · wheel to zoom";
        ui.label(RichText::new(help).weak().small());
    }))
}
