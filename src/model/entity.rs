use egui::Color32;
use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::observation::Observation;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValueFormat {
    Free,
    Ip,
    Hostname,
    Email,
    Url,
}


pub fn format_hint(format: ValueFormat) -> Result<&'static str, Error> {
    Ok(match format {
        ValueFormat::Free => "free text",
        ValueFormat::Ip => "IP address",
        ValueFormat::Hostname => "hostname",
        ValueFormat::Email => "email address",
        ValueFormat::Url => "http(s) URL",
    })
}


pub fn validate_value(format: ValueFormat, s: &str) -> Result<bool, Error> {
    let s = s.trim();
    Ok(match format {
        ValueFormat::Free => true,
        ValueFormat::Ip => s.parse::<std::net::IpAddr>().is_ok(),
        ValueFormat::Hostname => is_hostname(s),
        ValueFormat::Email => is_email(s),
        ValueFormat::Url => is_url(s),
    })
}

fn is_hostname(s: &str) -> bool {
    let s = s.to_lowercase();
    if s.is_empty() || s.len() > 253 || s.contains(' ') {
        return false;
    }
    let labels: Vec<&str> = s.split('.').collect();
    let Some(last) = labels.last() else {
        return false;
    };
    if !last.chars().any(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    labels.iter().all(|l| {
        !l.is_empty()
            && l.len() <= 63
            && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !l.starts_with('-')
            && !l.ends_with('-')
    })
}

fn is_email(s: &str) -> bool {
    if s.contains(' ') {
        return false;
    }
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    !local.is_empty() && !domain.contains('@') && is_hostname(domain)
}

fn strip_scheme(s: &str) -> Option<&str> {
    let Some(r) = s.strip_prefix("https://") else {
        return s.strip_prefix("http://");
    };
    Some(r)
}

fn is_url(s: &str) -> bool {
    let Some(rest) = strip_scheme(s) else {
        return false;
    };
    let Some(host) = rest.split(['/', '?', '#']).next() else {
        return false;
    };
    let Some(host) = host.split(':').next() else {
        return false;
    };
    !rest.contains(' ') && (is_hostname(host) || host.parse::<std::net::IpAddr>().is_ok())
}


pub fn url_host(s: &str) -> Result<Option<String>, Error> {
    let Some(rest) = strip_scheme(s.trim()) else {
        return Ok(None);
    };
    let Some(host) = rest.split(['/', '?', '#']).next() else {
        return Ok(None);
    };
    let Some(host) = host.split(':').next() else {
        return Ok(None);
    };
    let host = host.trim_start_matches("www.");
    if host.is_empty() {
        return Ok(None);
    }
    Ok(Some(host.to_lowercase()))
}

pub struct KindMeta {
    pub kind: EntityKind,
    pub label: &'static str,
    pub icon: &'static str,
    pub category: Category,
    pub color: Color32,
    pub attrs: &'static [&'static str],

    pub aliases: &'static [&'static str],

    pub value_format: ValueFormat,
}

const fn k(
    kind: EntityKind,
    label: &'static str,
    icon: &'static str,
    category: Category,
    rgb: (u8, u8, u8),
    attrs: &'static [&'static str],
    aliases: &'static [&'static str],
    value_format: ValueFormat,
) -> KindMeta {
    KindMeta {
        kind,
        label,
        icon,
        category,
        color: Color32::from_rgb(rgb.0, rgb.1, rgb.2),
        attrs,
        aliases,
        value_format,
    }
}

