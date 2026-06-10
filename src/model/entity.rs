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

    pub aliases: &'static [&'static str],
}

const fn k(
    kind: EntityKind,
    label: &'static str,
    icon: &'static str,
    category: Category,
    rgb: (u8, u8, u8),
    attrs: &'static [&'static str],
    aliases: &'static [&'static str],
) -> KindMeta {
    KindMeta {
        kind,
        label,
        icon,
        category,
        color: Color32::from_rgb(rgb.0, rgb.1, rgb.2),
        attrs,
        aliases,
    }
}

#[rustfmt::skip]
pub const KIND_META: [KindMeta; 54] = [
    k(EntityKind::Person, "Person", egui_phosphor::regular::USER, Category::People, (100, 170, 255), &["first_name", "last_name", "dob", "nationality", "occupation"], &["individual", "human", "people", "name"]),
    k(EntityKind::Alias, "Alias", egui_phosphor::regular::USER_SWITCH, Category::People, (140, 150, 235), &["context", "first_seen", "last_seen"], &["aka", "nickname", "pseudonym", "handle"]),
    k(EntityKind::Official, "Official", egui_phosphor::regular::IDENTIFICATION_BADGE, Category::People, (70, 135, 225), &["office", "jurisdiction", "party", "term"], &["politician", "minister", "mp", "bureaucrat", "public servant"]),
    k(EntityKind::Judge, "Judge", egui_phosphor::regular::SCALES, Category::People, (110, 120, 215), &["court", "jurisdiction", "appointed"], &["magistrate", "justice"]),
    k(EntityKind::Organization, "Organization", egui_phosphor::regular::BUILDINGS, Category::Organizations, (255, 170, 80), &["type", "country", "founded"], &["org", "group", "association"]),
    k(EntityKind::Company, "Company", egui_phosphor::regular::BUILDING_OFFICE, Category::Organizations, (250, 150, 60), &["reg_no", "jurisdiction", "incorporated", "status"], &["corporation", "business", "firm", "llc", "enterprise"]),
    k(EntityKind::ShellCompany, "Shell company", egui_phosphor::regular::CUBE_TRANSPARENT, Category::Organizations, (205, 145, 100), &["reg_no", "jurisdiction", "incorporated", "registered_agent"], &["offshore", "front", "spv", "letterbox"]),
    k(EntityKind::Ngo, "NGO", egui_phosphor::regular::HAND_HEART, Category::Organizations, (255, 200, 145), &["reg_no", "jurisdiction", "founded", "stated_purpose"], &["nonprofit", "charity", "foundation"]),
    k(EntityKind::GovernmentBody, "Government body", egui_phosphor::regular::BANK, Category::Organizations, (210, 130, 55), &["country", "level", "remit"], &["agency", "ministry", "department", "state", "authority"]),
    k(EntityKind::PoliticalParty, "Political party", egui_phosphor::regular::FLAG_BANNER, Category::Organizations, (255, 135, 85), &["country", "ideology", "founded"], &["party"]),
    k(EntityKind::Court, "Court", egui_phosphor::regular::GAVEL, Category::Organizations, (235, 165, 105), &["jurisdiction", "level"], &["tribunal", "judiciary"]),
    k(EntityKind::LawFirm, "Law firm", egui_phosphor::regular::BRIEFCASE, Category::Organizations, (240, 190, 110), &["jurisdiction", "specialty"], &["lawyers", "attorneys", "legal", "counsel"]),
    k(EntityKind::Bank, "Bank", egui_phosphor::regular::VAULT, Category::Organizations, (225, 155, 70), &["swift_bic", "jurisdiction", "regulator"], &["financial institution", "lender"]),
    k(EntityKind::MediaOutlet, "Media outlet", egui_phosphor::regular::BROADCAST, Category::Organizations, (255, 185, 125), &["country", "medium", "owner"], &["news", "press", "tv", "newspaper", "publication"]),
    k(EntityKind::CriminalOrg, "Criminal org", egui_phosphor::regular::SKULL, Category::Organizations, (180, 105, 45), &["territory", "activity", "active_since"], &["mafia", "cartel", "gang", "mob", "syndicate"]),
    k(EntityKind::Domain, "Domain", egui_phosphor::regular::GLOBE, Category::Digital, (120, 210, 150), &["registrar", "registered_on", "expires", "registrant"], &["dns", "site name"]),
    k(EntityKind::IpAddress, "IP address", egui_phosphor::regular::NETWORK, Category::Digital, (90, 200, 200), &["asn", "isp", "geolocation"], &["ip", "host", "ipv4", "ipv6"]),
    k(EntityKind::Email, "Email", egui_phosphor::regular::ENVELOPE, Category::Digital, (110, 215, 185), &["holder", "provider", "first_seen"], &["mail", "inbox"]),
    k(EntityKind::Phone, "Phone", egui_phosphor::regular::PHONE, Category::Digital, (135, 205, 165), &["country", "carrier", "holder"], &["mobile", "telephone", "number", "cell"]),
    k(EntityKind::SocialAccount, "Social account", egui_phosphor::regular::AT, Category::Digital, (80, 210, 170), &["platform", "handle", "holder", "created"], &["twitter", "facebook", "instagram", "telegram", "profile", "x"]),
    k(EntityKind::Website, "Website", egui_phosphor::regular::BROWSER, Category::Digital, (145, 220, 140), &["url", "operator", "first_archived"], &["site", "url", "webpage"]),
    k(EntityKind::Server, "Server", egui_phosphor::regular::HARD_DRIVES, Category::Digital, (70, 185, 185), &["ip", "hosting_provider", "location"], &["host", "vps", "infrastructure", "machine"]),
    k(EntityKind::Device, "Device", egui_phosphor::regular::DEVICE_MOBILE, Category::Digital, (105, 220, 205), &["type", "imei", "owner"], &["laptop", "computer", "hardware", "gadget"]),
    k(EntityKind::CryptoWallet, "Crypto wallet", egui_phosphor::regular::CURRENCY_BTC, Category::Digital, (60, 195, 155), &["chain", "address", "custodian"], &["bitcoin", "btc", "eth", "blockchain", "crypto"]),
    k(EntityKind::BankAccount, "Bank account", egui_phosphor::regular::CREDIT_CARD, Category::Financial, (230, 205, 75), &["iban", "bank", "holder", "currency"], &["iban", "account", "swift"]),
    k(EntityKind::Transaction, "Transaction", egui_phosphor::regular::ARROWS_LEFT_RIGHT, Category::Financial, (250, 215, 110), &["date", "amount", "currency", "from_account", "to_account"], &["transfer", "wire", "remittance"]),
    k(EntityKind::Loan, "Loan", egui_phosphor::regular::HAND_COINS, Category::Financial, (215, 185, 90), &["amount", "currency", "lender", "borrower", "date", "terms"], &["credit", "debt", "mortgage"]),
    k(EntityKind::Donation, "Donation", egui_phosphor::regular::GIFT, Category::Financial, (245, 225, 135), &["amount", "currency", "donor", "recipient", "date"], &["contribution", "funding", "grant"]),
    k(EntityKind::Contract, "Contract", egui_phosphor::regular::SCROLL, Category::Financial, (235, 210, 120), &["parties", "value", "signed", "subject"], &["agreement", "deal", "mou"]),
    k(EntityKind::Invoice, "Invoice", egui_phosphor::regular::RECEIPT, Category::Financial, (225, 200, 100), &["number", "amount", "currency", "issuer", "billed_to", "date"], &["bill"]),
    k(EntityKind::RegistryRecord, "Registry record", egui_phosphor::regular::ARCHIVE, Category::Financial, (205, 185, 70), &["registry", "record_no", "filed", "subject"], &["filing", "registration", "incorporation record"]),
    k(EntityKind::Money, "Money", egui_phosphor::regular::MONEY, Category::Financial, (240, 220, 90), &["amount", "currency"], &["cash", "funds", "sum"]),
    k(EntityKind::Location, "Location", egui_phosphor::regular::MAP_PIN, Category::Physical, (170, 150, 110), &["country", "coordinates"], &["place", "spot", "coordinates", "geo"]),
    k(EntityKind::Address, "Address", egui_phosphor::regular::HOUSE_LINE, Category::Physical, (190, 165, 125), &["street", "city", "country", "postcode"], &["street", "residence", "domicile"]),
    k(EntityKind::Property, "Property", egui_phosphor::regular::BUILDING_APARTMENT, Category::Physical, (160, 135, 95), &["address", "type", "value", "registered_owner"], &["real estate", "estate", "building", "land", "villa", "mansion"]),
    k(EntityKind::Vehicle, "Vehicle", egui_phosphor::regular::CAR, Category::Physical, (200, 175, 135), &["plate", "make_model", "vin", "registered_owner"], &["car", "truck", "auto", "motorcycle"]),
    k(EntityKind::Vessel, "Vessel", egui_phosphor::regular::BOAT, Category::Physical, (150, 140, 105), &["imo", "mmsi", "flag", "owner"], &["ship", "boat", "yacht"]),
    k(EntityKind::Aircraft, "Aircraft", egui_phosphor::regular::AIRPLANE, Category::Physical, (205, 185, 150), &["registration", "type", "operator", "owner"], &["plane", "jet", "helicopter"]),
    k(EntityKind::Shipment, "Shipment", egui_phosphor::regular::PACKAGE, Category::Physical, (180, 155, 115), &["bill_of_lading", "origin", "destination", "cargo", "date"], &["cargo", "container", "freight", "parcel"]),
    k(EntityKind::Weapon, "Weapon", egui_phosphor::regular::CROSSHAIR, Category::Physical, (140, 115, 85), &["type", "serial_no", "origin"], &["gun", "firearm", "arms", "pistol", "rifle", "knife"]),
    k(EntityKind::Asset, "Asset", egui_phosphor::regular::DIAMOND, Category::Physical, (210, 180, 140), &["type", "value", "currency", "owner"], &["valuables", "holding", "artwork", "jewelry"]),
    k(EntityKind::Document, "Document", egui_phosphor::regular::FILE_TEXT, Category::Documents, (200, 200, 200), &["date", "author", "file"], &["file", "paper", "doc", "memo"]),
    k(EntityKind::Passport, "Passport", egui_phosphor::regular::IDENTIFICATION_CARD, Category::Documents, (185, 190, 205), &["number", "country", "issued", "expires", "holder"], &["travel document", "visa", "id"]),
    k(EntityKind::Leak, "Leak", egui_phosphor::regular::DROP, Category::Documents, (170, 190, 200), &["date", "size", "origin", "obtained_via"], &["dump", "breach", "disclosure"]),
    k(EntityKind::Dataset, "Dataset", egui_phosphor::regular::DATABASE, Category::Documents, (175, 175, 185), &["source", "records", "format", "date"], &["data", "database", "records", "spreadsheet"]),
    k(EntityKind::Photo, "Photo", egui_phosphor::regular::CAMERA, Category::Documents, (190, 190, 190), &["date", "place", "photographer", "file"], &["image", "picture", "photograph"]),
    k(EntityKind::Video, "Video", egui_phosphor::regular::VIDEO_CAMERA, Category::Documents, (165, 165, 175), &["date", "place", "duration", "file"], &["footage", "recording", "clip"]),
    k(EntityKind::Article, "Article", egui_phosphor::regular::NEWSPAPER, Category::Documents, (205, 205, 190), &["outlet", "author", "published", "url"], &["story", "report", "news piece"]),
    k(EntityKind::CourtRecord, "Court record", egui_phosphor::regular::STAMP, Category::Documents, (180, 180, 165), &["case_no", "court", "date", "type"], &["case file", "docket", "ruling", "verdict", "indictment", "deposition"]),
    k(EntityKind::Event, "Event", egui_phosphor::regular::CALENDAR, Category::Events, (255, 120, 120), &["date", "place"], &["incident", "happening", "occurrence"]),
    k(EntityKind::Meeting, "Meeting", egui_phosphor::regular::USERS_THREE, Category::Events, (240, 145, 145), &["date", "place", "participants"], &["encounter", "appointment", "summit", "rendezvous"]),
    k(EntityKind::Payment, "Payment", egui_phosphor::regular::CURRENCY_CIRCLE_DOLLAR, Category::Events, (255, 110, 140), &["date", "amount", "currency", "method"], &["payout", "fee", "bribe", "kickback"]),
    k(EntityKind::Travel, "Travel", egui_phosphor::regular::SUITCASE_ROLLING, Category::Events, (230, 125, 105), &["date", "from", "to", "traveler", "purpose"], &["trip", "flight", "journey", "visit"]),
    k(EntityKind::Other, "Other", egui_phosphor::regular::QUESTION, Category::Other, (170, 170, 170), &[], &["misc", "unknown", "custom"]),
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
