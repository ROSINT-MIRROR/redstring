use egui_phosphor::regular::ARROW_RIGHT;
use petgraph::graph::NodeIndex;

use crate::error::Error;
use crate::model::{
    DateVal, Entity, EntityKind, ObsValue, Observation, RELATION_META, Relation, RelationKind,
    SOURCE_META, Source, SourceKind, SourceTier, Verification, pretty_key,
};

use super::super::app::RsGraph;
use super::event::{EventKey, Footing, Lane, Record, TimelineEvent, Via};
use super::when::{Reading, When, Zone, read_text_date};

pub struct Digest {
    pub events: Vec<TimelineEvent>,
    pub unplaced: Vec<Unplaced>,
    pub unknown_entry: usize,
}

pub struct Unplaced {
    pub record: Record,
    pub title: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    At(usize),
    Gone,
}

struct Base {
    record: Record,
    title: String,
    footing: Footing,
    status: Verification,
    confidence: u8,
    sealed: bool,
}

struct Mark {
    lane: Lane,
    when: When,
    title: String,
    fact: String,
    via: Via,
    tier: SourceTier,
    read_from_text: bool,
}

enum Stated {
    Given(When),
    ReadFromText(When),
    Undated,
}

impl Digest {
    pub fn of(g: &RsGraph) -> Result<Digest, Error> {
        let mut d = Digest {
            events: Vec::new(),
            unplaced: Vec::new(),
            unknown_entry: 0,
        };
        for idx in g.g().node_indices() {
            let Some(node) = g.node(idx) else {
                continue;
            };
            d.entity(idx, node.payload())?;
        }
        for edge in g.g().edge_indices() {
            let Some((from, to)) = g.edge_endpoints(edge) else {
                continue;
            };
            let Some(e) = g.edge(edge) else {
                continue;
            };
            let r = e.payload();
            let (a, v, b) = (label_of(g, from)?, verb(r.kind), label_of(g, to)?);
            let title = format!("{a} — {v} {ARROW_RIGHT} {b}");
            d.relation(Record::Rel { edge, from, to }, r, title)?;
        }
        d.events.sort_by(|a, b| {
            (a.key.lane, a.span.start, a.span.end, &a.title).cmp(&(
                b.key.lane,
                b.span.start,
                b.span.end,
                &b.title,
            ))
        });
        d.mark_conflicts()?;
        Ok(d)
    }

    pub fn slot(&self, key: EventKey) -> Result<Slot, Error> {
        for (i, ev) in self.events.iter().enumerate() {
            if ev.key == key {
                return Ok(Slot::At(i));
            }
        }
        Ok(Slot::Gone)
    }

    pub fn of_record(&self, record: Record) -> Result<Vec<usize>, Error> {
        let mut out = Vec::new();
        for (i, ev) in self.events.iter().enumerate() {
            if ev.key.record == record {
                out.push(i);
            }
        }
        Ok(out)
    }

    pub fn conflict_count(&self) -> Result<usize, Error> {
        let marked = self
            .events
            .iter()
            .filter(|ev| !ev.conflicts.is_empty())
            .count();
        Ok(marked / 2)
    }

    fn entity(&mut self, idx: NodeIndex, e: &Entity) -> Result<(), Error> {
        let before = self.events.len();
        for (i, o) in e.observations.iter().enumerate() {
            self.observation(Record::Obs(idx, i), e, o)?;
        }
        let event_kind = matches!(
            e.kind,
            EntityKind::Event | EntityKind::Meeting | EntityKind::Travel
        );
        if event_kind && self.events.len() == before {
            self.unplaced.push(Unplaced {
                record: Record::Entity(idx),
                title: e.label.clone(),
            });
        }
        Ok(())
    }

    fn observation(&mut self, record: Record, e: &Entity, o: &Observation) -> Result<(), Error> {
        let fact = pretty_key(&o.name)?;
        let base = Base {
            record,
            title: format!("{} · {fact}", e.label),
            footing: Footing::of(o.status, &o.origin, &o.sources)?,
            status: o.status,
            confidence: o.confidence,
            sealed: e.sealed || o.sealed,
        };
        let lane = Lane::of_fact(&o.name)?;
        if !self.place(&base, stated_value(&o.value)?, lane, &o.name)? {
            return Ok(());
        }
        self.sources(&base, &o.sources)?;
        self.entered(&base, o.created_at)
    }

    fn relation(&mut self, record: Record, r: &Relation, title: String) -> Result<(), Error> {
        let base = Base {
            record,
            title: title.clone(),
            footing: Footing::of(r.status, &r.origin, &r.sources)?,
            status: r.status,
            confidence: r.confidence,
            sealed: r.sealed,
        };
        let before = self.events.len();
        self.place(
            &base,
            window(r.valid_from, r.valid_to),
            Lane::Event,
            verb(r.kind),
        )?;
        self.sources(&base, &r.sources)?;
        if self.events.len() == before {
            self.unplaced.push(Unplaced { record, title });
            return Ok(());
        }
        self.entered(&base, r.created_at)
    }