#[rustfmt::skip]
pub const KIND_META: [KindMeta; 54] = [
    k(EntityKind::Person, "Person", egui_phosphor::regular::USER, Category::People, (100, 170, 255), &["first_name", "last_name", "dob", "nationality", "occupation"], &["individual", "human", "people", "name"], ValueFormat::Free),
    k(EntityKind::Alias, "Alias", egui_phosphor::regular::USER_SWITCH, Category::People, (140, 150, 235), &["context", "first_seen", "last_seen"], &["aka", "nickname", "pseudonym", "handle"], ValueFormat::Free),
    k(EntityKind::Official, "Official", egui_phosphor::regular::IDENTIFICATION_BADGE, Category::People, (70, 135, 225), &["office", "jurisdiction", "party", "term"], &["politician", "minister", "mp", "bureaucrat", "public servant"], ValueFormat::Free),
    k(EntityKind::Judge, "Judge", egui_phosphor::regular::SCALES, Category::People, (110, 120, 215), &["court", "jurisdiction", "appointed"], &["magistrate", "justice"], ValueFormat::Free),
    k(EntityKind::Organization, "Organization", egui_phosphor::regular::BUILDINGS, Category::Organizations, (255, 170, 80), &["type", "country", "founded"], &["org", "group", "association"], ValueFormat::Free),
    k(EntityKind::Company, "Company", egui_phosphor::regular::BUILDING_OFFICE, Category::Organizations, (250, 150, 60), &["reg_no", "jurisdiction", "incorporated", "status"], &["corporation", "business", "firm", "llc", "enterprise"], ValueFormat::Free),
    k(EntityKind::ShellCompany, "Shell company", egui_phosphor::regular::CUBE_TRANSPARENT, Category::Organizations, (205, 145, 100), &["reg_no", "jurisdiction", "incorporated", "registered_agent"], &["offshore", "front", "spv", "letterbox"], ValueFormat::Free),
    k(EntityKind::Ngo, "NGO", egui_phosphor::regular::HAND_HEART, Category::Organizations, (255, 200, 145), &["reg_no", "jurisdiction", "founded", "stated_purpose"], &["nonprofit", "charity", "foundation"], ValueFormat::Free),
    k(EntityKind::GovernmentBody, "Government body", egui_phosphor::regular::BANK, Category::Organizations, (210, 130, 55), &["country", "level", "remit"], &["agency", "ministry", "department", "state", "authority"], ValueFormat::Free),
    k(EntityKind::PoliticalParty, "Political party", egui_phosphor::regular::FLAG_BANNER, Category::Organizations, (255, 135, 85), &["country", "ideology", "founded"], &["party"], ValueFormat::Free),
    k(EntityKind::Court, "Court", egui_phosphor::regular::GAVEL, Category::Organizations, (235, 165, 105), &["jurisdiction", "level"], &["tribunal", "judiciary"], ValueFormat::Free),
    k(EntityKind::LawFirm, "Law firm", egui_phosphor::regular::BRIEFCASE, Category::Organizations, (240, 190, 110), &["jurisdiction", "specialty"], &["lawyers", "attorneys", "legal", "counsel"], ValueFormat::Free),
    k(EntityKind::Bank, "Bank", egui_phosphor::regular::VAULT, Category::Organizations, (225, 155, 70), &["swift_bic", "jurisdiction", "regulator"], &["financial institution", "lender"], ValueFormat::Free),
    k(EntityKind::MediaOutlet, "Media outlet", egui_phosphor::regular::BROADCAST, Category::Organizations, (255, 185, 125), &["country", "medium", "owner"], &["news", "press", "tv", "newspaper", "publication"], ValueFormat::Free),
    k(EntityKind::CriminalOrg, "Criminal org", egui_phosphor::regular::SKULL, Category::Organizations, (180, 105, 45), &["territory", "activity", "active_since"], &["mafia", "cartel", "gang", "mob", "syndicate"], ValueFormat::Free),
    k(EntityKind::Domain, "Domain", egui_phosphor::regular::GLOBE, Category::Digital, (120, 210, 150), &["registrar", "registered_on", "expires", "registrant"], &["dns", "site name"], ValueFormat::Hostname),
    k(EntityKind::IpAddress, "IP address", egui_phosphor::regular::NETWORK, Category::Digital, (90, 200, 200), &["asn", "isp", "geolocation"], &["ip", "host", "ipv4", "ipv6"], ValueFormat::Ip),
    k(EntityKind::Email, "Email", egui_phosphor::regular::ENVELOPE, Category::Digital, (110, 215, 185), &["holder", "provider", "first_seen"], &["mail", "inbox"], ValueFormat::Email),
    k(EntityKind::Phone, "Phone", egui_phosphor::regular::PHONE, Category::Digital, (135, 205, 165), &["country", "carrier", "holder"], &["mobile", "telephone", "number", "cell"], ValueFormat::Free),
    k(EntityKind::SocialAccount, "Social account", egui_phosphor::regular::AT, Category::Digital, (80, 210, 170), &["platform", "handle", "holder", "created"], &["twitter", "facebook", "instagram", "telegram", "profile", "x"], ValueFormat::Free),
    k(EntityKind::Website, "Website", egui_phosphor::regular::BROWSER, Category::Digital, (145, 220, 140), &["url", "operator", "first_archived"], &["site", "url", "webpage"], ValueFormat::Url),
    k(EntityKind::Server, "Server", egui_phosphor::regular::HARD_DRIVES, Category::Digital, (70, 185, 185), &["ip", "hosting_provider", "location"], &["host", "vps", "infrastructure", "machine"], ValueFormat::Hostname),
    k(EntityKind::Device, "Device", egui_phosphor::regular::DEVICE_MOBILE, Category::Digital, (105, 220, 205), &["type", "imei", "owner"], &["laptop", "computer", "hardware", "gadget"], ValueFormat::Free),
    k(EntityKind::CryptoWallet, "Crypto wallet", egui_phosphor::regular::CURRENCY_BTC, Category::Digital, (60, 195, 155), &["chain", "address", "custodian"], &["bitcoin", "btc", "eth", "blockchain", "crypto"], ValueFormat::Free),
    k(EntityKind::BankAccount, "Bank account", egui_phosphor::regular::CREDIT_CARD, Category::Financial, (230, 205, 75), &["iban", "bank", "holder", "currency"], &["iban", "account", "swift"], ValueFormat::Free),
    k(EntityKind::Transaction, "Transaction", egui_phosphor::regular::ARROWS_LEFT_RIGHT, Category::Financial, (250, 215, 110), &["date", "amount", "currency", "from_account", "to_account"], &["transfer", "wire", "remittance"], ValueFormat::Free),
    k(EntityKind::Loan, "Loan", egui_phosphor::regular::HAND_COINS, Category::Financial, (215, 185, 90), &["amount", "currency", "lender", "borrower", "date", "terms"], &["credit", "debt", "mortgage"], ValueFormat::Free),
    k(EntityKind::Donation, "Donation", egui_phosphor::regular::GIFT, Category::Financial, (245, 225, 135), &["amount", "currency", "donor", "recipient", "date"], &["contribution", "funding", "grant"], ValueFormat::Free),
    k(EntityKind::Contract, "Contract", egui_phosphor::regular::SCROLL, Category::Financial, (235, 210, 120), &["parties", "value", "signed", "subject"], &["agreement", "deal", "mou"], ValueFormat::Free),
    k(EntityKind::Invoice, "Invoice", egui_phosphor::regular::RECEIPT, Category::Financial, (225, 200, 100), &["number", "amount", "currency", "issuer", "billed_to", "date"], &["bill"], ValueFormat::Free),
    k(EntityKind::RegistryRecord, "Registry record", egui_phosphor::regular::ARCHIVE, Category::Financial, (205, 185, 70), &["registry", "record_no", "filed", "subject"], &["filing", "registration", "incorporation record"], ValueFormat::Free),
    k(EntityKind::Money, "Money", egui_phosphor::regular::MONEY, Category::Financial, (240, 220, 90), &["amount", "currency"], &["cash", "funds", "sum"], ValueFormat::Free),
    k(EntityKind::Location, "Location", egui_phosphor::regular::MAP_PIN, Category::Physical, (170, 150, 110), &["country", "country_code", "city", "region", "coordinates"], &["place", "spot", "coordinates", "geo"], ValueFormat::Free),
    k(EntityKind::Address, "Address", egui_phosphor::regular::HOUSE_LINE, Category::Physical, (190, 165, 125), &["street", "city", "country", "postcode"], &["street", "residence", "domicile"], ValueFormat::Free),
    k(EntityKind::Property, "Property", egui_phosphor::regular::BUILDING_APARTMENT, Category::Physical, (160, 135, 95), &["address", "type", "value", "registered_owner"], &["real estate", "estate", "building", "land", "villa", "mansion"], ValueFormat::Free),
    k(EntityKind::Vehicle, "Vehicle", egui_phosphor::regular::CAR, Category::Physical, (200, 175, 135), &["plate", "make_model", "vin", "registered_owner"], &["car", "truck", "auto", "motorcycle"], ValueFormat::Free),
    k(EntityKind::Vessel, "Vessel", egui_phosphor::regular::BOAT, Category::Physical, (150, 140, 105), &["imo", "mmsi", "flag", "owner"], &["ship", "boat", "yacht"], ValueFormat::Free),
    k(EntityKind::Aircraft, "Aircraft", egui_phosphor::regular::AIRPLANE, Category::Physical, (205, 185, 150), &["registration", "type", "operator", "owner"], &["plane", "jet", "helicopter"], ValueFormat::Free),
    k(EntityKind::Shipment, "Shipment", egui_phosphor::regular::PACKAGE, Category::Physical, (180, 155, 115), &["bill_of_lading", "origin", "destination", "cargo", "date"], &["cargo", "container", "freight", "parcel"], ValueFormat::Free),
    k(EntityKind::Weapon, "Weapon", egui_phosphor::regular::CROSSHAIR, Category::Physical, (140, 115, 85), &["type", "serial_no", "origin"], &["gun", "firearm", "arms", "pistol", "rifle", "knife"], ValueFormat::Free),
    k(EntityKind::Asset, "Asset", egui_phosphor::regular::DIAMOND, Category::Physical, (210, 180, 140), &["type", "value", "currency", "owner"], &["valuables", "holding", "artwork", "jewelry"], ValueFormat::Free),
    k(EntityKind::Document, "Document", egui_phosphor::regular::FILE_TEXT, Category::Documents, (200, 200, 200), &["date", "author", "file"], &["file", "paper", "doc", "memo"], ValueFormat::Free),
    k(EntityKind::Passport, "Passport", egui_phosphor::regular::IDENTIFICATION_CARD, Category::Documents, (185, 190, 205), &["number", "country", "issued", "expires", "holder"], &["travel document", "visa", "id"], ValueFormat::Free),
    k(EntityKind::Leak, "Leak", egui_phosphor::regular::DROP, Category::Documents, (170, 190, 200), &["date", "size", "origin", "obtained_via"], &["dump", "breach", "disclosure"], ValueFormat::Free),
    k(EntityKind::Dataset, "Dataset", egui_phosphor::regular::DATABASE, Category::Documents, (175, 175, 185), &["source", "records", "format", "date"], &["data", "database", "records", "spreadsheet"], ValueFormat::Free),
    k(EntityKind::Photo, "Photo", egui_phosphor::regular::CAMERA, Category::Documents, (190, 190, 190), &["date", "place", "photographer", "file"], &["image", "picture", "photograph"], ValueFormat::Free),
    k(EntityKind::Video, "Video", egui_phosphor::regular::VIDEO_CAMERA, Category::Documents, (165, 165, 175), &["date", "place", "duration", "file"], &["footage", "recording", "clip"], ValueFormat::Free),
    k(EntityKind::Article, "Article", egui_phosphor::regular::NEWSPAPER, Category::Documents, (205, 205, 190), &["outlet", "author", "published", "url"], &["story", "report", "news piece"], ValueFormat::Free),
    k(EntityKind::CourtRecord, "Court record", egui_phosphor::regular::STAMP, Category::Documents, (180, 180, 165), &["case_no", "court", "date", "type"], &["case file", "docket", "ruling", "verdict", "indictment", "deposition"], ValueFormat::Free),
    k(EntityKind::Event, "Event", egui_phosphor::regular::CALENDAR, Category::Events, (255, 120, 120), &["date", "place"], &["incident", "happening", "occurrence"], ValueFormat::Free),
    k(EntityKind::Meeting, "Meeting", egui_phosphor::regular::USERS_THREE, Category::Events, (240, 145, 145), &["date", "place", "participants"], &["encounter", "appointment", "summit", "rendezvous"], ValueFormat::Free),
    k(EntityKind::Payment, "Payment", egui_phosphor::regular::CURRENCY_CIRCLE_DOLLAR, Category::Events, (255, 110, 140), &["date", "amount", "currency", "method"], &["payout", "fee", "bribe", "kickback"], ValueFormat::Free),
    k(EntityKind::Travel, "Travel", egui_phosphor::regular::SUITCASE_ROLLING, Category::Events, (230, 125, 105), &["date", "from", "to", "traveler", "purpose"], &["trip", "flight", "journey", "visit"], ValueFormat::Free),
    k(EntityKind::Other, "Other", egui_phosphor::regular::QUESTION, Category::Other, (170, 170, 170), &[], &["misc", "unknown", "custom"], ValueFormat::Free),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub slug: String,
    pub kind: EntityKind,
    pub label: String,

    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub observations: Vec<Observation>,
    pub note: String,
    pub image: Option<String>,
    #[serde(default = "default_node_size")]
    pub size: f32,


    #[serde(skip, default = "default_boost")]
    pub boost: f32,

    #[serde(skip)]
    pub collapsed: bool,

    #[serde(skip)]
    pub hidden: bool,
}

