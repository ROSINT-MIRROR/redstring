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

pub struct SourceMeta {
    pub kind: SourceKind,
    pub label: &'static str,
    pub hint: &'static str,
    pub file_backed: bool,
}

const fn s(kind: SourceKind, label: &'static str, hint: &'static str, file_backed: bool) -> SourceMeta {
    SourceMeta {
        kind,
        label,
        hint,
        file_backed,
    }
}

#[rustfmt::skip]
pub const SOURCE_META: [SourceMeta; 6] = [
    s(SourceKind::Markdown, "Markdown ref", "notes.md#section", true),
    s(SourceKind::Url, "URL", "https://…", false),
    s(SourceKind::Archive, "Archive", "snapshot id / path", true),
    s(SourceKind::Transcript, "Transcript", "mm:ss", true),
    s(SourceKind::Document, "Document", "file · page", true),
    s(SourceKind::Note, "Note", "free text", false),
];

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Source {
    pub kind: SourceKind,
    pub reference: String,
    #[serde(default)]
    pub excerpt: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_mirrors_declaration_order() {
        for (i, m) in SOURCE_META.iter().enumerate() {
            assert_eq!(m.kind as usize, i, "SOURCE_META[{i}] is {:?}", m.kind);
        }
    }
}
