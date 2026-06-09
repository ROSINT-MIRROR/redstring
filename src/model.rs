//! Core investigation model: entities (nodes), relations (edges), and the
//! sources that justify each relation. The whole thesis of redstring lives on
//! the edge: a `Relation` with an empty `sources` vec is an unproven assertion.

use std::collections::BTreeMap;

use egui::Color32;
use serde::{Deserialize, Serialize};

/// What an entity *is*. Drives icon + colour on the board.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum EntityKind {
    Person,
    Org,
    Domain,
    Ip,
    Email,
    Phone,
    Account,
    Document,
    Event,
    Location,
    Asset,
    Money,
    Other,
}

impl EntityKind {
    pub const ALL: [EntityKind; 13] = [
        Self::Person,
        Self::Org,
        Self::Domain,
        Self::Ip,
        Self::Email,
        Self::Phone,
        Self::Account,
        Self::Document,
        Self::Event,
        Self::Location,
        Self::Asset,
        Self::Money,
        Self::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Person => "Person",
            Self::Org => "Org",
            Self::Domain => "Domain",
            Self::Ip => "IP",
            Self::Email => "Email",
            Self::Phone => "Phone",
            Self::Account => "Account",
            Self::Document => "Document",
            Self::Event => "Event",
            Self::Location => "Location",
            Self::Asset => "Asset",
            Self::Money => "Money",
            Self::Other => "Other",
        }
    }

    /// Leading glyph for the node label. Sticks to glyphs egui's bundled
    /// NotoEmoji subset actually renders; ASCII fallback for the iffy ones.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Person => "👤",
            Self::Org => "🏢",
            Self::Domain => "🌐",
            Self::Ip => "📡",
            Self::Email => "✉",
            Self::Phone => "☎",
            Self::Account => "@",
            Self::Document => "📄",
            Self::Event => "📅",
            Self::Location => "📍",
            Self::Asset => "📦",
            Self::Money => "💰",
            Self::Other => "?",
        }
    }

    pub fn color(self) -> Color32 {
        match self {
            Self::Person => Color32::from_rgb(100, 170, 255),
            Self::Org => Color32::from_rgb(255, 170, 80),
            Self::Domain => Color32::from_rgb(120, 210, 150),
            Self::Ip => Color32::from_rgb(90, 200, 200),
            Self::Email => Color32::from_rgb(200, 150, 255),
            Self::Phone => Color32::from_rgb(255, 200, 90),
            Self::Account => Color32::from_rgb(255, 140, 180),
            Self::Document => Color32::from_rgb(200, 200, 200),
            Self::Event => Color32::from_rgb(255, 120, 120),
            Self::Location => Color32::from_rgb(150, 220, 120),
            Self::Asset => Color32::from_rgb(210, 180, 140),
            Self::Money => Color32::from_rgb(240, 220, 90),
            Self::Other => Color32::from_rgb(170, 170, 170),
        }
    }
}

/// The verb on the red string. Edges are directed and the board renders them
/// `A —label→ B`, so every label is phrased to sit between subject and object:
/// "doe —beneficial owner of→ shell_co", "shell_co —registered at→ registry".
///
/// Variants are grouped by investigation family — see `category()` — and both
/// the enum and `ALL` keep that grouping order, so anything iterating `ALL`
/// comes out visually grouped for free.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RelationKind {
    // ownership/control
    Owns,
    PartOwns,
    ShareholderOf,
    BeneficialOwnerOf,
    Controls,
    SubsidiaryOf,
    Manages,
    Founded,
    Dissolved,
    Acquired,
    // roles
    DirectorOf,
    CeoOf,
    BoardMemberOf,
    MemberOf,
    EmployeeOf,
    ContractorFor,
    LawyerFor,
    AgentOf,
    AdvisorTo,
    Represents,
    // people
    MarriedTo,
    PartnerOf,
    ParentOf,
    ChildOf,
    SiblingOf,
    RelativeOf,
    FriendOf,
    AssociateOf,
    AliasOf,
    SameAs,
    // comms/meetings
    MetWith,
    CommunicatedWith,
    Emailed,
    Called,
    Messaged,
    Introduced,
    // money
    Paid,
    Financed,
    LoanedTo,
    DonatedTo,
    InvestedIn,
    Bribed,
    Invoiced,
    TransferredTo,
    LaunderedThrough,
    Owes,
    // legal/state
    Sued,
    Prosecuted,
    Investigated,
    Sanctioned,
    ConvictedOf,
    WitnessIn,
    RegulatedBy,
    LicensedBy,
    RegisteredAt,
    IncorporatedIn,
    // digital/infra
    OwnsDomain,
    Hosts,
    ResolvesTo,
    UsesAccount,
    Operates,
    Accessed,
    // movement/physical
    LocatedAt,
    ResidesAt,
    TraveledTo,
    Transported,
    Supplied,
    Smuggled,
    ShippedTo,
    // info
    ReportedOn,
    LeakedTo,
    SourceFor,
    Authored,
    Signed,
    Witnessed,
    MentionedIn,
    AppearedIn,
    // fallback
    RelatedTo,
}