pub const DEFAULT_NODE_SIZE: f32 = 2.0;

fn default_node_size() -> f32 {
    DEFAULT_NODE_SIZE
}

fn default_boost() -> f32 {
    1.0
}

pub fn slugify(s: &str) -> Result<String, Error> {
    Ok(s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_"))
}


pub fn pretty_key(slug: &str) -> Result<String, Error> {
    Ok(slug
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|w| {
            let mut cs = w.chars();
            let Some(c) = cs.next() else {
                return String::new();
            };
            c.to_uppercase().collect::<String>() + cs.as_str()
        })
        .collect::<Vec<_>>()
        .join(" "))
}

impl Entity {


    pub fn attr_pairs(&self) -> Result<Vec<(String, String)>, Error> {
        let mut out = Vec::new();
        for o in &self.observations {
            out.push((o.name.clone(), o.value.text()?));
        }
        Ok(out)
    }


    pub fn module_value(&self) -> Result<Option<String>, Error> {
        let effective = if self.value.trim().is_empty() {
            self.label.trim()
        } else {
            self.value.trim()
        };
        if effective.is_empty() {
            return Ok(None);
        }
        if !validate_value(KIND_META[self.kind as usize].value_format, effective)? {
            return Ok(None);
        }
        Ok(Some(effective.to_owned()))
    }
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
    fn attr_pairs_flattens_observations_to_text() {
        use crate::model::{Author, ObsValue, Observation, Origin, Verification};
        let mut e = Entity {
            slug: "j_doe".into(),
            kind: EntityKind::Person,
            label: "J. Doe".into(),
            value: String::new(),
            observations: Vec::new(),
            note: String::new(),
            image: None,
            size: DEFAULT_NODE_SIZE,
            boost: 1.0,
            collapsed: false,
            hidden: false,
        };
        e.observations.push(
            Observation::stamp(
                "dob".into(),
                ObsValue::Text("1953-01-20".into()),
                Origin::Manual,
                Verification::AnalystAsserted,
                Author::default(),
            )
            .unwrap(),
        );
        e.observations.push(
            Observation::stamp(
                "weight_kg".into(),
                ObsValue::Number(82.0),
                Origin::Manual,
                Verification::AnalystAsserted,
                Author::default(),
            )
            .unwrap(),
        );
        let pairs = e.attr_pairs().unwrap();
        assert_eq!(pairs[0], ("dob".to_string(), "1953-01-20".to_string()));
        assert_eq!(pairs[1], ("weight_kg".to_string(), "82".to_string()));
    }

