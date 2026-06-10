use egui::Color32;
use serde::{Deserialize, Serialize};

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

pub struct CategoryMeta {
    pub category: Category,
    pub label: &'static str,
    pub color: Color32,
}

const fn c(category: Category, label: &'static str, rgb: (u8, u8, u8)) -> CategoryMeta {
    CategoryMeta {
        category,
        label,
        color: Color32::from_rgb(rgb.0, rgb.1, rgb.2),
    }
}

pub const CATEGORY_META: [CategoryMeta; 8] = [
    c(Category::People, "People", (100, 170, 255)),
    c(Category::Organizations, "Organizations", (255, 170, 80)),
    c(Category::Digital, "Digital", (100, 210, 175)),
    c(Category::Financial, "Financial", (235, 210, 95)),
    c(Category::Physical, "Physical", (185, 160, 120)),
    c(Category::Documents, "Documents", (195, 195, 200)),
    c(Category::Events, "Events", (250, 125, 125)),
    c(Category::Other, "Other", (170, 170, 170)),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum EntityKind {
    Person,
    Alias,
    Official,
    Judge,
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
    Domain,
    IpAddress,
    Email,
    Phone,
    SocialAccount,
    Website,
    Server,
    Device,
    CryptoWallet,
    BankAccount,
    Transaction,
    Loan,
    Donation,
    Contract,
    Invoice,
    RegistryRecord,
    Money,
    Location,
    Address,
    Property,
    Vehicle,
    Vessel,
    Aircraft,
    Shipment,
    Weapon,
    Asset,
    Document,
    Passport,
    Leak,
    Dataset,
    Photo,
    Video,
    Article,
    CourtRecord,
    Event,
    Meeting,
    Payment,
    Travel,
    Other,
}

pub struct KindMeta {
    pub kind: EntityKind,
    pub label: &'static str,
    pub icon: &'static str,
    pub category: Category,
    pub color: Color32,
    pub attrs: &'static [&'static str],
}

const fn k(
    kind: EntityKind,
    label: &'static str,
    icon: &'static str,
    category: Category,
    rgb: (u8, u8, u8),
    attrs: &'static [&'static str],
) -> KindMeta {
    KindMeta {
        kind,
        label,
        icon,
        category,
        color: Color32::from_rgb(rgb.0, rgb.1, rgb.2),
        attrs,
    }
}

