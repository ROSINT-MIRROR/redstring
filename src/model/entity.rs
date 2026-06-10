//! Entity taxonomy: what a node *is*. Kinds are grouped into broad ontology
//! categories that drive colour-coding across the palette and the board.

use egui::Color32;
use serde::{Deserialize, Serialize};

/// Broad ontology group of an [`EntityKind`]. The palette colour-codes rows
/// by it and pickers render section headers from it. Being an enum makes the
/// grouping exhaustive — a kind without a group is a compile error.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    People,
    Organizations,
    Digital,
    Financial,
    Physical,
    Documents,
    Events,
    Other,
}

impl Category {
    /// Display order. `EntityKind::ALL` keeps its variants grouped to match —
    /// a test pins both, so pickers can render sections off either.
    pub const ALL: [Category; 8] = [
        Self::People,
        Self::Organizations,
        Self::Digital,
        Self::Financial,
        Self::Physical,
        Self::Documents,
        Self::Events,
        Self::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::People => "People",
            Self::Organizations => "Organizations",
            Self::Digital => "Digital",
            Self::Financial => "Financial",
            Self::Physical => "Physical",
            Self::Documents => "Documents",
            Self::Events => "Events",
            Self::Other => "Other",
        }
    }

    /// Anchor colour of the group — the hue every kind in it varies around.
    pub fn color(self) -> Color32 {
        match self {
            Self::People => Color32::from_rgb(100, 170, 255),
            Self::Organizations => Color32::from_rgb(255, 170, 80),
            Self::Digital => Color32::from_rgb(100, 210, 175),
            Self::Financial => Color32::from_rgb(235, 210, 95),
            Self::Physical => Color32::from_rgb(185, 160, 120),
            Self::Documents => Color32::from_rgb(195, 195, 200),
            Self::Events => Color32::from_rgb(250, 125, 125),
            Self::Other => Color32::from_rgb(170, 170, 170),
        }
    }
}

/// What an entity *is*. Drives icon + colour on the board, plus the
/// predictable attribute keys an investigator wants pre-filled per kind.
///
/// The vocabulary is tuned for investigative journalism — corruption,
/// organized crime, OSINT, finance, geopolitics. Variants are grouped by
/// [`EntityKind::category`]; keep `ALL` in the same grouped order so any
/// picker built from it reads coherently.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum EntityKind {
    // people
    Person,
    Alias,
    Official,
    Judge,
    // organizations
    Organization,
    Company,
    ShellCompany,
    Ngo,
    GovernmentBody,
    PoliticalParty,
    Court,
    LawFirm,
    Bank,
    MediaOutlet,
    CriminalOrg,
    // digital
    Domain,
    IpAddress,
    Email,
    Phone,
    SocialAccount,
    Website,
    Server,
    Device,
    CryptoWallet,
    // financial
    BankAccount,
    Transaction,
    Loan,
    Donation,
    Contract,
    Invoice,
    RegistryRecord,
    Money,
    // physical
    Location,
    Address,
    Property,
    Vehicle,
    Vessel,
    Aircraft,
    Shipment,
    Weapon,
    Asset,
    // documents & evidence
    Document,
    Passport,
    Leak,
    Dataset,
    Photo,
    Video,
    Article,
    CourtRecord,
    // events
    Event,
    Meeting,
    Payment,
    Travel,
    // other
    Other,
}

