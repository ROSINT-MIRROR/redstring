//! Core investigation model: entities (nodes), relations (edges), and the
//! sources that justify each relation. The whole thesis of redstring lives on
//! the edge: a `Relation` with an empty `sources` vec is an unproven assertion.

use std::collections::BTreeMap;

use egui::Color32;
use serde::{Deserialize, Serialize};

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

    /// Coarse groups, in display order. Every [`EntityKind::category`] value
    /// appears here exactly once.
    // RECONCILE(chrome): the grid kind picker can group ALL by category() and
    // use this array for stable section order/headers.
    pub const CATEGORIES: [&'static str; 8] = [
        "People",
        "Organizations",
        "Digital",
        "Financial",
        "Physical",
        "Documents",
        "Events",
        "Other",
    ];

    /// Anchor colour of a category — the broad ontology hue every kind in the
    /// group varies around (see [`EntityKind::color`]). Drives the picker's
    /// section headers and cell tinting.
    pub fn category_color(category: &str) -> Color32 {
        match category {
            "People" => Color32::from_rgb(100, 170, 255),
            "Organizations" => Color32::from_rgb(255, 170, 80),
            "Digital" => Color32::from_rgb(100, 210, 175),
            "Financial" => Color32::from_rgb(235, 210, 95),
            "Physical" => Color32::from_rgb(185, 160, 120),
            "Documents" => Color32::from_rgb(195, 195, 200),
            "Events" => Color32::from_rgb(250, 125, 125),
            _ => Color32::from_rgb(170, 170, 170),
        }
    }

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
    /// (both sit in the default Proportional fallback chain); `@`/`?`/`§`
    /// come from the text fonts. Don't add glyphs without checking — e.g.
    /// 🏛 ⚖-alternatives 🗳 🧾 ₿ 🤝 🧳 are NOT bundled.
    // RECONCILE(images): when an entity carries an image, the textured node
    // should win; this glyph stays the fallback for imageless entities.
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

    /// Coarse group for picker grouping and the palette doc above. Values are
    /// exactly the entries of [`EntityKind::CATEGORIES`].
    // RECONCILE(chrome): grid kind picker groups by this — treat the strings
    // as stable UI headers.
    pub fn category(self) -> &'static str {
        match self {
            Self::Person | Self::Alias | Self::Official | Self::Judge => "People",
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
            | Self::CriminalOrg => "Organizations",
            Self::Domain
            | Self::IpAddress
            | Self::Email
            | Self::Phone
            | Self::SocialAccount
            | Self::Website
            | Self::Server
            | Self::Device
            | Self::CryptoWallet => "Digital",
            Self::BankAccount
            | Self::Transaction
            | Self::Loan
            | Self::Donation
            | Self::Contract
            | Self::Invoice
            | Self::RegistryRecord
            | Self::Money => "Financial",
            Self::Location
            | Self::Address
            | Self::Property
            | Self::Vehicle
            | Self::Vessel
            | Self::Aircraft
            | Self::Shipment
            | Self::Weapon
            | Self::Asset => "Physical",
            Self::Document
            | Self::Passport
            | Self::Leak
            | Self::Dataset
            | Self::Photo
            | Self::Video
            | Self::Article
            | Self::CourtRecord => "Documents",
            Self::Event | Self::Meeting | Self::Payment | Self::Travel => "Events",
            Self::Other => "Other",
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
    /// Path to an image file (mugshot, logo, …) drawn on the board instead of
    /// the kind-coloured circle. None / unreadable ⇒ circle fallback.
    pub image: Option<String>,
    /// Board scale multiplier — scenography, lives in `CaseDisplay::sizes`
    /// in the case file, not in `data`. All node geometry and text scale by it.
    #[serde(default = "default_node_size")]
    pub size: f32,
}

/// New nodes draw at 2× the base metrics.
pub const DEFAULT_NODE_SIZE: f32 = 2.0;

fn default_node_size() -> f32 {
    DEFAULT_NODE_SIZE
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
    #[serde(default)]
    pub image: Option<String>,
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
    /// slug → node scale multiplier. Missing ⇒ [`DEFAULT_NODE_SIZE`].
    #[serde(default)]
    pub sizes: BTreeMap<String, f32>,
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

    /// Taxonomy invariants: every kind has a label/icon, sits in a known
    /// category, and pre-fills at most 6 attribute keys.
    #[test]
    fn entity_kind_taxonomy_consistent() {
        for k in EntityKind::ALL {
            assert!(!k.label().is_empty(), "{k:?} label");
            assert!(!k.icon().is_empty(), "{k:?} icon");
            assert!(
                EntityKind::CATEGORIES.contains(&k.category()),
                "{k:?} category '{}' not in CATEGORIES",
                k.category()
            );
            assert!(k.default_attrs().len() <= 6, "{k:?} attr budget");
        }
        // ALL is grouped: each category occupies one contiguous run, in
        // CATEGORIES order — pickers can render section headers off it.
        let mut seen: Vec<&str> = Vec::new();
        for k in EntityKind::ALL {
            if seen.last() != Some(&k.category()) {
                assert!(!seen.contains(&k.category()), "ALL not grouped at {k:?}");
                seen.push(k.category());
            }
        }
        assert_eq!(seen, EntityKind::CATEGORIES, "category order");
    }

    /// The AI-editing contract: a minimal hand-written file — no display, no
    /// sources, no attrs/note/excerpt — must load, with everything defaulted.
    #[test]
    fn minimal_external_edit_loads() {
        let s = r#"(
            data: (
                entities: [
                    (slug: "doe", kind: Person, label: "J. Doe"),
                    (slug: "shell", kind: ShellCompany, label: "Shell Co"),
                ],
                relations: [
                    (from: "doe", to: "shell", kind: Controls),
                ],
            ),
        )"#;
        let cf: CaseFile = ron::from_str(s).expect("minimal file must parse");
        assert_eq!(cf.data.entities.len(), 2);
        assert_eq!(cf.data.relations.len(), 1);
        assert!(cf.data.entities[0].image.is_none());
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
            image: Some("mugshots/doe.png".into()),
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