    #[test]
    fn value_validators_accept_and_reject() {
        assert!(validate_value(ValueFormat::Ip, "1.1.1.1").unwrap());
        assert!(validate_value(ValueFormat::Ip, "2606:4700::1111").unwrap());
        assert!(!validate_value(ValueFormat::Ip, "1.1.1.999").unwrap());
        assert!(!validate_value(ValueFormat::Ip, "evil.example").unwrap());

        assert!(validate_value(ValueFormat::Hostname, "evil.example").unwrap());
        assert!(validate_value(ValueFormat::Hostname, "NS1.GOOGLE.COM").unwrap());
        assert!(!validate_value(ValueFormat::Hostname, "1.1.1.1").unwrap());
        assert!(!validate_value(ValueFormat::Hostname, "-bad.example").unwrap());
        assert!(!validate_value(ValueFormat::Hostname, "two words.com").unwrap());
        assert!(!validate_value(ValueFormat::Hostname, "").unwrap());

        assert!(validate_value(ValueFormat::Email, "a@b.example").unwrap());
        assert!(!validate_value(ValueFormat::Email, "a@b@c.example").unwrap());
        assert!(!validate_value(ValueFormat::Email, "nope").unwrap());

        assert!(validate_value(ValueFormat::Url, "https://evil.example/x?y#z").unwrap());
        assert!(validate_value(ValueFormat::Url, "http://1.1.1.1:8080/p").unwrap());
        assert!(!validate_value(ValueFormat::Url, "ftp://evil.example").unwrap());
        assert!(!validate_value(ValueFormat::Url, "evil.example").unwrap());

        assert!(validate_value(ValueFormat::Free, "anything at all").unwrap());
    }