    fn place(
        &mut self,
        base: &Base,
        stated: Stated,
        lane: Lane,
        fact: &str,
    ) -> Result<bool, Error> {
        let (when, read_from_text) = match stated {
            Stated::Given(w) => (w, false),
            Stated::ReadFromText(w) => (w, true),
            Stated::Undated => return Ok(false),
        };
        self.push(
            base,
            Mark {
                lane,
                when,
                title: base.title.clone(),
                fact: fact.to_owned(),
                via: Via::Record,
                tier: SourceTier::Unrated,
                read_from_text,
            },
        )?;
        Ok(true)
    }

    fn sources(&mut self, base: &Base, sources: &[Source]) -> Result<(), Error> {
        for (i, s) in sources.iter().enumerate() {
            let who = source_name(s);
            for p in s.published.iter() {
                self.push(
                    base,
                    Mark {
                        lane: Lane::Published,
                        when: When::Date(*p),
                        title: format!("{} · published by {who}", base.title),
                        fact: "published".to_owned(),
                        via: Via::Source(i),
                        tier: s.tier,
                        read_from_text: false,
                    },
                )?;
            }
            for f in s.fetched_at.iter() {
                self.push(
                    base,
                    Mark {
                        lane: Lane::Captured,
                        when: When::Instant {
                            secs: *f,
                            zone: Zone::Utc,
                        },
                        title: format!("{} · captured from {who}", base.title),
                        fact: "captured".to_owned(),
                        via: Via::Source(i),
                        tier: s.tier,
                        read_from_text: false,
                    },
                )?;
            }
        }
        Ok(())
    }

    fn entered(&mut self, base: &Base, created_at: i64) -> Result<(), Error> {
        if created_at <= 0 {
            self.unknown_entry += 1;
            return Ok(());
        }
        self.push(
            base,
            Mark {
                lane: Lane::Entered,
                when: When::Instant {
                    secs: created_at,
                    zone: Zone::Utc,
                },
                title: format!("{} · entered", base.title),
                fact: "entered".to_owned(),
                via: Via::Record,
                tier: SourceTier::Unrated,
                read_from_text: false,
            },
        )
    }

    fn push(&mut self, base: &Base, mark: Mark) -> Result<(), Error> {
        let span = mark.when.bounds()?;
        self.events.push(TimelineEvent {
            key: EventKey {
                lane: mark.lane,
                record: base.record,
                via: mark.via,
            },
            when: mark.when,
            span,
            title: mark.title,
            fact: mark.fact,
            footing: base.footing,
            status: base.status,
            confidence: base.confidence,
            tier: mark.tier,
            read_from_text: mark.read_from_text,
            sealed: base.sealed,
            conflicts: Vec::new(),
        });
        Ok(())
    }

    fn mark_conflicts(&mut self) -> Result<(), Error> {
        let mut facts: Vec<(usize, String, usize)> = Vec::new();
        for (i, ev) in self.events.iter().enumerate() {
            if ev.key.lane != Lane::Event {
                continue;
            }
            let Record::Obs(node, ..) = ev.key.record else {
                continue;
            };
            facts.push((node.index(), ev.fact.to_ascii_lowercase(), i));
        }
        facts.sort();
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        for (pos, a) in facts.iter().enumerate() {
            for b in facts.iter().skip(pos + 1) {
                if a.0 != b.0 || a.1 != b.1 {
                    break;
                }
                if disjoint(&self.events[a.2], &self.events[b.2]) {
                    pairs.push((a.2, b.2));
                }
            }
        }
        for (a, b) in pairs {
            self.events[a].conflicts.push(b);
            self.events[b].conflicts.push(a);
        }
        Ok(())
    }
}

fn stated_value(v: &ObsValue) -> Result<Stated, Error> {
    Ok(match v {
        ObsValue::Date(dv) => Stated::Given(When::Date(*dv)),
        ObsValue::DateRange { start, end } => window(*start, *end),
        ObsValue::Text(s) => match read_text_date(s)? {
            Reading::Dated(w) => Stated::ReadFromText(w),
            Reading::NotADate => Stated::Undated,
        },
        ObsValue::Number(..) | ObsValue::Coord { .. } | ObsValue::Bool(..) => Stated::Undated,
    })
}

fn window(start: Option<DateVal>, end: Option<DateVal>) -> Stated {
    let Some(a) = start else {
        let Some(b) = end else {
            return Stated::Undated;
        };
        return Stated::Given(When::Until(b));
    };
    let Some(b) = end else {
        return Stated::Given(When::From(a));
    };
    Stated::Given(When::Between(a, b))
}

fn disjoint(a: &TimelineEvent, b: &TimelineEvent) -> bool {
    let a_end = a.span.end.max(a.span.start + 1);
    let b_end = b.span.end.max(b.span.start + 1);
    a_end <= b.span.start || b_end <= a.span.start
}

fn label_of(g: &RsGraph, idx: NodeIndex) -> Result<String, Error> {
    let node = g.node(idx).ok_or(Error::Timeline {
        what: format!("relation endpoint {} vanished", idx.index()),
    })?;
    Ok(node.payload().label.clone())
}

fn verb(kind: RelationKind) -> &'static str {
    for m in RELATION_META.iter() {
        if m.kind == kind {
            return m.label;
        }
    }
    "related to"
}

fn source_name(s: &Source) -> String {
    let Some(org) = &s.source_org else {
        return kind_label(s.kind).to_owned();
    };
    org.clone()
}

fn kind_label(kind: SourceKind) -> &'static str {
    for m in SOURCE_META.iter() {
        if m.kind == kind {
            return m.label;
        }
    }
    "source"
}