impl RelationKind {
    // RECONCILE(edgedrag): the right-drag verb-picker popup iterates `ALL`.
    // Keep it exhaustive and category-ordered (contiguous runs per
    // `category()`) so the list reads grouped; a test below enforces this.
    pub const ALL: [RelationKind; 78] = [
        // ownership/control
        Self::Owns,
        Self::PartOwns,
        Self::ShareholderOf,
        Self::BeneficialOwnerOf,
        Self::Controls,
        Self::SubsidiaryOf,
        Self::Manages,
        Self::Founded,
        Self::Dissolved,
        Self::Acquired,
        // roles
        Self::DirectorOf,
        Self::CeoOf,
        Self::BoardMemberOf,
        Self::MemberOf,
        Self::EmployeeOf,
        Self::ContractorFor,
        Self::LawyerFor,
        Self::AgentOf,
        Self::AdvisorTo,
        Self::Represents,
        // people
        Self::MarriedTo,
        Self::PartnerOf,
        Self::ParentOf,
        Self::ChildOf,
        Self::SiblingOf,
        Self::RelativeOf,
        Self::FriendOf,
        Self::AssociateOf,
        Self::AliasOf,
        Self::SameAs,
        // comms/meetings
        Self::MetWith,
        Self::CommunicatedWith,
        Self::Emailed,
        Self::Called,
        Self::Messaged,
        Self::Introduced,
        // money
        Self::Paid,
        Self::Financed,
        Self::LoanedTo,
        Self::DonatedTo,
        Self::InvestedIn,
        Self::Bribed,
        Self::Invoiced,
        Self::TransferredTo,
        Self::LaunderedThrough,
        Self::Owes,
        // legal/state
        Self::Sued,
        Self::Prosecuted,
        Self::Investigated,
        Self::Sanctioned,
        Self::ConvictedOf,
        Self::WitnessIn,
        Self::RegulatedBy,
        Self::LicensedBy,
        Self::RegisteredAt,
        Self::IncorporatedIn,
        // digital/infra
        Self::OwnsDomain,
        Self::Hosts,
        Self::ResolvesTo,
        Self::UsesAccount,
        Self::Operates,
        Self::Accessed,
        // movement/physical
        Self::LocatedAt,
        Self::ResidesAt,
        Self::TraveledTo,
        Self::Transported,
        Self::Supplied,
        Self::Smuggled,
        Self::ShippedTo,
        // info
        Self::ReportedOn,
        Self::LeakedTo,
        Self::SourceFor,
        Self::Authored,
        Self::Signed,
        Self::Witnessed,
        Self::MentionedIn,
        Self::AppearedIn,
        // fallback
        Self::RelatedTo,
    ];

