use egui::{Pos2, Vec2};
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::Entity;
use crate::shapes::base_radius;

pub const PACK_GAP: f32 = 8.0;
const PACK_ITERS: usize = 48;
const AXIS_EPS: f32 = 0.001;

pub fn pack_radius(e: &Entity) -> Result<f32, Error> {
    Ok(base_radius(e)? * e.size * e.boost)
}

pub fn scale_about_centroid(
    points: &[(NodeIndex, Pos2)],
    factor: f32,
) -> Result<Vec<(NodeIndex, Pos2, Pos2)>, Error> {
    if points.len() < 2 {
        return Ok(Vec::new());
    }
    let mut sum = Vec2::ZERO;
    for (_, p) in points {
        sum += p.to_vec2();
    }
    let center = (sum / points.len() as f32).to_pos2();
    Ok(points
        .iter()
        .map(|(idx, p)| (*idx, *p, center + (*p - center) * factor))
        .collect())
}

pub fn separate(steps: &mut [(NodeIndex, Pos2, Pos2)], radii: &[f32]) -> Result<(), Error> {
    let n = steps.len().min(radii.len());
    for _pass in 0..PACK_ITERS {
        if !separate_pass(steps, radii, n) {
            break;
        }
    }
    Ok(())
}

fn separate_pass(steps: &mut [(NodeIndex, Pos2, Pos2)], radii: &[f32], n: usize) -> bool {
    let mut moved = false;
    for i in 0..n {
        for j in (i + 1)..n {
            let floor = radii[i] + radii[j] + PACK_GAP;
            let delta = steps[j].2 - steps[i].2;
            let dist = delta.length();
            if dist >= floor {
                continue;
            }
            let axis = if dist > AXIS_EPS {
                delta / dist
            } else {
                Vec2::RIGHT
            };
            let push = axis * (floor - dist) * 0.5;
            steps[i].2 -= push;
            steps[j].2 += push;
            moved = true;
        }
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nix(i: usize) -> NodeIndex {
        NodeIndex::new(i)
    }

    fn st(i: usize, x: f32, y: f32) -> (NodeIndex, Pos2, Pos2) {
        (nix(i), Pos2::new(x, y), Pos2::new(x, y))
    }

    fn centroid_of(ps: impl Iterator<Item = Pos2>) -> Pos2 {
        let mut sum = Vec2::ZERO;
        let mut n = 0.0f32;
        for p in ps {
            sum += p.to_vec2();
            n += 1.0;
        }
        (sum / n).to_pos2()
    }

    fn sample() -> Vec<(NodeIndex, Pos2)> {
        vec![
            (nix(0), Pos2::new(-30.0, 10.0)),
            (nix(1), Pos2::new(20.0, 10.0)),
            (nix(2), Pos2::new(10.0, -40.0)),
        ]
    }

    #[test]
    fn loosen_grows_distances_and_holds_centroid() {
        let pts = sample();
        let f = 1.2;
        let steps = scale_about_centroid(&pts, f).unwrap();
        assert_eq!(steps.len(), pts.len());

        let before = centroid_of(pts.iter().map(|(_, p)| *p));
        let after = centroid_of(steps.iter().map(|(_, _, to)| *to));
        assert!((before - after).length() < 1e-3);

        for i in 0..steps.len() {
            for j in (i + 1)..steps.len() {
                let d_before = (steps[i].1 - steps[j].1).length();
                let d_after = (steps[i].2 - steps[j].2).length();
                assert!(d_after > d_before);
                assert!((d_after - d_before * f).abs() < 1e-2);
            }
        }
    }

    #[test]
    fn tighten_after_loosen_returns_to_origin() {
        let pts = sample();
        let f = 1.2;
        let loosened: Vec<(NodeIndex, Pos2)> = scale_about_centroid(&pts, f)
            .unwrap()
            .into_iter()
            .map(|(idx, _, to)| (idx, to))
            .collect();
        let restored = scale_about_centroid(&loosened, 1.0 / f).unwrap();
        assert_eq!(restored.len(), pts.len());
        for (i, (_, _, to)) in restored.iter().enumerate() {
            assert!((*to - pts[i].1).length() < 1e-2);
        }
    }

    #[test]
    fn fewer_than_two_nodes_is_noop() {
        assert!(scale_about_centroid(&[], 1.2).unwrap().is_empty());
        let one = vec![(nix(0), Pos2::new(3.0, 4.0))];
        assert!(scale_about_centroid(&one, 1.2).unwrap().is_empty());
    }

    #[test]
    fn overlapping_pair_stops_at_the_contact_floor() {
        let mut steps = vec![st(0, 0.0, 0.0), st(1, 5.0, 0.0)];
        separate(&mut steps, &[11.0, 11.0]).unwrap();
        let d = (steps[1].2 - steps[0].2).length();
        let floor = 22.0 + PACK_GAP;
        assert!(
            (d - floor).abs() < 0.5,
            "pair must land exactly at contact, got {d} vs {floor}"
        );
    }

    #[test]
    fn far_pair_is_untouched() {
        let mut steps = vec![st(0, 0.0, 0.0), st(1, 300.0, 0.0)];
        separate(&mut steps, &[11.0, 11.0]).unwrap();
        assert_eq!(steps[0].2, Pos2::new(0.0, 0.0));
        assert_eq!(steps[1].2, Pos2::new(300.0, 0.0));
    }

    #[test]
    fn bigger_nodes_claim_a_bigger_floor() {
        let mut steps = vec![st(0, 0.0, 0.0), st(1, 10.0, 0.0)];
        separate(&mut steps, &[11.0, 33.0]).unwrap();
        let d = (steps[1].2 - steps[0].2).length();
        assert!(d >= 44.0 + PACK_GAP - 0.5, "radii sum the floor, got {d}");
    }

    #[test]
    fn coincident_nodes_split_deterministically() {
        let mut a = vec![st(0, 50.0, 50.0), st(1, 50.0, 50.0)];
        let mut b = a.clone();
        separate(&mut a, &[11.0, 11.0]).unwrap();
        separate(&mut b, &[11.0, 11.0]).unwrap();
        assert_eq!(a, b, "same input must split the same way every time");
        let d = (a[1].2 - a[0].2).length();
        assert!(
            d >= 22.0 + PACK_GAP - 0.5,
            "coincident pair must still reach the floor"
        );
    }

    #[test]
    fn crammed_cluster_converges_with_no_overlap() {
        let mut steps: Vec<(NodeIndex, Pos2, Pos2)> = (0..5)
            .map(|i| st(i, (i as f32) * 3.0, (i as f32) * 2.0))
            .collect();
        let radii = vec![11.0; 5];
        separate(&mut steps, &radii).unwrap();
        for i in 0..steps.len() {
            for j in (i + 1)..steps.len() {
                let d = (steps[j].2 - steps[i].2).length();
                assert!(
                    d >= 22.0 + PACK_GAP - 0.5,
                    "nodes {i} and {j} still overlap after separation: {d}"
                );
            }
        }
    }

    #[test]
    fn repeated_tighten_packs_free_nodes_while_the_packed_pair_holds() {
        let floor = 22.0 + PACK_GAP;
        let mut pts = vec![
            (nix(0), Pos2::new(0.0, 0.0)),
            (nix(1), Pos2::new(floor, 0.0)),
            (nix(2), Pos2::new(600.0, 0.0)),
        ];
        let radii = vec![11.0; 3];
        let mut last_far = f32::MAX;
        for round in 0..12 {
            let mut steps = scale_about_centroid(&pts, 1.0 / 1.2).unwrap();
            separate(&mut steps, &radii).unwrap();
            for i in 0..steps.len() {
                for j in (i + 1)..steps.len() {
                    let d = (steps[j].2 - steps[i].2).length();
                    assert!(d >= floor - 0.5, "round {round}: {i}-{j} overlap at {d}");
                }
            }
            let pair = (steps[1].2 - steps[0].2).length();
            assert!(
                pair <= floor + 1.0,
                "round {round}: the packed pair drifted to {pair}"
            );
            let c = centroid_of(steps.iter().map(|(_, _, to)| *to));
            let far = (steps[2].2 - c).length();
            assert!(
                far < last_far + 0.5,
                "round {round}: the free node stopped approaching ({far} vs {last_far})"
            );
            last_far = far;
            pts = steps.into_iter().map(|(idx, _, to)| (idx, to)).collect();
        }
        let spread = (pts[2].1 - pts[0].1).length();
        assert!(
            spread < 3.0 * floor,
            "after 12 tightens everything is one compact pack: {spread}"
        );
    }
}
