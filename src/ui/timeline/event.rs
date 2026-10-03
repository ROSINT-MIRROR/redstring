use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::error::Error;
use crate::model::{Origin, SourceTier, Verification};

use super::when::{Bounds, When};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Lane {
    Event,
    Published,
    Captured,
    Entered,
}

impl Lane {
    pub const ALL: [Lane; 4] = [Lane::Event, Lane::Published, Lane::Captured, Lane::Entered];
    pub const PUBLISHED_KEYS: [&'static str; 4] =
        ["published", "publication_date", "date_published", "posted"];
    pub const CAPTURED_KEYS: [&'static str; 5] = [
        "received",
        "received_on",
        "obtained",
        "obtained_on",
        "captured",
    ];

    pub fn slot(self) -> Result<usize, Error> {
        Ok(match self {
            Lane::Event => 0,
            Lane::Published => 1,
            Lane::Captured => 2,
            Lane::Entered => 3,
        })
    }

    pub fn label(self) -> Result<&'static str, Error> {
        Ok(match self {
            Lane::Event => "event",
            Lane::Published => "published",
            Lane::Captured => "captured",
            Lane::Entered => "entered",
        })
    }

    pub fn caption(self) -> Result<&'static str, Error> {
        Ok(match self {
            Lane::Event => "when it happened",
            Lane::Published => "made public",
            Lane::Captured => "fetched / received",
            Lane::Entered => "into this case",
        })
    }

    pub fn of_fact(name: &str) -> Result<Lane, Error> {
        let n = name.trim().to_ascii_lowercase();
        if Lane::PUBLISHED_KEYS.contains(&n.as_str()) {
            return Ok(Lane::Published);
        }
        if Lane::CAPTURED_KEYS.contains(&n.as_str()) {
            return Ok(Lane::Captured);
        }
        Ok(Lane::Event)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Footing {
    Corroborated,
    Sourced,
    Unproven,
    Disputed,
    Rejected,
}

impl Footing {
    pub fn of(status: Verification, origin: &Origin, sources: usize) -> Result<Footing, Error> {
        Ok(match status {
            Verification::Rejected => Footing::Rejected,
            Verification::DeniedByTarget | Verification::NoPublicRecord => Footing::Disputed,
            Verification::Unreviewed
            | Verification::MachineSuggested
            | Verification::AnalystAsserted
            | Verification::Corroborated => grade(status, origin, sources),
        })
    }

    pub fn label(self) -> Result<&'static str, Error> {
        Ok(match self {
            Footing::Corroborated => "corroborated",
            Footing::Sourced => "sourced claim",
            Footing::Unproven => "unproven",
            Footing::Disputed => "disputed",
            Footing::Rejected => "rejected",
        })
    }

    pub fn hint(self) -> Result<&'static str, Error> {
        Ok(match self {
            Footing::Corroborated => "an analyst marked it corroborated and it cites its sources",
            Footing::Sourced => "cites a source but nobody has corroborated it yet",
            Footing::Unproven => "no source, or inferred — a hypothesis, not a fact",
            Footing::Disputed => "denied by the target, or absent from the public record",
            Footing::Rejected => "an analyst rejected it — kept on the record, not as fact",
        })
    }

    pub fn tag(self) -> Result<&'static str, Error> {
        Ok(match self {
            Footing::Corroborated | Footing::Sourced => "",
            Footing::Unproven => "? ",
            Footing::Disputed => "! ",
            Footing::Rejected => "× ",
        })
    }
}

fn grade(status: Verification, origin: &Origin, sources: usize) -> Footing {
    if sources == 0 || *origin == Origin::Inference {
        return Footing::Unproven;
    }
    if status == Verification::Corroborated {
        return Footing::Corroborated;
    }
    Footing::Sourced
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Record {
    Entity(NodeIndex),
    Obs(NodeIndex, usize),
    Rel {
        edge: EdgeIndex,
        from: NodeIndex,
        to: NodeIndex,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Via {
    Record,
    Source(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct EventKey {
    pub lane: Lane,
    pub record: Record,
    pub via: Via,
}

#[derive(Clone, PartialEq, Debug)]
pub struct TimelineEvent {
    pub key: EventKey,
    pub when: When,
    pub span: Bounds,
    pub title: String,
    pub fact: String,
    pub footing: Footing,
    pub status: Verification,
    pub confidence: u8,
    pub tier: SourceTier,
    pub read_from_text: bool,
    pub sealed: bool,
    pub conflicts: Vec<usize>,
}