impl EntityKind {
    pub const ALL: [EntityKind; 54] = [
        // people
        Self::Person,
        Self::Alias,
        Self::Official,
        Self::Judge,
        // organizations
        Self::Organization,
        Self::Company,
        Self::ShellCompany,
        Self::Ngo,
        Self::GovernmentBody,
        Self::PoliticalParty,
        Self::Court,
        Self::LawFirm,
        Self::Bank,
        Self::MediaOutlet,
        Self::CriminalOrg,
        // digital
        Self::Domain,
        Self::IpAddress,
        Self::Email,
        Self::Phone,
        Self::SocialAccount,
        Self::Website,
        Self::Server,
        Self::Device,
        Self::CryptoWallet,
        // financial
        Self::BankAccount,
        Self::Transaction,
        Self::Loan,
        Self::Donation,
        Self::Contract,
        Self::Invoice,
        Self::RegistryRecord,
        Self::Money,
        // physical
        Self::Location,
        Self::Address,
        Self::Property,
        Self::Vehicle,
        Self::Vessel,
        Self::Aircraft,
        Self::Shipment,
        Self::Weapon,
        Self::Asset,
        // documents & evidence
        Self::Document,
        Self::Passport,
        Self::Leak,
        Self::Dataset,
        Self::Photo,
        Self::Video,
        Self::Article,
        Self::CourtRecord,
        // events
        Self::Event,
        Self::Meeting,
        Self::Payment,
        Self::Travel,
        // other
        Self::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Person => "Person",
            Self::Alias => "Alias",
            Self::Official => "Official",
            Self::Judge => "Judge",
            Self::Organization => "Organization",
            Self::Company => "Company",
            Self::ShellCompany => "Shell company",
            Self::Ngo => "NGO",
            Self::GovernmentBody => "Government body",
            Self::PoliticalParty => "Political party",
            Self::Court => "Court",
            Self::LawFirm => "Law firm",
            Self::Bank => "Bank",
            Self::MediaOutlet => "Media outlet",
            Self::CriminalOrg => "Criminal org",
            Self::Domain => "Domain",
            Self::IpAddress => "IP address",
            Self::Email => "Email",
            Self::Phone => "Phone",
            Self::SocialAccount => "Social account",
            Self::Website => "Website",
            Self::Server => "Server",
            Self::Device => "Device",
            Self::CryptoWallet => "Crypto wallet",
            Self::BankAccount => "Bank account",
            Self::Transaction => "Transaction",
            Self::Loan => "Loan",
            Self::Donation => "Donation",
            Self::Contract => "Contract",
            Self::Invoice => "Invoice",
            Self::RegistryRecord => "Registry record",
            Self::Money => "Money",
            Self::Location => "Location",
            Self::Address => "Address",
            Self::Property => "Property",
            Self::Vehicle => "Vehicle",
            Self::Vessel => "Vessel",
            Self::Aircraft => "Aircraft",
            Self::Shipment => "Shipment",
            Self::Weapon => "Weapon",
            Self::Asset => "Asset",
            Self::Document => "Document",
            Self::Passport => "Passport",
            Self::Leak => "Leak",
            Self::Dataset => "Dataset",
            Self::Photo => "Photo",
            Self::Video => "Video",
            Self::Article => "Article",
            Self::CourtRecord => "Court record",
            Self::Event => "Event",
            Self::Meeting => "Meeting",
            Self::Payment => "Payment",
            Self::Travel => "Travel",
            Self::Other => "Other",
        }
    }

    /// Leading glyph for the node label. Every emoji here is verified against
    /// the cmaps of egui 0.34's bundled NotoEmoji subset / emoji-icon-font
    /// — both sit in the default Proportional fallback chain; `@` `?` `§`
    /// come from the text fonts. Don't add glyphs without checking — e.g.
    /// 🏛 🗳 🧾 ₿ 🤝 🧳 are NOT bundled.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Person => "👤",
            Self::Alias => "🎭",
            Self::Official => "👔",
            Self::Judge => "⚖",
            Self::Organization => "🏬",
            Self::Company => "🏢",
            Self::ShellCompany => "🐚",
            Self::Ngo => "🌱",
            Self::GovernmentBody => "🏰",
            Self::PoliticalParty => "🚩",
            Self::Court => "🔨",
            Self::LawFirm => "💼",
            Self::Bank => "🏦",
            Self::MediaOutlet => "📺",
            Self::CriminalOrg => "💀",
            Self::Domain => "🌐",
            Self::IpAddress => "📡",
            Self::Email => "✉",
            Self::Phone => "☎",
            Self::SocialAccount => "@",
            Self::Website => "🔗",
            Self::Server => "🖥",
            Self::Device => "📱",
            Self::CryptoWallet => "🔑",
            Self::BankAccount => "💳",
            Self::Transaction => "💱",
            Self::Loan => "💵",
            Self::Donation => "🎁",
            Self::Contract => "📜",
            Self::Invoice => "💲",
            Self::RegistryRecord => "📇",
            Self::Money => "💰",
            Self::Location => "📍",
            Self::Address => "🏠",
            Self::Property => "🏡",
            Self::Vehicle => "🚗",
            Self::Vessel => "🚢",
            Self::Aircraft => "✈",
            Self::Shipment => "📦",
            Self::Weapon => "🔫",
            Self::Asset => "💎",
            Self::Document => "📄",
            Self::Passport => "🛂",
            Self::Leak => "💧",
            Self::Dataset => "📊",
            Self::Photo => "📷",
            Self::Video => "🎥",
            Self::Article => "📰",
            Self::CourtRecord => "§",
            Self::Event => "📅",
            Self::Meeting => "👥",
            Self::Payment => "💸",
            Self::Travel => "🗺",
            Self::Other => "?",
        }
    }

    /// Node colour. Kinds in the same category share a hue family — people
    /// blues, orgs oranges, digital teals/greens, financial yellows, physical
    /// earth tones, documents grays, events reds — varying light/saturation
    /// so neighbours stay tellable apart.
    pub fn color(self) -> Color32 {
        match self {
            // people — blues
            Self::Person => Color32::from_rgb(100, 170, 255),
            Self::Alias => Color32::from_rgb(140, 150, 235),
            Self::Official => Color32::from_rgb(70, 135, 225),
            Self::Judge => Color32::from_rgb(110, 120, 215),
            // organizations — oranges
            Self::Organization => Color32::from_rgb(255, 170, 80),
            Self::Company => Color32::from_rgb(250, 150, 60),
            Self::ShellCompany => Color32::from_rgb(205, 145, 100),
            Self::Ngo => Color32::from_rgb(255, 200, 145),
            Self::GovernmentBody => Color32::from_rgb(210, 130, 55),
            Self::PoliticalParty => Color32::from_rgb(255, 135, 85),
            Self::Court => Color32::from_rgb(235, 165, 105),
            Self::LawFirm => Color32::from_rgb(240, 190, 110),
            Self::Bank => Color32::from_rgb(225, 155, 70),
            Self::MediaOutlet => Color32::from_rgb(255, 185, 125),
            Self::CriminalOrg => Color32::from_rgb(180, 105, 45),
            // digital — teals/greens
            Self::Domain => Color32::from_rgb(120, 210, 150),
            Self::IpAddress => Color32::from_rgb(90, 200, 200),
            Self::Email => Color32::from_rgb(110, 215, 185),
            Self::Phone => Color32::from_rgb(135, 205, 165),
            Self::SocialAccount => Color32::from_rgb(80, 210, 170),
            Self::Website => Color32::from_rgb(145, 220, 140),
            Self::Server => Color32::from_rgb(70, 185, 185),
            Self::Device => Color32::from_rgb(105, 220, 205),
            Self::CryptoWallet => Color32::from_rgb(60, 195, 155),
            // financial — yellows/golds
            Self::BankAccount => Color32::from_rgb(230, 205, 75),
            Self::Transaction => Color32::from_rgb(250, 215, 110),
            Self::Loan => Color32::from_rgb(215, 185, 90),
            Self::Donation => Color32::from_rgb(245, 225, 135),
            Self::Contract => Color32::from_rgb(235, 210, 120),
            Self::Invoice => Color32::from_rgb(225, 200, 100),
            Self::RegistryRecord => Color32::from_rgb(205, 185, 70),
            Self::Money => Color32::from_rgb(240, 220, 90),
            // physical — earth tones
            Self::Location => Color32::from_rgb(170, 150, 110),
            Self::Address => Color32::from_rgb(190, 165, 125),
            Self::Property => Color32::from_rgb(160, 135, 95),
            Self::Vehicle => Color32::from_rgb(200, 175, 135),
            Self::Vessel => Color32::from_rgb(150, 140, 105),
            Self::Aircraft => Color32::from_rgb(205, 185, 150),
            Self::Shipment => Color32::from_rgb(180, 155, 115),
            Self::Weapon => Color32::from_rgb(140, 115, 85),
            Self::Asset => Color32::from_rgb(210, 180, 140),
            // documents — grays
            Self::Document => Color32::from_rgb(200, 200, 200),
            Self::Passport => Color32::from_rgb(185, 190, 205),
            Self::Leak => Color32::from_rgb(170, 190, 200),
            Self::Dataset => Color32::from_rgb(175, 175, 185),
            Self::Photo => Color32::from_rgb(190, 190, 190),
            Self::Video => Color32::from_rgb(165, 165, 175),
            Self::Article => Color32::from_rgb(205, 205, 190),
            Self::CourtRecord => Color32::from_rgb(180, 180, 165),
            // events — reds
            Self::Event => Color32::from_rgb(255, 120, 120),
            Self::Meeting => Color32::from_rgb(240, 145, 145),
            Self::Payment => Color32::from_rgb(255, 110, 140),
            Self::Travel => Color32::from_rgb(230, 125, 105),
            // other
            Self::Other => Color32::from_rgb(170, 170, 170),
        }
    }

    /// Broad ontology group — palette colour-coding and picker sections.
    pub fn category(self) -> Category {
        match self {
            Self::Person | Self::Alias | Self::Official | Self::Judge => Category::People,
            Self::Organization
            | Self::Company
            | Self::ShellCompany
            | Self::Ngo
            | Self::GovernmentBody
            | Self::PoliticalParty
            | Self::Court
            | Self::LawFirm
            | Self::Bank
            | Self::MediaOutlet
            | Self::CriminalOrg => Category::Organizations,
            Self::Domain
            | Self::IpAddress
            | Self::Email
            | Self::Phone
            | Self::SocialAccount
            | Self::Website
            | Self::Server
            | Self::Device
            | Self::CryptoWallet => Category::Digital,
            Self::BankAccount
            | Self::Transaction
            | Self::Loan
            | Self::Donation
            | Self::Contract
            | Self::Invoice
            | Self::RegistryRecord
            | Self::Money => Category::Financial,
            Self::Location
            | Self::Address
            | Self::Property
            | Self::Vehicle
            | Self::Vessel
            | Self::Aircraft
            | Self::Shipment
            | Self::Weapon
            | Self::Asset => Category::Physical,
            Self::Document
            | Self::Passport
            | Self::Leak
            | Self::Dataset
            | Self::Photo
            | Self::Video
            | Self::Article
            | Self::CourtRecord => Category::Documents,
            Self::Event | Self::Meeting | Self::Payment | Self::Travel => Category::Events,
            Self::Other => Category::Other,
        }
    }

    /// The attribute keys an investigator predictably wants for this kind.
    /// `add_entity` pre-fills them with empty values; they're a prompt, not a
    /// schema — free-form keys remain first-class. Kept terse and snake_case
    /// so they diff well in the RON.
    pub fn default_attrs(self) -> &'static [&'static str] {
        match self {
            // people
            Self::Person => &["dob", "nationality", "occupation"],
            Self::Alias => &["context", "first_seen", "last_seen"],
            Self::Official => &["office", "jurisdiction", "party", "term"],
            Self::Judge => &["court", "jurisdiction", "appointed"],
            // organizations
            Self::Organization => &["type", "country", "founded"],
            Self::Company => &["reg_no", "jurisdiction", "incorporated", "status"],
            Self::ShellCompany => &["reg_no", "jurisdiction", "incorporated", "registered_agent"],
            Self::Ngo => &["reg_no", "jurisdiction", "founded", "stated_purpose"],
            Self::GovernmentBody => &["country", "level", "remit"],
            Self::PoliticalParty => &["country", "ideology", "founded"],
            Self::Court => &["jurisdiction", "level"],
            Self::LawFirm => &["jurisdiction", "specialty"],
            Self::Bank => &["swift_bic", "jurisdiction", "regulator"],
            Self::MediaOutlet => &["country", "medium", "owner"],
            Self::CriminalOrg => &["territory", "activity", "active_since"],
            // digital
            Self::Domain => &["registrar", "registered_on", "expires", "registrant"],
            Self::IpAddress => &["asn", "isp", "geolocation"],
            Self::Email => &["holder", "provider", "first_seen"],
            Self::Phone => &["country", "carrier", "holder"],
            Self::SocialAccount => &["platform", "handle", "holder", "created"],
            Self::Website => &["url", "operator", "first_archived"],
            Self::Server => &["ip", "hosting_provider", "location"],
            Self::Device => &["type", "imei", "owner"],
            Self::CryptoWallet => &["chain", "address", "custodian"],
            // financial
            Self::BankAccount => &["iban", "bank", "holder", "currency"],
            Self::Transaction => &["date", "amount", "currency", "from_account", "to_account"],
            Self::Loan => &["amount", "currency", "lender", "borrower", "date", "terms"],
            Self::Donation => &["amount", "currency", "donor", "recipient", "date"],
            Self::Contract => &["parties", "value", "signed", "subject"],
            Self::Invoice => &["number", "amount", "currency", "issuer", "billed_to", "date"],
            Self::RegistryRecord => &["registry", "record_no", "filed", "subject"],
            Self::Money => &["amount", "currency"],
            // physical
            Self::Location => &["country", "coordinates"],
            Self::Address => &["street", "city", "country", "postcode"],
            Self::Property => &["address", "type", "value", "registered_owner"],
            Self::Vehicle => &["plate", "make_model", "vin", "registered_owner"],
            Self::Vessel => &["imo", "mmsi", "flag", "owner"],
            Self::Aircraft => &["registration", "type", "operator", "owner"],
            Self::Shipment => &["bill_of_lading", "origin", "destination", "cargo", "date"],
            Self::Weapon => &["type", "serial_no", "origin"],
            Self::Asset => &["type", "value", "currency", "owner"],
            // documents & evidence
            Self::Document => &["date", "author", "file"],
            Self::Passport => &["number", "country", "issued", "expires", "holder"],
            Self::Leak => &["date", "size", "origin", "obtained_via"],
            Self::Dataset => &["source", "records", "format", "date"],
            Self::Photo => &["date", "place", "photographer", "file"],
            Self::Video => &["date", "place", "duration", "file"],
            Self::Article => &["outlet", "author", "published", "url"],
            Self::CourtRecord => &["case_no", "court", "date", "type"],
            // events
            Self::Event => &["date", "place"],
            Self::Meeting => &["date", "place", "participants"],
            Self::Payment => &["date", "amount", "currency", "method"],
            Self::Travel => &["date", "from", "to", "traveler", "purpose"],
            // other
            Self::Other => &[],
        }
    }
}

