use std::collections::HashSet;

use crate::error::Error;

use super::case::RelationRec;
use super::observation::{Observation, Origin, Verification};
use super::relation::Relation;
use super::source::{Source, SourceTier};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Footing {
    Corroborated,
    Confirmed,
    Sourced,
    Unproven,
    Disputed,
    Rejected,
}

impl Footing {
    pub const STRONG: [Footing; 2] = [Footing::Corroborated, Footing::Confirmed];

    pub fn of(status: Verification, origin: &Origin, sources: &[Source]) -> Result<Footing, Error> {
        Ok(classify(status, origin, sources))
    }

    pub fn would_hold(
        status: Verification,
        origin: &Origin,
        sources: &[Source],
    ) -> Result<bool, Error> {
        Ok(match status {
            Verification::Confirmed | Verification::Corroborated => {
                Footing::STRONG.contains(&classify(status, origin, sources))
            }
            Verification::Unreviewed
            | Verification::MachineSuggested
            | Verification::AnalystAsserted
            | Verification::Rejected
            | Verification::DeniedByTarget
            | Verification::NoPublicRecord => true,
        })
    }

    pub fn needs(status: Verification) -> Result<&'static str, Error> {
        Ok(match status {
            Verification::Confirmed => {
                "a source rated primary, on a record that is not an inference"
            }
            Verification::Corroborated => {
                "2+ independent origins (distinct source org, else reference), on a record that is not an inference"
            }
            Verification::Unreviewed
            | Verification::MachineSuggested
            | Verification::AnalystAsserted
            | Verification::Rejected
            | Verification::DeniedByTarget
            | Verification::NoPublicRecord => {
                "nothing more — this status never draws the strong string"
            }
        })
    }

    pub fn label(self) -> Result<&'static str, Error> {
        Ok(match self {
            Footing::Corroborated => "corroborated",
            Footing::Confirmed => "confirmed · primary source",
            Footing::Sourced => "sourced claim",
            Footing::Unproven => "unproven",
            Footing::Disputed => "disputed",
            Footing::Rejected => "rejected",
        })
    }

    pub fn hint(self) -> Result<&'static str, Error> {
        Ok(match self {
            Footing::Corroborated => {
                "an analyst marked it corroborated and it cites 2+ independent origins"
            }
            Footing::Confirmed => {
                "an analyst confirmed it against a primary source on file — one authoritative record, not independent corroboration"
            }
            Footing::Sourced => {
                "cites a source, but no analyst has confirmed or corroborated it against that evidence"
            }
            Footing::Unproven => "no source, or inferred — a hypothesis, not a fact",
            Footing::Disputed => "denied by the target, or absent from the public record",
            Footing::Rejected => "an analyst rejected it — kept on the record, not as fact",
        })
    }
}

impl From<&Relation> for Footing {
    fn from(r: &Relation) -> Self {
        classify(r.status, &r.origin, &r.sources)
    }
}

impl From<&RelationRec> for Footing {
    fn from(r: &RelationRec) -> Self {
        classify(r.status, &r.origin, &r.sources)
    }
}

impl From<&Observation> for Footing {
    fn from(o: &Observation) -> Self {
        classify(o.status, &o.origin, &o.sources)
    }
}

fn classify(status: Verification, origin: &Origin, sources: &[Source]) -> Footing {
    match status {
        Verification::Rejected => return Footing::Rejected,
        Verification::DeniedByTarget | Verification::NoPublicRecord => return Footing::Disputed,
        Verification::Unreviewed
        | Verification::MachineSuggested
        | Verification::AnalystAsserted
        | Verification::Confirmed
        | Verification::Corroborated => {}
    }
    if sources.is_empty() || *origin == Origin::Inference {
        return Footing::Unproven;
    }
    if status == Verification::Corroborated && independent_origins(sources) >= 2 {
        return Footing::Corroborated;
    }
    if status == Verification::Confirmed && sources.iter().any(|s| s.tier == SourceTier::Primary) {
        return Footing::Confirmed;
    }
    Footing::Sourced
}

fn independent_origins(sources: &[Source]) -> usize {
    let mut seen: HashSet<String> = HashSet::new();
    for s in sources {
        seen.insert(origin_key(s));
    }
    seen.len()
}

fn origin_key(s: &Source) -> String {
    for org in s.source_org.iter() {
        let org = org.trim();
        if !org.is_empty() {
            return format!("org:{}", org.to_lowercase());
        }
    }
    format!("ref:{}", s.reference.trim().to_lowercase())
}
