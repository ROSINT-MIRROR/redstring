use egui::Color32;
use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::entity::{Entity, EntityKind};
use super::observation::{Author, ObsValue, Observation, Origin, Verification};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Role {
    #[default]
    Unset,
    Suspect,
    Perpetrator,
    Victim,
    Witness,
    Source,
}

pub struct RoleMeta {
    pub role: Role,
    pub label: &'static str,
    pub icon: &'static str,
    pub color: Color32,
}

const fn rm(role: Role, label: &'static str, icon: &'static str, rgb: (u8, u8, u8)) -> RoleMeta {
    RoleMeta {
        role,
        label,
        icon,
        color: Color32::from_rgb(rgb.0, rgb.1, rgb.2),
    }
}

pub const ROLE_META: [RoleMeta; 6] = [
    rm(Role::Unset, "none", egui_phosphor::regular::CIRCLE_DASHED, (150, 150, 150)),
    rm(Role::Suspect, "suspect", egui_phosphor::regular::USER_FOCUS, (235, 165, 60)),
    rm(Role::Perpetrator, "perpetrator", egui_phosphor::regular::HAND_FIST, (225, 70, 70)),
    rm(Role::Victim, "victim", egui_phosphor::regular::BANDAIDS, (95, 155, 240)),
    rm(Role::Witness, "witness", egui_phosphor::regular::EYE, (90, 195, 165)),
    rm(Role::Source, "source", egui_phosphor::regular::USER_SOUND, (185, 130, 225)),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Preset {
    pub label: &'static str,
    pub icon: &'static str,
    pub base_kind: EntityKind,
    pub role: Option<Role>,
    pub attr: Option<(&'static str, &'static str)>,
}

const fn ps(
    label: &'static str,
    icon: &'static str,
    base_kind: EntityKind,
    role: Option<Role>,
    attr: Option<(&'static str, &'static str)>,
) -> Preset {
    Preset {
        label,
        icon,
        base_kind,
        role,
        attr,
    }
}

#[rustfmt::skip]
pub const PRESETS: [Preset; 56] = [
    ps("Suspect", egui_phosphor::regular::USER_FOCUS, EntityKind::Person, Some(Role::Suspect), None),
    ps("Perpetrator", egui_phosphor::regular::HAND_FIST, EntityKind::Person, Some(Role::Perpetrator), None),
    ps("Victim", egui_phosphor::regular::BANDAIDS, EntityKind::Person, Some(Role::Victim), None),
    ps("Witness", egui_phosphor::regular::EYE, EntityKind::Person, Some(Role::Witness), None),
    ps("Source", egui_phosphor::regular::USER_SOUND, EntityKind::Person, Some(Role::Source), None),
    ps("Judge", egui_phosphor::regular::SCALES, EntityKind::Person, None, Some(("occupation", "Judge"))),
    ps("Journalist", egui_phosphor::regular::PEN_NIB, EntityKind::Person, None, Some(("occupation", "Journalist"))),
    ps("Lawyer", egui_phosphor::regular::BRIEFCASE, EntityKind::Person, None, Some(("occupation", "Lawyer"))),
    ps("Police officer", egui_phosphor::regular::SHIELD_STAR, EntityKind::Person, None, Some(("occupation", "Police officer"))),
    ps("Politician", egui_phosphor::regular::MICROPHONE_STAGE, EntityKind::Person, None, Some(("occupation", "Politician"))),
    ps("Company", egui_phosphor::regular::BUILDING_OFFICE, EntityKind::Organization, None, Some(("org_type", "Company"))),
    ps("Shell company", egui_phosphor::regular::CUBE_TRANSPARENT, EntityKind::Organization, None, Some(("org_type", "Shell company"))),
    ps("NGO", egui_phosphor::regular::HAND_HEART, EntityKind::Organization, None, Some(("org_type", "NGO"))),
    ps("Government agency", egui_phosphor::regular::BANK, EntityKind::Organization, None, Some(("org_type", "Government agency"))),
    ps("Registrar", egui_phosphor::regular::IDENTIFICATION_CARD, EntityKind::Organization, None, Some(("org_type", "registrar"))),
    ps("Hosting provider", egui_phosphor::regular::HARD_DRIVES, EntityKind::Organization, None, Some(("org_type", "hosting"))),
    ps("CPA network", egui_phosphor::regular::HANDSHAKE, EntityKind::Organization, None, Some(("org_type", "cpa_network"))),
    ps("Burner", egui_phosphor::regular::FLAME, EntityKind::SocialAccount, None, Some(("account_type", "Burner"))),
    ps("Sockpuppet", egui_phosphor::regular::MASK_HAPPY, EntityKind::SocialAccount, None, Some(("account_type", "Sockpuppet"))),
    ps("Parked", egui_phosphor::regular::PARK, EntityKind::Domain, None, Some(("domain_type", "Parked"))),
    ps("Typosquat", egui_phosphor::regular::WARNING, EntityKind::Domain, None, Some(("domain_type", "Typosquat"))),
    ps("A", egui_phosphor::regular::NETWORK, EntityKind::DnsRecord, None, Some(("record_type", "A"))),
    ps("AAAA", egui_phosphor::regular::NETWORK, EntityKind::DnsRecord, None, Some(("record_type", "AAAA"))),
    ps("MX", egui_phosphor::regular::ENVELOPE, EntityKind::DnsRecord, None, Some(("record_type", "MX"))),
    ps("TXT", egui_phosphor::regular::TEXT_T, EntityKind::DnsRecord, None, Some(("record_type", "TXT"))),
    ps("SPF", egui_phosphor::regular::SHIELD_CHECK, EntityKind::DnsRecord, None, Some(("record_type", "SPF"))),
    ps("NS", egui_phosphor::regular::HARD_DRIVES, EntityKind::DnsRecord, None, Some(("record_type", "NS"))),
    ps("CNAME", egui_phosphor::regular::ARROW_BEND_UP_RIGHT, EntityKind::DnsRecord, None, Some(("record_type", "CNAME"))),
    ps("Google Analytics", egui_phosphor::regular::GOOGLE_LOGO, EntityKind::TrackerId, None, Some(("vendor", "Google Analytics"))),
    ps("Google Tag Manager", egui_phosphor::regular::GOOGLE_LOGO, EntityKind::TrackerId, None, Some(("vendor", "Google Tag Manager"))),
    ps("Meta Pixel", egui_phosphor::regular::META_LOGO, EntityKind::TrackerId, None, Some(("vendor", "Meta"))),
    ps("Microsoft Clarity", egui_phosphor::regular::EYE, EntityKind::TrackerId, None, Some(("vendor", "Microsoft"))),
    ps("Yandex Metrica", egui_phosphor::regular::CHART_LINE, EntityKind::TrackerId, None, Some(("vendor", "Yandex"))),
    ps("WordPress", egui_phosphor::regular::ARTICLE, EntityKind::Technology, None, Some(("vendor", "WordPress"))),
    ps("WooCommerce", egui_phosphor::regular::SHOPPING_CART, EntityKind::Technology, None, Some(("vendor", "WooCommerce"))),
    ps("nginx", egui_phosphor::regular::HARD_DRIVES, EntityKind::Technology, None, Some(("vendor", "nginx"))),
    ps("Cloudflare", egui_phosphor::regular::CLOUD, EntityKind::Technology, None, Some(("vendor", "Cloudflare"))),
    ps("PHP", egui_phosphor::regular::FILE_CODE, EntityKind::Technology, None, Some(("vendor", "PHP"))),
    ps("React", egui_phosphor::regular::ATOM, EntityKind::Technology, None, Some(("vendor", "React"))),
    ps("String", egui_phosphor::regular::TEXT_T, EntityKind::Parameter, None, Some(("datatype", "String"))),
    ps("Integer", egui_phosphor::regular::HASH, EntityKind::Parameter, None, Some(("datatype", "Integer"))),
    ps("Email", egui_phosphor::regular::ENVELOPE, EntityKind::Parameter, None, Some(("datatype", "Email"))),
    ps("Phone", egui_phosphor::regular::PHONE, EntityKind::Parameter, None, Some(("datatype", "Phone"))),
    ps("File", egui_phosphor::regular::FILE, EntityKind::Parameter, None, Some(("datatype", "File"))),
    ps("JSON", egui_phosphor::regular::BRACKETS_CURLY, EntityKind::Parameter, None, Some(("datatype", "JSON"))),
    ps("Boolean", egui_phosphor::regular::TOGGLE_LEFT, EntityKind::Parameter, None, Some(("datatype", "Boolean"))),
    ps("Login", egui_phosphor::regular::SIGN_IN, EntityKind::Endpoint, None, Some(("purpose", "Login"))),
    ps("Checkout", egui_phosphor::regular::SHOPPING_CART, EntityKind::Endpoint, None, Some(("purpose", "Checkout"))),
    ps("Upload", egui_phosphor::regular::UPLOAD_SIMPLE, EntityKind::Endpoint, None, Some(("purpose", "Upload"))),
    ps("Search", egui_phosphor::regular::MAGNIFYING_GLASS, EntityKind::Endpoint, None, Some(("purpose", "Search"))),
    ps("API call", egui_phosphor::regular::WEBHOOKS_LOGO, EntityKind::Endpoint, None, Some(("purpose", "API call"))),
    ps("WooCommerce store", egui_phosphor::regular::SHOPPING_CART, EntityKind::Website, None, Some(("site_type", "WooCommerce store"))),
    ps("Cloaker", egui_phosphor::regular::EYE_SLASH, EntityKind::Website, None, Some(("site_type", "Cloaker"))),
    ps("Landing page", egui_phosphor::regular::ROCKET_LAUNCH, EntityKind::Website, None, Some(("site_type", "Landing page"))),
    ps("Storefront", egui_phosphor::regular::STOREFRONT, EntityKind::Website, None, Some(("site_type", "Storefront"))),
    ps("Blog", egui_phosphor::regular::ARTICLE, EntityKind::Website, None, Some(("site_type", "Blog"))),
];

pub fn apply_preset(e: &mut Entity, preset: &Preset, author: &Author) -> Result<(), Error> {
    for r in preset.role.iter() {
        e.role = *r;
    }
    let Some((key, value)) = preset.attr else {
        return Ok(());
    };
    if e.observations.iter().any(|o| o.name == key) {
        return Ok(());
    }
    e.observations.push(Observation::stamp(
        key.to_owned(),
        ObsValue::Text(value.to_owned()),
        Origin::Manual,
        Verification::AnalystAsserted,
        author.clone(),
    )?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DEFAULT_NODE_SIZE, KIND_META};

    fn person(slug: &str) -> Entity {
        Entity {
            slug: slug.to_owned(),
            kind: EntityKind::Person,
            label: slug.to_owned(),
            value: String::new(),
            observations: Vec::new(),
            role: Role::Unset,
            note: String::new(),
            image: None,
            size: DEFAULT_NODE_SIZE,
            boost: 1.0,
            collapsed: false,
            hidden: false,
        }
    }

    #[test]
    fn role_meta_mirrors_declaration_order() {
        for (i, m) in ROLE_META.iter().enumerate() {
            assert_eq!(m.role as usize, i, "ROLE_META[{i}] is {:?}", m.role);
            assert!(!m.label.is_empty(), "{:?} label", m.role);
            assert!(!m.icon.is_empty(), "{:?} icon", m.role);
        }
    }

    #[test]
    fn presets_have_valid_base_kind_and_a_facet() {
        assert!(!PRESETS.is_empty());
        for p in &PRESETS {
            assert!((p.base_kind as usize) < KIND_META.len(), "{} base_kind out of range", p.label);
            assert!(
                p.role.is_some() || p.attr.is_some(),
                "{} sets no facet",
                p.label
            );
            if let Some(r) = p.role {
                assert!((r as usize) < ROLE_META.len(), "{} bad role index", p.label);
            }
            if let Some((key, value)) = p.attr {
                assert!(!key.is_empty(), "{} attr key empty", p.label);
                assert!(!value.is_empty(), "{} attr value empty", p.label);
            }
            assert!(!p.icon.is_empty(), "{} icon", p.label);
            assert!(!p.label.is_empty(), "{} label", p.label);
        }
    }

    #[test]
    fn web_tech_presets_stamp_expected_attrs() {
        let cases = [
            ("A", EntityKind::DnsRecord, "record_type", "A"),
            ("CNAME", EntityKind::DnsRecord, "record_type", "CNAME"),
            ("Google Analytics", EntityKind::TrackerId, "vendor", "Google Analytics"),
            ("WordPress", EntityKind::Technology, "vendor", "WordPress"),
            ("Boolean", EntityKind::Parameter, "datatype", "Boolean"),
            ("Checkout", EntityKind::Endpoint, "purpose", "Checkout"),
            ("Cloaker", EntityKind::Website, "site_type", "Cloaker"),
            ("Registrar", EntityKind::Organization, "org_type", "registrar"),
            ("Hosting provider", EntityKind::Organization, "org_type", "hosting"),
            ("CPA network", EntityKind::Organization, "org_type", "cpa_network"),
        ];
        for (label, base, key, value) in cases {
            let p = PRESETS.iter().find(|p| p.label == label).expect("preset present");
            assert_eq!(p.base_kind, base, "{label} base kind");
            assert_eq!(p.role, None, "{label} sets no role");
            assert_eq!(p.attr, Some((key, value)), "{label} attr");
        }
    }

    #[test]
    fn presets_group_by_base_kind_contiguously() {
        let mut seen: Vec<EntityKind> = Vec::new();
        for p in &PRESETS {
            if seen.last() != Some(&p.base_kind) {
                assert!(!seen.contains(&p.base_kind), "{} breaks base-kind grouping", p.label);
                seen.push(p.base_kind);
            }
        }
        assert_eq!(
            seen,
            vec![
                EntityKind::Person,
                EntityKind::Organization,
                EntityKind::SocialAccount,
                EntityKind::Domain,
                EntityKind::DnsRecord,
                EntityKind::TrackerId,
                EntityKind::Technology,
                EntityKind::Parameter,
                EntityKind::Endpoint,
                EntityKind::Website,
            ],
            "preset base-kind order pinned"
        );
    }

    #[test]
    fn apply_preset_sets_occupation_for_occupation_presets() {
        let mut e = person("x");
        let judge = PRESETS.iter().find(|p| p.label == "Judge").expect("judge preset");
        apply_preset(&mut e, judge, &Author::default()).unwrap();
        assert_eq!(e.role, Role::Unset);
        assert!(
            e.observations
                .iter()
                .any(|o| o.name == "occupation"
                    && matches!(&o.value, ObsValue::Text(t) if t == "Judge")),
            "occupation observation missing"
        );
    }

    #[test]
    fn apply_preset_sets_role_without_occupation_for_role_presets() {
        let mut e = person("y");
        let suspect = PRESETS
            .iter()
            .find(|p| p.role == Some(Role::Suspect))
            .expect("suspect preset");
        apply_preset(&mut e, suspect, &Author::default()).unwrap();
        assert_eq!(e.role, Role::Suspect);
        assert!(e.observations.is_empty(), "role preset must not add an attr");
    }

    #[test]
    fn apply_preset_sets_org_type_for_org_presets() {
        let mut e = person("acme");
        e.kind = EntityKind::Organization;
        let company = PRESETS.iter().find(|p| p.label == "Company").expect("company preset");
        apply_preset(&mut e, company, &Author::default()).unwrap();
        assert_eq!(e.role, Role::Unset, "org preset sets no role");
        assert!(
            e.observations
                .iter()
                .any(|o| o.name == "org_type"
                    && matches!(&o.value, ObsValue::Text(t) if t == "Company")),
            "org_type observation missing"
        );
    }

    #[test]
    fn apply_preset_never_clobbers_existing_occupation() {
        let mut e = person("z");
        e.observations.push(
            Observation::stamp(
                "occupation".to_owned(),
                ObsValue::Text("Existing".to_owned()),
                Origin::Manual,
                Verification::AnalystAsserted,
                Author::default(),
            )
            .unwrap(),
        );
        let lawyer = PRESETS.iter().find(|p| p.label == "Lawyer").expect("lawyer preset");
        apply_preset(&mut e, lawyer, &Author::default()).unwrap();
        let occ: Vec<&Observation> = e.observations.iter().filter(|o| o.name == "occupation").collect();
        assert_eq!(occ.len(), 1, "must not duplicate occupation");
        assert!(matches!(&occ[0].value, ObsValue::Text(t) if t == "Existing"));
    }
}
