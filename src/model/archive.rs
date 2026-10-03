use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ArchiveStatus {
    Confirmed,
    Pending,
    Absent,
    Skipped,
    Submitted,
    Errored,
}

impl ArchiveStatus {
    pub fn word(self) -> Result<&'static str, Error> {
        Ok(match self {
            Self::Confirmed => "confirmed",
            Self::Pending => "pending",
            Self::Absent => "none",
            Self::Skipped => "skipped",
            Self::Submitted => "submitted",
            Self::Errored => "error",
        })
    }

    pub fn from_word(word: &str) -> Result<Self, Error> {
        Ok(match word {
            "confirmed" => Self::Confirmed,
            "pending" => Self::Pending,
            "none" => Self::Absent,
            "skipped" => Self::Skipped,
            "submitted" => Self::Submitted,
            "error" => Self::Errored,
            unknown => {
                return Err(Error::Alibi {
                    what: format!("unknown witness status '{unknown}'"),
                });
            }
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ArchiveVia {
    Check,
    Archive,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ArchiveRecord {
    pub url: String,
    pub witness: String,
    pub status: ArchiveStatus,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub via: ArchiveVia,
    pub checked_at: i64,
}

impl ArchiveRecord {
    fn same_witness(&self, other: &ArchiveRecord) -> bool {
        self.url == other.url && self.witness == other.witness
    }

    fn confirmed(&self) -> bool {
        self.status == ArchiveStatus::Confirmed
    }
}

pub fn merge_archive(archives: &mut Vec<ArchiveRecord>, rec: ArchiveRecord) -> Result<bool, Error> {
    let before = archives.len();
    archives.retain(|kept| kept.confirmed() || !kept.same_witness(&rec));
    let superseded = archives.len() != before;
    let known = rec.confirmed()
        && archives
            .iter()
            .any(|kept| kept.confirmed() && kept.same_witness(&rec) && kept.snapshot == rec.snapshot);
    if known {
        return Ok(superseded);
    }
    archives.push(rec);
    Ok(true)
}