    pub fn label(self) -> &'static str {
        match self {
            // ownership/control
            Self::Owns => "owns",
            Self::PartOwns => "part-owns",
            Self::ShareholderOf => "shareholder of",
            Self::BeneficialOwnerOf => "beneficial owner of",
            Self::Controls => "controls",
            Self::SubsidiaryOf => "subsidiary of",
            Self::Manages => "manages",
            Self::Founded => "founded",
            Self::Dissolved => "dissolved",
            Self::Acquired => "acquired",
            // roles
            Self::DirectorOf => "director of",
            Self::CeoOf => "CEO of",
            Self::BoardMemberOf => "board member of",
            Self::MemberOf => "member of",
            Self::EmployeeOf => "employee of",
            Self::ContractorFor => "contractor for",
            Self::LawyerFor => "lawyer for",
            Self::AgentOf => "agent of",
            Self::AdvisorTo => "advisor to",
            Self::Represents => "represents",
            // people
            Self::MarriedTo => "married to",
            Self::PartnerOf => "partner of",
            Self::ParentOf => "parent of",
            Self::ChildOf => "child of",
            Self::SiblingOf => "sibling of",
            Self::RelativeOf => "relative of",
            Self::FriendOf => "friend of",
            Self::AssociateOf => "associate of",
            Self::AliasOf => "alias of",
            Self::SameAs => "same as",
            // comms/meetings
            Self::MetWith => "met with",
            Self::CommunicatedWith => "communicated with",
            Self::Emailed => "emailed",
            Self::Called => "called",
            Self::Messaged => "messaged",
            Self::Introduced => "introduced",
            // money
            Self::Paid => "paid",
            Self::Financed => "financed",
            Self::LoanedTo => "loaned to",
            Self::DonatedTo => "donated to",
            Self::InvestedIn => "invested in",
            Self::Bribed => "bribed",
            Self::Invoiced => "invoiced",
            Self::TransferredTo => "transferred to",
            Self::LaunderedThrough => "laundered through",
            Self::Owes => "owes",
            // legal/state
            Self::Sued => "sued",
            Self::Prosecuted => "prosecuted",
            Self::Investigated => "investigated",
            Self::Sanctioned => "sanctioned",
            Self::ConvictedOf => "convicted of",
            Self::WitnessIn => "witness in",
            Self::RegulatedBy => "regulated by",
            Self::LicensedBy => "licensed by",
            Self::RegisteredAt => "registered at",
            Self::IncorporatedIn => "incorporated in",
            // digital/infra
            Self::OwnsDomain => "owns domain",
            Self::Hosts => "hosts",
            Self::ResolvesTo => "resolves to",
            Self::UsesAccount => "uses account",
            Self::Operates => "operates",
            Self::Accessed => "accessed",
            // movement/physical
            Self::LocatedAt => "located at",
            Self::ResidesAt => "resides at",
            Self::TraveledTo => "traveled to",
            Self::Transported => "transported",
            Self::Supplied => "supplied",
            Self::Smuggled => "smuggled",
            Self::ShippedTo => "shipped to",
            // info
            Self::ReportedOn => "reported on",
            Self::LeakedTo => "leaked to",
            Self::SourceFor => "source for",
            Self::Authored => "authored",
            Self::Signed => "signed",
            Self::Witnessed => "witnessed",
            Self::MentionedIn => "mentioned in",
            Self::AppearedIn => "appeared in",
            // fallback
            Self::RelatedTo => "related to",
        }
    }

    /// Investigation family of the verb.
    // RECONCILE(chrome): grouping key for picker UIs (section headers in the
    // verb combo/popup). Stable display strings, ordered as in `ALL`.
    pub fn category(self) -> &'static str {
        match self {
            Self::Owns
            | Self::PartOwns
            | Self::ShareholderOf
            | Self::BeneficialOwnerOf
            | Self::Controls
            | Self::SubsidiaryOf
            | Self::Manages
            | Self::Founded
            | Self::Dissolved
            | Self::Acquired => "ownership/control",
            Self::DirectorOf
            | Self::CeoOf
            | Self::BoardMemberOf
            | Self::MemberOf
            | Self::EmployeeOf
            | Self::ContractorFor
            | Self::LawyerFor
            | Self::AgentOf
            | Self::AdvisorTo
            | Self::Represents => "roles",
            Self::MarriedTo
            | Self::PartnerOf
            | Self::ParentOf
            | Self::ChildOf
            | Self::SiblingOf
            | Self::RelativeOf
            | Self::FriendOf
            | Self::AssociateOf
            | Self::AliasOf
            | Self::SameAs => "people",
            Self::MetWith
            | Self::CommunicatedWith
            | Self::Emailed
            | Self::Called
            | Self::Messaged
            | Self::Introduced => "comms/meetings",
            Self::Paid
            | Self::Financed
            | Self::LoanedTo
            | Self::DonatedTo
            | Self::InvestedIn
            | Self::Bribed
            | Self::Invoiced
            | Self::TransferredTo
            | Self::LaunderedThrough
            | Self::Owes => "money",
            Self::Sued
            | Self::Prosecuted
            | Self::Investigated
            | Self::Sanctioned
            | Self::ConvictedOf
            | Self::WitnessIn
            | Self::RegulatedBy
            | Self::LicensedBy
            | Self::RegisteredAt
            | Self::IncorporatedIn => "legal/state",
            Self::OwnsDomain
            | Self::Hosts
            | Self::ResolvesTo
            | Self::UsesAccount
            | Self::Operates
            | Self::Accessed => "digital/infra",
            Self::LocatedAt
            | Self::ResidesAt
            | Self::TraveledTo
            | Self::Transported
            | Self::Supplied
            | Self::Smuggled
            | Self::ShippedTo => "movement/physical",
            Self::ReportedOn
            | Self::LeakedTo
            | Self::SourceFor
            | Self::Authored
            | Self::Signed
            | Self::Witnessed
            | Self::MentionedIn
            | Self::AppearedIn => "info",
            Self::RelatedTo => "fallback",
        }
    }
}

/// Where a claim comes from. Points into the rest of the kit's corpus.
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

/// Node payload. The slug is the case-wide stable handle: relations and
/// display positions reference it, and it's what an external editor (human or
/// AI) uses to point at an entity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub slug: String,
    pub kind: EntityKind,
    pub label: String,
    pub attrs: Vec<(String, String)>,
    pub note: String,
}