    #[test]
    fn module_value_prefers_value_falls_back_to_label() {
        let mut e = Entity {
            slug: "x".into(),
            kind: EntityKind::Domain,
            label: "the suspicious site evil.example".into(),
            value: String::new(),
            observations: Vec::new(),
            note: String::new(),
            image: None,
            size: DEFAULT_NODE_SIZE,
            boost: 1.0,
            collapsed: false,
            hidden: false,
        };
        assert_eq!(e.module_value().unwrap(), None, "prose label fails hostname check");
        e.value = "evil.example".into();
        assert_eq!(e.module_value().unwrap(), Some("evil.example".to_owned()));
        e.value = String::new();
        e.label = "evil.example".into();
        assert_eq!(e.module_value().unwrap(), Some("evil.example".to_owned()), "valid label adopted");
    }

    #[test]
    fn url_host_strips_scheme_path_port_and_www() {
        assert_eq!(url_host("https://github.com/torvalds").unwrap(), Some("github.com".to_owned()));
        assert_eq!(url_host("https://www.Reddit.com/user/x").unwrap(), Some("reddit.com".to_owned()));
        assert_eq!(url_host("http://host.example:8080/p?q#f").unwrap(), Some("host.example".to_owned()));
        assert_eq!(url_host("not a url").unwrap(), None);
        assert_eq!(url_host("ftp://x.example").unwrap(), None);
    }

    #[test]
    fn keys_round_trip_pretty_and_slug() {
        assert_eq!(pretty_key("first_name").unwrap(), "First Name");
        assert_eq!(pretty_key("swift_bic").unwrap(), "Swift Bic");
        assert_eq!(slugify("Shoe Size").unwrap(), "shoe_size");
        assert_eq!(slugify("  Weird--Key!! ").unwrap(), "weird_key");
        assert_eq!(slugify("IP address").unwrap(), "ip_address");
        assert_eq!(slugify(&pretty_key("date_of_birth").unwrap()).unwrap(), "date_of_birth");
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