/// Node payload. The slug is the case-wide stable handle: relations and
/// display positions reference it, and it's what an external editor — human
/// or AI — uses to point at an entity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub slug: String,
    pub kind: EntityKind,
    pub label: String,
    pub attrs: Vec<(String, String)>,
    pub note: String,
    /// Path to an image file drawn on the board instead of the kind-coloured
    /// circle. None / unreadable means the circle fallback.
    pub image: Option<String>,
    /// Board scale multiplier — scenography, persisted per slug in the case
    /// display block. All node geometry and text scale by it.
    #[serde(default = "default_node_size")]
    pub size: f32,
}

/// New nodes draw at 2x the base metrics.
pub const DEFAULT_NODE_SIZE: f32 = 2.0;

fn default_node_size() -> f32 {
    DEFAULT_NODE_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Taxonomy invariants: every kind has a label/icon and pre-fills at most
    /// 6 attribute keys; `ALL` keeps categories in contiguous runs matching
    /// `Category::ALL` order so pickers can section off either.
    #[test]
    fn entity_kind_taxonomy_consistent() {
        for k in EntityKind::ALL {
            assert!(!k.label().is_empty(), "{k:?} label");
            assert!(!k.icon().is_empty(), "{k:?} icon");
            assert!(k.default_attrs().len() <= 6, "{k:?} attr budget");
        }
        let mut seen: Vec<Category> = Vec::new();
        for k in EntityKind::ALL {
            if seen.last() != Some(&k.category()) {
                assert!(!seen.contains(&k.category()), "ALL not grouped at {k:?}");
                seen.push(k.category());
            }
        }
        assert_eq!(seen, Category::ALL.to_vec(), "category order");
    }
}
