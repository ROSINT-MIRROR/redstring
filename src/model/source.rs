//! Where a claim comes from. Sources point into the rest of the kit's corpus
//! — markdown notes, archived snapshots, transcripts, documents, URLs — and
//! attach to relations. This is the evidentiary seam of the whole tool.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SourceKind {
    Markdown,
    Url,
    Archive,
    Transcript,
    Document,
    Note,
}

impl SourceKind {
    pub const ALL: [SourceKind; 6] = [
        Self::Markdown,
        Self::Url,
        Self::Archive,
        Self::Transcript,
        Self::Document,
        Self::Note,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Markdown => "Markdown ref",
            Self::Url => "URL",
            Self::Archive => "Archive",
            Self::Transcript => "Transcript",
            Self::Document => "Document",
            Self::Note => "Note",
        }
    }

    /// Whether the reference names a file in the case directory — these
    /// kinds participate in copy-on-attach, validation and bundle integrity.
    pub fn is_file_backed(self) -> bool {
        match self {
            Self::Markdown | Self::Archive | Self::Transcript | Self::Document => true,
            Self::Url | Self::Note => false,
        }
    }

    /// Placeholder shown in the reference field.
    pub fn hint(self) -> &'static str {
        match self {
            Self::Markdown => "notes.md#section",
            Self::Url => "https://…",
            Self::Archive => "snapshot id / path",
            Self::Transcript => "mm:ss",
            Self::Document => "file · page",
            Self::Note => "free text",
        }
    }
}

/// One piece of evidence backing a relation.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Source {
    pub kind: SourceKind,
    pub reference: String,
    #[serde(default)]
    pub excerpt: String,
}
