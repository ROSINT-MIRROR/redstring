use egui::{Pos2, Rect};

use crate::error::Error;

const MIN_CLUSTER: usize = 4;
const OUTLIER_PCTL: f32 = 0.90;
const OUTLIER_SLACK: f32 = 1.35;

pub fn cluster_bounds(pts: &[(Pos2, f32)]) -> Result<Rect, Error> {
    if pts.is_empty() {
        return Err(Error::Render {
            what: "cluster bounds: no visible nodes".to_owned(),
        });
    }
    if pts.len() < MIN_CLUSTER {
        return Ok(bounds_of(pts));
    }
    let mut xs: Vec<f32> = pts.iter().map(|(p, _)| p.x).collect();
    let mut ys: Vec<f32> = pts.iter().map(|(p, _)| p.y).collect();
    let center = Pos2::new(median(&mut xs), median(&mut ys));
    let mut dists: Vec<f32> = pts.iter().map(|(p, _)| (*p - center).length()).collect();
    let cut = percentile(&mut dists, OUTLIER_PCTL) * OUTLIER_SLACK;
    let kept: Vec<(Pos2, f32)> = pts
        .iter()
        .copied()
        .filter(|(p, _)| (*p - center).length() <= cut)
        .collect();
    if kept.is_empty() {
        return Ok(bounds_of(pts));
    }
    Ok(bounds_of(&kept))
}

fn bounds_of(pts: &[(Pos2, f32)]) -> Rect {
    let mut min = Pos2::new(f32::INFINITY, f32::INFINITY);
    let mut max = Pos2::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (p, half) in pts {
        min.x = min.x.min(p.x - half);
        min.y = min.y.min(p.y - half);
        max.x = max.x.max(p.x + half);
        max.y = max.y.max(p.y + half);
    }
    Rect::from_min_max(min, max)
}

fn median(vals: &mut [f32]) -> f32 {
    if vals.is_empty() {
        return 0.0;
    }
    vals.sort_by(f32::total_cmp);
    let mid = vals.len() / 2;
    if vals.len() % 2 == 1 {
        vals[mid]
    } else {
        (vals[mid - 1] + vals[mid]) * 0.5
    }
}

fn percentile(vals: &mut [f32], p: f32) -> f32 {
    if vals.is_empty() {
        return 0.0;
    }
    vals.sort_by(f32::total_cmp);
    let clamped = p.clamp(0.0, 1.0);
    let idx = ((clamped * (vals.len() as f32 - 1.0)).round() as usize).min(vals.len() - 1);
    vals[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f32, y: f32) -> (Pos2, f32) {
        (Pos2::new(x, y), 5.0)
    }

    #[test]
    fn empty_is_error() {
        assert!(cluster_bounds(&[]).is_err(), "no nodes must be an error, not a bogus rect");
    }

    #[test]
    fn single_node_bounds_that_node() {
        let r = cluster_bounds(&[pt(100.0, 40.0)]).unwrap();
        assert!(r.contains(Pos2::new(100.0, 40.0)));
        assert!(r.width() >= 10.0 && r.height() >= 10.0, "half-extent must inflate the box");
    }

    #[test]
    fn outlier_is_dropped_from_bounds() {
        let mut pts: Vec<(Pos2, f32)> = Vec::new();
        for i in 0..19 {
            let a = i as f32 * 0.33;
            pts.push(pt(200.0 + a.cos() * 30.0, 200.0 + a.sin() * 30.0));
        }
        pts.push(pt(5000.0, 5000.0));
        let clamped = cluster_bounds(&pts).unwrap();
        let full = bounds_of(&pts);
        assert!(
            clamped.max.x < 400.0 && clamped.max.y < 400.0,
            "the far outlier must be excluded from the fit box, got {clamped:?}"
        );
        assert!(
            full.max.x > 4000.0,
            "sanity: the full box would include the outlier, got {full:?}"
        );
    }

    #[test]
    fn tight_cluster_keeps_everyone() {
        let mut pts: Vec<(Pos2, f32)> = Vec::new();
        for i in 0..12 {
            let a = i as f32 / 12.0 * std::f32::consts::TAU;
            pts.push(pt(a.cos() * 50.0, a.sin() * 50.0));
        }
        let r = cluster_bounds(&pts).unwrap();
        assert!(
            r.width() > 90.0 && r.height() > 90.0,
            "a spread with no outliers must keep its full extent, got {r:?}"
        );
    }
}
