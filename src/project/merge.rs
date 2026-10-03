use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::error::Error;
use crate::model::{CaseDisplay, EntityRec, Observation, RelationRec};

use super::state::{CaseState, note_on_disk};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Local,
    Disk,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resolve {
    Ask,
    Keep(Side),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Clash {
    CaseName,
    Head(String),
    Note(String),
    Obs { slug: String, name: String },
    Rel { from: String, to: String, kind: String },
    Gone { slug: String, deleted_by: Side },
}

#[derive(Clone, Debug)]
pub struct Merged {
    pub state: CaseState,
    pub clashes: Vec<Clash>,
    pub dropped: Vec<String>,
}

#[derive(Clone, Serialize)]
struct Agg {
    head: EntityRec,
    note: String,
    obs: Vec<Observation>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Way {
    Local,
    Disk,
    Clash,
}

type ObsKey = (String, usize);
type RelKey = (String, String, String, i64, usize);

struct Ctx {
    resolve: Resolve,
    clashes: Vec<Clash>,
}

impl Ctx {
    fn clash(&mut self, clash: Clash) -> Side {
        if !self.clashes.contains(&clash) {
            self.clashes.push(clash);
        }
        self.winner()
    }

    fn winner(&self) -> Side {
        match self.resolve {
            Resolve::Ask | Resolve::Keep(Side::Local) => Side::Local,
            Resolve::Keep(Side::Disk) => Side::Disk,
        }
    }
}

fn side_of<T: Clone>(side: Side, l: Option<&T>, d: Option<&T>) -> Option<T> {
    match side {
        Side::Local => l.cloned(),
        Side::Disk => d.cloned(),
    }
}

pub fn merge(base: &CaseState, local: &CaseState, disk: &CaseState, resolve: Resolve) -> Result<Merged, Error> {
    let mut ctx = Ctx {
        resolve,
        clashes: Vec::new(),
    };
    let (ba, la, da) = (aggs(base)?, aggs(local)?, aggs(disk)?);
    let mut out = merge_entities(&mut ctx, &ba, &la, &da)?;
    let rels = merge_relations(&mut ctx, base, local, disk)?;
    let (relations, dropped) = repair_ends(&mut ctx, &mut out, rels, &ba, &la, &da)?;

    let mut state = CaseState::default();
    state.file.meta = local.file.meta.clone();
    let name = (&base.file.meta.name, &local.file.meta.name, &disk.file.meta.name);
    state.file.meta.name = pick(&mut ctx, Clash::CaseName, Some(name.0), name.1, name.2)?;
    let present: BTreeSet<String> = out.keys().cloned().collect();
    for (slug, agg) in out {
        let mut rec = agg.head;
        rec.observations = agg.obs;
        state.notes.insert(slug, agg.note);
        state.file.data.entities.push(rec);
    }
    state.file.data.relations = relations;
    state.file.display = merge_display(&base.file.display, &local.file.display, &disk.file.display, &present)?;
    Ok(Merged {
        state,
        clashes: ctx.clashes,
        dropped,
    })
}

pub fn describe(clash: &Clash) -> Result<String, Error> {
    Ok(match clash {
        Clash::CaseName => "case name — renamed here and on disk".to_owned(),
        Clash::Head(slug) => format!("{slug} — entity details edited here and on disk"),
        Clash::Note(slug) => format!("{slug} — note edited here and on disk"),
        Clash::Obs { slug, name } => format!("{slug} · {name} — fact edited here and on disk"),
        Clash::Rel { from, to, kind } => format!("{from} —{kind}→ {to} — link edited here and on disk"),
        Clash::Gone {
            slug,
            deleted_by: Side::Disk,
        } => format!("{slug} — deleted on disk, but you changed or linked it here"),
        Clash::Gone {
            slug,
            deleted_by: Side::Local,
        } => format!("{slug} — you deleted or renamed it, but disk changed or linked it"),
    })
}

fn aggs(s: &CaseState) -> Result<BTreeMap<String, Agg>, Error> {
    let mut out = BTreeMap::new();
    for e in &s.file.data.entities {
        let mut head = e.clone();
        head.observations = Vec::new();
        let agg = Agg {
            head,
            note: note_on_disk(&s.note(&e.slug)?)?,
            obs: e.observations.clone(),
        };
        out.insert(e.slug.clone(), agg);
    }
    Ok(out)
}

fn same<T: Serialize>(a: &T, b: &T) -> Result<bool, Error> {
    let x = serde_json::to_string(a).map_err(|source| Error::JsonSer { source })?;
    let y = serde_json::to_string(b).map_err(|source| Error::JsonSer { source })?;
    Ok(x == y)
}

fn eq_opt<T: Serialize>(a: Option<&T>, b: Option<&T>) -> Result<bool, Error> {
    match (a, b) {
        (Some(x), Some(y)) => same(x, y),
        (None, None) => Ok(true),
        (Some(_), None) | (None, Some(_)) => Ok(false),
    }
}

fn way<T: Serialize>(b: Option<&T>, l: Option<&T>, d: Option<&T>) -> Result<Way, Error> {
    if eq_opt(l, d)? {
        return Ok(Way::Local);
    }
    if eq_opt(l, b)? {
        return Ok(Way::Disk);
    }
    if eq_opt(d, b)? {
        return Ok(Way::Local);
    }
    Ok(Way::Clash)
}

fn merge_entities(
    ctx: &mut Ctx,
    ba: &BTreeMap<String, Agg>,
    la: &BTreeMap<String, Agg>,
    da: &BTreeMap<String, Agg>,
) -> Result<BTreeMap<String, Agg>, Error> {
    let mut slugs: BTreeSet<&String> = BTreeSet::new();
    slugs.extend(ba.keys());
    slugs.extend(la.keys());
    slugs.extend(da.keys());
    let mut out = BTreeMap::new();
    for slug in slugs {
        let (b, l, d) = (ba.get(slug), la.get(slug), da.get(slug));
        let chosen = match way(b, l, d)? {
            Way::Local => l.cloned(),
            Way::Disk => d.cloned(),
            Way::Clash => match (l, d) {
                (Some(l), Some(d)) => Some(merge_agg(ctx, slug, b, l, d)?),
                (None, d) => {
                    let gone = Clash::Gone {
                        slug: slug.clone(),
                        deleted_by: Side::Local,
                    };
                    side_of(ctx.clash(gone), None, d)
                }
                (l, None) => {
                    let gone = Clash::Gone {
                        slug: slug.clone(),
                        deleted_by: Side::Disk,
                    };
                    side_of(ctx.clash(gone), l, None)
                }
            },
        };
        for agg in chosen.into_iter() {
            out.insert(slug.clone(), agg);
        }
    }
    Ok(out)
}

fn merge_agg(ctx: &mut Ctx, slug: &str, b: Option<&Agg>, l: &Agg, d: &Agg) -> Result<Agg, Error> {
    let head = pick(
        ctx,
        Clash::Head(slug.to_owned()),
        b.map(|x| &x.head),
        &l.head,
        &d.head,
    )?;
    let note = pick(
        ctx,
        Clash::Note(slug.to_owned()),
        b.map(|x| &x.note),
        &l.note,
        &d.note,
    )?;
    let base_obs = b.into_iter().flat_map(|x| x.obs.iter());
    let obs = merge_obs(ctx, slug, obs_keys(base_obs), &l.obs, &d.obs)?;
    Ok(Agg { head, note, obs })
}

fn pick<T: Serialize + Clone>(ctx: &mut Ctx, clash: Clash, b: Option<&T>, l: &T, d: &T) -> Result<T, Error> {
    Ok(match way(b, Some(l), Some(d))? {
        Way::Local => l.clone(),
        Way::Disk => d.clone(),
        Way::Clash => match ctx.clash(clash) {
            Side::Local => l.clone(),
            Side::Disk => d.clone(),
        },
    })
}

fn obs_keys<'a>(v: impl Iterator<Item = &'a Observation>) -> Vec<(ObsKey, &'a Observation)> {
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    let mut out = Vec::new();
    for o in v {
        let nth = seen.entry(o.id.0.as_str()).or_insert(0);
        out.push(((o.id.0.clone(), *nth), o));
        *nth += 1;
    }
    out
}

fn clash_name(lo: Option<&Observation>, dobs: Option<&Observation>, key: &ObsKey) -> String {
    for o in lo.into_iter().chain(dobs) {
        return o.name.clone();
    }
    key.0.clone()
}

fn merge_obs(
    ctx: &mut Ctx,
    slug: &str,
    bk: Vec<(ObsKey, &Observation)>,
    l: &[Observation],
    d: &[Observation],
) -> Result<Vec<Observation>, Error> {
    let (lk, dk) = (obs_keys(l.iter()), obs_keys(d.iter()));
    let bm: BTreeMap<&ObsKey, &Observation> = bk.iter().map(|(k, o)| (k, *o)).collect();
    let lm: BTreeMap<&ObsKey, &Observation> = lk.iter().map(|(k, o)| (k, *o)).collect();
    let dm: BTreeMap<&ObsKey, &Observation> = dk.iter().map(|(k, o)| (k, *o)).collect();
    let mut order: Vec<&ObsKey> = Vec::new();
    for (k, _) in lk.iter().chain(dk.iter()).chain(bk.iter()) {
        if !order.contains(&k) {
            order.push(k);
        }
    }
    let mut out = Vec::new();
    for k in order {
        let (bo, lo, dobs) = (bm.get(k).copied(), lm.get(k).copied(), dm.get(k).copied());
        let chosen = match way(bo, lo, dobs)? {
            Way::Local => lo.cloned(),
            Way::Disk => dobs.cloned(),
            Way::Clash => {
                let clash = Clash::Obs {
                    slug: slug.to_owned(),
                    name: clash_name(lo, dobs, k),
                };
                side_of(ctx.clash(clash), lo, dobs)
            }
        };
        out.extend(chosen);
    }
    Ok(out)
}

fn rel_keys(v: &[RelationRec]) -> Vec<(RelKey, &RelationRec)> {
    let mut seen: BTreeMap<(String, String, String, i64), usize> = BTreeMap::new();
    let mut out = Vec::new();
    for r in v {
        let k = (r.from.clone(), r.to.clone(), format!("{:?}", r.kind), r.created_at);
        let nth = seen.entry(k.clone()).or_insert(0);
        out.push(((k.0, k.1, k.2, k.3, *nth), r));
        *nth += 1;
    }
    out
}

struct RelOut {
    rec: RelationRec,
    by: Option<Side>,
}

fn merge_relations(
    ctx: &mut Ctx,
    base: &CaseState,
    local: &CaseState,
    disk: &CaseState,
) -> Result<Vec<RelOut>, Error> {
    let bk = rel_keys(&base.file.data.relations);
    let lk = rel_keys(&local.file.data.relations);
    let dk = rel_keys(&disk.file.data.relations);
    let bm: BTreeMap<&RelKey, &RelationRec> = bk.iter().map(|(k, r)| (k, *r)).collect();
    let lm: BTreeMap<&RelKey, &RelationRec> = lk.iter().map(|(k, r)| (k, *r)).collect();
    let dm: BTreeMap<&RelKey, &RelationRec> = dk.iter().map(|(k, r)| (k, *r)).collect();
    let mut order: Vec<&RelKey> = Vec::new();
    for (k, _) in lk.iter().chain(dk.iter()).chain(bk.iter()) {
        if !order.contains(&k) {
            order.push(k);
        }
    }
    let mut out = Vec::new();
    for k in order {
        let (br, lr, dr) = (bm.get(k).copied(), lm.get(k).copied(), dm.get(k).copied());
        let (chosen, by) = match way(br, lr, dr)? {
            Way::Local => (lr.cloned(), changed_by(br, lr, Side::Local)?),
            Way::Disk => (dr.cloned(), changed_by(br, dr, Side::Disk)?),
            Way::Clash => {
                let clash = Clash::Rel {
                    from: k.0.clone(),
                    to: k.1.clone(),
                    kind: k.2.clone(),
                };
                let by = ctx.clash(clash);
                (side_of(by, lr, dr), Some(by))
            }
        };
        for rec in chosen.into_iter() {
            out.push(RelOut { rec, by });
        }
    }
    Ok(out)
}

fn changed_by(b: Option<&RelationRec>, side: Option<&RelationRec>, who: Side) -> Result<Option<Side>, Error> {
    if eq_opt(b, side)? {
        return Ok(None);
    }
    Ok(Some(who))
}

fn repair_ends(
    ctx: &mut Ctx,
    out: &mut BTreeMap<String, Agg>,
    rels: Vec<RelOut>,
    ba: &BTreeMap<String, Agg>,
    la: &BTreeMap<String, Agg>,
    da: &BTreeMap<String, Agg>,
) -> Result<(Vec<RelationRec>, Vec<String>), Error> {
    for r in &rels {
        for end in [&r.rec.from, &r.rec.to] {
            if out.contains_key(end) || !ba.contains_key(end) {
                continue;
            }
            let (dangling_side, owner) = match r.by {
                Some(Side::Local) if !da.contains_key(end) => (Side::Disk, la),
                Some(Side::Disk) if !la.contains_key(end) => (Side::Local, da),
                Some(Side::Local) | Some(Side::Disk) | None => continue,
            };
            let clash = Clash::Gone {
                slug: end.clone(),
                deleted_by: dangling_side,
            };
            if !ctx.clashes.contains(&clash) {
                ctx.clashes.push(clash);
            }
            if ctx.winner() == dangling_side {
                continue;
            }
            for agg in owner.get(end).into_iter() {
                out.insert(end.clone(), agg.clone());
            }
        }
    }
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for r in rels {
        if out.contains_key(&r.rec.from) && out.contains_key(&r.rec.to) {
            kept.push(r.rec);
            continue;
        }
        dropped.push(format!("{} -> {}", r.rec.from, r.rec.to));
    }
    Ok((kept, dropped))
}

fn view3<T: PartialEq + Clone>(b: &T, l: &T, d: &T) -> T {
    if l == b {
        return d.clone();
    }
    l.clone()
}

fn view_map<V: PartialEq + Clone>(
    b: &BTreeMap<String, V>,
    l: &BTreeMap<String, V>,
    d: &BTreeMap<String, V>,
    present: &BTreeSet<String>,
) -> BTreeMap<String, V> {
    let mut out = BTreeMap::new();
    for slug in present {
        let chosen = view3(&b.get(slug), &l.get(slug), &d.get(slug));
        for v in chosen.into_iter() {
            out.insert(slug.clone(), v.clone());
        }
    }
    out
}

fn merge_display(
    b: &CaseDisplay,
    l: &CaseDisplay,
    d: &CaseDisplay,
    present: &BTreeSet<String>,
) -> Result<CaseDisplay, Error> {
    let mut collapsed = BTreeSet::new();
    for slug in present {
        let has = |x: &CaseDisplay| x.collapsed.contains(slug);
        if view3(&has(b), &has(l), &has(d)) {
            collapsed.insert(slug.clone());
        }
    }
    Ok(CaseDisplay {
        camera: view3(&b.camera, &l.camera, &d.camera),
        positions: view_map(&b.positions, &l.positions, &d.positions, present),
        sizes: view_map(&b.sizes, &l.sizes, &d.sizes, present),
        size_by_degree: view3(&b.size_by_degree, &l.size_by_degree, &d.size_by_degree),
        layout: view3(&b.layout, &l.layout, &d.layout),
        collapsed,
    })
}