#[rustfmt::skip]
pub const KIND_META: [KindMeta; 54] = [
    k(EntityKind::Person, "Person", "👤", Category::People, (100, 170, 255), &["dob", "nationality", "occupation"]),
    k(EntityKind::Alias, "Alias", "🎭", Category::People, (140, 150, 235), &["context", "first_seen", "last_seen"]),
    k(EntityKind::Official, "Official", "👔", Category::People, (70, 135, 225), &["office", "jurisdiction", "party", "term"]),
    k(EntityKind::Judge, "Judge", "⚖", Category::People, (110, 120, 215), &["court", "jurisdiction", "appointed"]),
    k(EntityKind::Organization, "Organization", "🏬", Category::Organizations, (255, 170, 80), &["type", "country", "founded"]),
    k(EntityKind::Company, "Company", "🏢", Category::Organizations, (250, 150, 60), &["reg_no", "jurisdiction", "incorporated", "status"]),
    k(EntityKind::ShellCompany, "Shell company", "🐚", Category::Organizations, (205, 145, 100), &["reg_no", "jurisdiction", "incorporated", "registered_agent"]),
    k(EntityKind::Ngo, "NGO", "🌱", Category::Organizations, (255, 200, 145), &["reg_no", "jurisdiction", "founded", "stated_purpose"]),
    k(EntityKind::GovernmentBody, "Government body", "🏰", Category::Organizations, (210, 130, 55), &["country", "level", "remit"]),
    k(EntityKind::PoliticalParty, "Political party", "🚩", Category::Organizations, (255, 135, 85), &["country", "ideology", "founded"]),
    k(EntityKind::Court, "Court", "🔨", Category::Organizations, (235, 165, 105), &["jurisdiction", "level"]),
    k(EntityKind::LawFirm, "Law firm", "💼", Category::Organizations, (240, 190, 110), &["jurisdiction", "specialty"]),
    k(EntityKind::Bank, "Bank", "🏦", Category::Organizations, (225, 155, 70), &["swift_bic", "jurisdiction", "regulator"]),
    k(EntityKind::MediaOutlet, "Media outlet", "📺", Category::Organizations, (255, 185, 125), &["country", "medium", "owner"]),
    k(EntityKind::CriminalOrg, "Criminal org", "💀", Category::Organizations, (180, 105, 45), &["territory", "activity", "active_since"]),
    k(EntityKind::Domain, "Domain", "🌐", Category::Digital, (120, 210, 150), &["registrar", "registered_on", "expires", "registrant"]),
    k(EntityKind::IpAddress, "IP address", "📡", Category::Digital, (90, 200, 200), &["asn", "isp", "geolocation"]),
    k(EntityKind::Email, "Email", "✉", Category::Digital, (110, 215, 185), &["holder", "provider", "first_seen"]),
    k(EntityKind::Phone, "Phone", "☎", Category::Digital, (135, 205, 165), &["country", "carrier", "holder"]),
    k(EntityKind::SocialAccount, "Social account", "@", Category::Digital, (80, 210, 170), &["platform", "handle", "holder", "created"]),
    k(EntityKind::Website, "Website", "🔗", Category::Digital, (145, 220, 140), &["url", "operator", "first_archived"]),
    k(EntityKind::Server, "Server", "🖥", Category::Digital, (70, 185, 185), &["ip", "hosting_provider", "location"]),
    k(EntityKind::Device, "Device", "📱", Category::Digital, (105, 220, 205), &["type", "imei", "owner"]),
    k(EntityKind::CryptoWallet, "Crypto wallet", "🔑", Category::Digital, (60, 195, 155), &["chain", "address", "custodian"]),
    k(EntityKind::BankAccount, "Bank account", "💳", Category::Financial, (230, 205, 75), &["iban", "bank", "holder", "currency"]),
    k(EntityKind::Transaction, "Transaction", "💱", Category::Financial, (250, 215, 110), &["date", "amount", "currency", "from_account", "to_account"]),
    k(EntityKind::Loan, "Loan", "💵", Category::Financial, (215, 185, 90), &["amount", "currency", "lender", "borrower", "date", "terms"]),
    k(EntityKind::Donation, "Donation", "🎁", Category::Financial, (245, 225, 135), &["amount", "currency", "donor", "recipient", "date"]),
    k(EntityKind::Contract, "Contract", "📜", Category::Financial, (235, 210, 120), &["parties", "value", "signed", "subject"]),
    k(EntityKind::Invoice, "Invoice", "💲", Category::Financial, (225, 200, 100), &["number", "amount", "currency", "issuer", "billed_to", "date"]),
    k(EntityKind::RegistryRecord, "Registry record", "📇", Category::Financial, (205, 185, 70), &["registry", "record_no", "filed", "subject"]),
    k(EntityKind::Money, "Money", "💰", Category::Financial, (240, 220, 90), &["amount", "currency"]),
    k(EntityKind::Location, "Location", "📍", Category::Physical, (170, 150, 110), &["country", "coordinates"]),
    k(EntityKind::Address, "Address", "🏠", Category::Physical, (190, 165, 125), &["street", "city", "country", "postcode"]),
    k(EntityKind::Property, "Property", "🏡", Category::Physical, (160, 135, 95), &["address", "type", "value", "registered_owner"]),
    k(EntityKind::Vehicle, "Vehicle", "🚗", Category::Physical, (200, 175, 135), &["plate", "make_model", "vin", "registered_owner"]),
    k(EntityKind::Vessel, "Vessel", "🚢", Category::Physical, (150, 140, 105), &["imo", "mmsi", "flag", "owner"]),
    k(EntityKind::Aircraft, "Aircraft", "✈", Category::Physical, (205, 185, 150), &["registration", "type", "operator", "owner"]),
    k(EntityKind::Shipment, "Shipment", "📦", Category::Physical, (180, 155, 115), &["bill_of_lading", "origin", "destination", "cargo", "date"]),
    k(EntityKind::Weapon, "Weapon", "🔫", Category::Physical, (140, 115, 85), &["type", "serial_no", "origin"]),
    k(EntityKind::Asset, "Asset", "💎", Category::Physical, (210, 180, 140), &["type", "value", "currency", "owner"]),
    k(EntityKind::Document, "Document", "📄", Category::Documents, (200, 200, 200), &["date", "author", "file"]),
    k(EntityKind::Passport, "Passport", "🛂", Category::Documents, (185, 190, 205), &["number", "country", "issued", "expires", "holder"]),
    k(EntityKind::Leak, "Leak", "💧", Category::Documents, (170, 190, 200), &["date", "size", "origin", "obtained_via"]),
    k(EntityKind::Dataset, "Dataset", "📊", Category::Documents, (175, 175, 185), &["source", "records", "format", "date"]),
    k(EntityKind::Photo, "Photo", "📷", Category::Documents, (190, 190, 190), &["date", "place", "photographer", "file"]),
    k(EntityKind::Video, "Video", "🎥", Category::Documents, (165, 165, 175), &["date", "place", "duration", "file"]),
    k(EntityKind::Article, "Article", "📰", Category::Documents, (205, 205, 190), &["outlet", "author", "published", "url"]),
    k(EntityKind::CourtRecord, "Court record", "§", Category::Documents, (180, 180, 165), &["case_no", "court", "date", "type"]),
    k(EntityKind::Event, "Event", "📅", Category::Events, (255, 120, 120), &["date", "place"]),
    k(EntityKind::Meeting, "Meeting", "👥", Category::Events, (240, 145, 145), &["date", "place", "participants"]),
    k(EntityKind::Payment, "Payment", "💸", Category::Events, (255, 110, 140), &["date", "amount", "currency", "method"]),
    k(EntityKind::Travel, "Travel", "🗺", Category::Events, (230, 125, 105), &["date", "from", "to", "traveler", "purpose"]),
    k(EntityKind::Other, "Other", "?", Category::Other, (170, 170, 170), &[]),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub slug: String,
    pub kind: EntityKind,
    pub label: String,
    pub attrs: Vec<(String, String)>,
    pub note: String,
    pub image: Option<String>,
    #[serde(default = "default_node_size")]
    pub size: f32,
}

pub const DEFAULT_NODE_SIZE: f32 = 2.0;

fn default_node_size() -> f32 {
    DEFAULT_NODE_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_mirror_declaration_order() {
        for (i, m) in CATEGORY_META.iter().enumerate() {
            assert_eq!(m.category as usize, i, "CATEGORY_META[{i}] out of order");
        }
        for (i, m) in KIND_META.iter().enumerate() {
            assert_eq!(m.kind as usize, i, "KIND_META[{i}] is {:?}", m.kind);
        }
    }

    #[test]
    fn entity_kind_taxonomy_consistent() {
        for m in &KIND_META {
            assert!(!m.label.is_empty(), "{:?} label", m.kind);
            assert!(!m.icon.is_empty(), "{:?} icon", m.kind);
            assert!(m.attrs.len() <= 6, "{:?} attr budget", m.kind);
        }
        let mut seen: Vec<Category> = Vec::new();
        for m in &KIND_META {
            if seen.last() != Some(&m.category) {
                assert!(!seen.contains(&m.category), "grouping broken at {:?}", m.kind);
                seen.push(m.category);
            }
        }
        let cats: Vec<Category> = CATEGORY_META.iter().map(|m| m.category).collect();
        assert_eq!(seen, cats, "category order");
    }
}