/// Edge payload — carries the sources. `is_sourced() == false` ⇒ unproven.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relation {
    pub kind: RelationKind,
    pub sources: Vec<Source>,
}

impl Relation {
    pub fn new(kind: RelationKind) -> Self {
        Self {
            kind,
            sources: Vec::new(),
        }
    }

    pub fn is_sourced(&self) -> bool {
        !self.sources.is_empty()
    }
}

// ---- on-disk format ----
//
// One RON file, two structs: `data` is the investigation (what an AI edits —
// everything display-related is optional and the app heals it), `display` is
// machine-owned scenography (positions + camera) so a case opens looking the
// same every time. Editing contract for `data`: reference entities by slug;
// omit anything optional and the app fills it in on load.

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct CaseFile {
    pub data: CaseData,
    #[serde(default)]
    pub display: CaseDisplay,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct CaseData {
    #[serde(default)]
    pub entities: Vec<EntityRec>,
    #[serde(default)]
    pub relations: Vec<RelationRec>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct EntityRec {
    pub slug: String,
    pub kind: EntityKind,
    pub label: String,
    #[serde(default)]
    pub attrs: Vec<(String, String)>,
    #[serde(default)]
    pub note: String,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RelationRec {
    pub from: String,
    pub to: String,
    pub kind: RelationKind,
    #[serde(default)]
    pub sources: Vec<Source>,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct CaseDisplay {
    #[serde(default)]
    pub camera: Camera,
    /// slug → (x, y). BTreeMap so saves serialize in sorted order → stable diffs.
    #[serde(default)]
    pub positions: BTreeMap<String, (f32, f32)>,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The AI-editing contract: a minimal hand-written file — no display, no
    /// sources, no attrs/note/excerpt — must load, with everything defaulted.
    #[test]
    fn minimal_external_edit_loads() {
        let s = r#"(
            data: (
                entities: [
                    (slug: "doe", kind: Person, label: "J. Doe"),
                    (slug: "shell", kind: Org, label: "Shell Co"),
                ],
                relations: [
                    (from: "doe", to: "shell", kind: Controls),
                ],
            ),
        )"#;
        let cf: CaseFile = ron::from_str(s).expect("minimal file must parse");
        assert_eq!(cf.data.entities.len(), 2);
        assert_eq!(cf.data.relations.len(), 1);
        assert!(cf.data.relations[0].sources.is_empty());
        assert!(cf.display.positions.is_empty());
        assert_eq!(cf.display.camera, Camera::default());
    }

    /// `ALL` is the iteration order every picker uses (RECONCILE(edgedrag)):
    /// categories must form contiguous runs so the list reads grouped, and no
    /// variant may appear twice.
    #[test]
    fn relation_kinds_grouped_and_unique() {
        let mut seen_cats: Vec<&str> = Vec::new();
        for k in RelationKind::ALL {
            let c = k.category();
            if seen_cats.last() != Some(&c) {
                assert!(
                    !seen_cats.contains(&c),
                    "category '{c}' is split into non-contiguous runs in ALL"
                );
                seen_cats.push(c);
            }
        }
        assert_eq!(seen_cats.len(), 10, "expected the 10 verb families");

        let mut kinds = RelationKind::ALL.to_vec();
        kinds.dedup_by(|a, b| a == b); // adjacent dupes
        assert_eq!(kinds.len(), RelationKind::ALL.len());
        for (i, a) in RelationKind::ALL.iter().enumerate() {
            assert!(
                !RelationKind::ALL[i + 1..].contains(a),
                "duplicate variant {a:?} in ALL"
            );
        }
    }

    #[test]
    fn round_trip() {
        let mut cf = CaseFile::default();
        cf.data.entities.push(EntityRec {
            slug: "doe".into(),
            kind: EntityKind::Person,
            label: "J. Doe".into(),
            attrs: vec![("dob".into(), "1971-03-04".into())],
            note: "note".into(),
        });
        cf.data.relations.push(RelationRec {
            from: "doe".into(),
            to: "doe".into(),
            kind: RelationKind::AliasOf,
            sources: vec![Source {
                kind: SourceKind::Markdown,
                reference: "dossier.md#x".into(),
                excerpt: "quote".into(),
            }],
        });
        cf.display.positions.insert("doe".into(), (12.5, -40.0));
        cf.display.camera = Camera {
            x: 3.0,
            y: -7.5,
            zoom: 1.25,
        };

        let s = ron::ser::to_string_pretty(&cf, ron::ser::PrettyConfig::default()).unwrap();
        let back: CaseFile = ron::from_str(&s).unwrap();
        assert_eq!(cf, back);
    }
}
