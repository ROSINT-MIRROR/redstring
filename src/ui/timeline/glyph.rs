use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2};

use crate::error::Error;

use super::event::Footing;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Glyph {
    Dot {
        x: f32,
    },
    Band {
        x0: f32,
        x1: f32,
    },
    Bar {
        x0: f32,
        x1: f32,
        open_start: bool,
        open_end: bool,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sample {
    Dot,
    Band,
    Bar,
    OpenBar,
}

impl Glyph {
    pub const DOT: f32 = 4.5;
    pub const TAIL_PX: f32 = 44.0;
    const BAND_H: f32 = 12.0;
    const BAR_W: f32 = 5.0;
    const SAMPLE_TAIL: f32 = 18.0;

    pub fn extent(self) -> Result<(f32, f32), Error> {
        Ok(match self {
            Glyph::Dot { x } => (x - Glyph::DOT, x + Glyph::DOT),
            Glyph::Band { x0, x1 } => (x0, x1),
            Glyph::Bar {
                x0,
                x1,
                open_start,
                open_end,
            } => {
                let left = if open_start { x0 - Glyph::TAIL_PX } else { x0 };
                let right = if open_end { x1 + Glyph::TAIL_PX } else { x1 };
                (left, right)
            }
        })
    }

    pub fn draw(
        self,
        painter: &Painter,
        y: f32,
        color: Color32,
        footing: Footing,
        tail: f32,
    ) -> Result<(), Error> {
        match self {
            Glyph::Dot { x } => dot(painter, Pos2::new(x, y), color, footing),
            Glyph::Band { x0, x1 } => {
                let half = Glyph::BAND_H * 0.5;
                let r = Rect::from_min_max(Pos2::new(x0, y - half), Pos2::new(x1, y + half));
                band(painter, r, color, footing);
            }
            Glyph::Bar {
                x0,
                x1,
                open_start,
                open_end,
            } => {
                bar(painter, (x0, x1), y, color, footing);
                caps(painter, (x0, x1), y, color, (open_start, open_end), tail);
            }
        }
        if footing == Footing::Rejected {
            let (x0, x1) = self.extent()?;
            let strike = Stroke::new(1.2_f32, color);
            painter.line_segment(
                [Pos2::new(x0 - 2.0, y + 5.0), Pos2::new(x1 + 2.0, y - 5.0)],
                strike,
            );
        }
        Ok(())
    }
}

pub fn sample(
    ui: &mut egui::Ui,
    shape: Sample,
    footing: Footing,
    color: Color32,
) -> Result<(), Error> {
    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(46.0, 14.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let glyph = match shape {
        Sample::Dot => Glyph::Dot { x: rect.center().x },
        Sample::Band => Glyph::Band {
            x0: rect.left() + 6.0,
            x1: rect.right() - 6.0,
        },
        Sample::Bar => Glyph::Bar {
            x0: rect.left() + 4.0,
            x1: rect.right() - 4.0,
            open_start: false,
            open_end: false,
        },
        Sample::OpenBar => Glyph::Bar {
            x0: rect.left() + 4.0,
            x1: rect.center().x,
            open_start: false,
            open_end: true,
        },
    };
    glyph.draw(
        &painter,
        rect.center().y,
        color,
        footing,
        Glyph::SAMPLE_TAIL,
    )
}

fn dot(painter: &Painter, c: Pos2, color: Color32, footing: Footing) {
    match footing {
        Footing::Corroborated | Footing::Confirmed => {
            painter.circle_filled(c, Glyph::DOT, color);
        }
        Footing::Sourced => {
            painter.circle_filled(c, Glyph::DOT, color.gamma_multiply(0.45));
            painter.circle_stroke(c, Glyph::DOT, Stroke::new(1.5_f32, color));
        }
        Footing::Unproven => dashed_ring(painter, c, Glyph::DOT + 0.5, color),
        Footing::Disputed => {
            painter.circle_stroke(c, Glyph::DOT, Stroke::new(1.5_f32, color));
            painter.circle_filled(c, 1.3, color);
        }
        Footing::Rejected => {
            let faint = Stroke::new(1.0_f32, color.gamma_multiply(0.6));
            painter.circle_stroke(c, Glyph::DOT, faint);
        }
    }
}

fn band(painter: &Painter, r: Rect, color: Color32, footing: Footing) {
    match footing {
        Footing::Corroborated | Footing::Confirmed => {
            painter.rect_filled(r, 3.0, color.gamma_multiply(0.55));
        }
        Footing::Sourced => {
            painter.rect_filled(r, 3.0, color.gamma_multiply(0.22));
            painter.rect_stroke(r, 3.0, Stroke::new(1.2_f32, color), StrokeKind::Inside);
        }
        Footing::Unproven => dashed_rect(painter, r, color),
        Footing::Disputed => {
            painter.rect_stroke(r, 3.0, Stroke::new(1.2_f32, color), StrokeKind::Inside);
        }
        Footing::Rejected => {
            let faint = Stroke::new(1.0_f32, color.gamma_multiply(0.5));
            painter.rect_stroke(r, 3.0, faint, StrokeKind::Inside);
        }
    }
}

fn bar(painter: &Painter, (x0, x1): (f32, f32), y: f32, color: Color32, footing: Footing) {
    let path = [Pos2::new(x0, y), Pos2::new(x1, y)];
    match footing {
        Footing::Corroborated | Footing::Confirmed => {
            painter.line_segment(path, Stroke::new(Glyph::BAR_W, color));
        }
        Footing::Sourced => {
            let tinted = Stroke::new(Glyph::BAR_W, color.gamma_multiply(0.5));
            painter.line_segment(path, tinted);
        }
        Footing::Unproven => {
            let dashes = Shape::dashed_line(&path, Stroke::new(3.0_f32, color), 6.0, 4.0);
            painter.extend(dashes);
        }
        Footing::Disputed | Footing::Rejected => {
            painter.line_segment(path, Stroke::new(1.5_f32, color));
        }
    }
}

fn caps(
    painter: &Painter,
    (x0, x1): (f32, f32),
    y: f32,
    color: Color32,
    (open_start, open_end): (bool, bool),
    tail: f32,
) {
    let tick = Stroke::new(1.5_f32, color);
    if open_start {
        open_tail(painter, x0, -tail, y, color);
    } else {
        painter.line_segment([Pos2::new(x0, y - 6.0), Pos2::new(x0, y + 6.0)], tick);
    }
    if open_end {
        open_tail(painter, x1, tail, y, color);
    } else {
        painter.line_segment([Pos2::new(x1, y - 6.0), Pos2::new(x1, y + 6.0)], tick);
    }
}

fn open_tail(painter: &Painter, from: f32, run: f32, y: f32, color: Color32) {
    let mut alpha = 0.7;
    let mut at = from;
    let step = run / 4.0;
    for _seg in 0..4 {
        let next = at + step * 0.6;
        let fading = Stroke::new(2.0_f32, color.gamma_multiply(alpha));
        painter.line_segment([Pos2::new(at, y), Pos2::new(next, y)], fading);
        at += step;
        alpha *= 0.6;
    }
    let dots_at = Pos2::new(from + run, y - 1.0);
    let align = if run < 0.0 {
        Align2::RIGHT_CENTER
    } else {
        Align2::LEFT_CENTER
    };
    painter.text(dots_at, align, "…", FontId::proportional(11.0), color);
}

fn dashed_ring(painter: &Painter, c: Pos2, r: f32, color: Color32) {
    let step = std::f32::consts::TAU / 10.0;
    let mut a = 0.0f32;
    for _seg in 0..5 {
        let p0 = c + Vec2::angled(a) * r;
        let p1 = c + Vec2::angled(a + step) * r;
        painter.line_segment([p0, p1], Stroke::new(1.4_f32, color));
        a += step * 2.0;
    }
}

fn dashed_rect(painter: &Painter, r: Rect, color: Color32) {
    let path = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
        r.left_top(),
    ];
    let dashes = Shape::dashed_line(&path, Stroke::new(1.2_f32, color), 4.0, 3.0);
    painter.extend(dashes);
}
