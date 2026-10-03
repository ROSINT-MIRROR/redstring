use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::error::Error;
use crate::model::{SourceTier, Verification};

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

pub use crate::model::Footing;

impl Footing {
    pub fn tag(self) -> Result<&'static str, Error> {
        Ok(match self {
            Footing::Corroborated | Footing::Confirmed | Footing::Sourced => "",
            Footing::Unproven => "? ",
            Footing::Disputed => "! ",
            Footing::Rejected => "× ",
        })
    }
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
