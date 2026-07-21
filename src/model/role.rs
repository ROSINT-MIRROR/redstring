use egui::Color32;
use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::entity::{Entity, EntityKind};
use super::facet::{Facet, occupation_of, set_occupation, set_role, set_subtype};
use super::observation::{Author, ObsValue, Observation, Origin, Verification};
use super::occupation::{Occupation, OccupationIcon};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Role {
    #[default]
    Unset,
    Suspect,
    Perpetrator,
    Victim,
    Witness,
    Source,
    Impersonated,
    Associate,
    Enabler,
    Broker,
    Facilitator,
    Patron,
    Frontman,
    Informant,
    Whistleblower,
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

pub const ROLE_META: [RoleMeta; 15] = [
    rm(Role::Unset, "none", egui_phosphor::regular::CIRCLE_DASHED, (150, 150, 150)),
    rm(Role::Suspect, "suspect", egui_phosphor::regular::USER_FOCUS, (235, 165, 60)),
    rm(Role::Perpetrator, "perpetrator", egui_phosphor::regular::HAND_FIST, (225, 70, 70)),
    rm(Role::Victim, "victim", egui_phosphor::regular::BANDAIDS, (95, 155, 240)),
    rm(Role::Witness, "witness", egui_phosphor::regular::EYE, (90, 195, 165)),
    rm(Role::Source, "source", egui_phosphor::regular::USER_SOUND, (185, 130, 225)),
    rm(Role::Impersonated, "impersonated", egui_phosphor::regular::MASK_HAPPY, (216, 138, 178)),
    rm(Role::Associate, "associate", egui_phosphor::regular::HANDSHAKE, (150, 170, 195)),
    rm(Role::Enabler, "enabler", egui_phosphor::regular::PLUGS_CONNECTED, (205, 165, 95)),
    rm(Role::Broker, "broker", egui_phosphor::regular::SWAP, (200, 140, 110)),
    rm(Role::Facilitator, "facilitator", egui_phosphor::regular::FLOW_ARROW, (150, 185, 130)),
    rm(Role::Patron, "patron", egui_phosphor::regular::CROWN, (215, 175, 90)),
    rm(Role::Frontman, "frontman", egui_phosphor::regular::SUNGLASSES, (190, 145, 200)),
    rm(Role::Informant, "informant", egui_phosphor::regular::EAR, (135, 175, 200)),
    rm(Role::Whistleblower, "whistleblower", egui_phosphor::regular::MEGAPHONE, (100, 195, 175)),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Preset {
    pub label: &'static str,
    pub icon: &'static str,
    pub base_kind: EntityKind,
    pub role: Option<Role>,
    pub occupation: Option<(&'static str, OccupationIcon)>,
    pub facet: Option<(Facet, u16)>,
    pub attr: Option<(&'static str, &'static str)>,
}

const fn ps(
    label: &'static str,
    icon: &'static str,
    base_kind: EntityKind,
    role: Option<Role>,
    occupation: Option<(&'static str, OccupationIcon)>,
    facet: Option<(Facet, u16)>,
    attr: Option<(&'static str, &'static str)>,
) -> Preset {
    Preset {
        label,
        icon,
        base_kind,
        role,
        occupation,
        facet,
        attr,
    }
}

#[rustfmt::skip]
pub const PRESETS: [Preset; 66] = [
    ps("Suspect", egui_phosphor::regular::USER_FOCUS, EntityKind::Person, Some(Role::Suspect), None, None, None),
    ps("Perpetrator", egui_phosphor::regular::HAND_FIST, EntityKind::Person, Some(Role::Perpetrator), None, None, None),
    ps("Victim", egui_phosphor::regular::BANDAIDS, EntityKind::Person, Some(Role::Victim), None, None, None),
    ps("Witness", egui_phosphor::regular::EYE, EntityKind::Person, Some(Role::Witness), None, None, None),
    ps("Source", egui_phosphor::regular::USER_SOUND, EntityKind::Person, Some(Role::Source), None, None, None),
    ps("Judge", egui_phosphor::regular::SCALES, EntityKind::Person, None, Some(("Judge", OccupationIcon::Scales)), None, None),
    ps("Journalist", egui_phosphor::regular::PEN_NIB, EntityKind::Person, None, Some(("Journalist", OccupationIcon::PenNib)), None, None),
    ps("Lawyer", egui_phosphor::regular::BRIEFCASE, EntityKind::Person, None, Some(("Lawyer", OccupationIcon::Briefcase)), None, None),
    ps("Police officer", egui_phosphor::regular::SHIELD_STAR, EntityKind::Person, None, Some(("Police officer", OccupationIcon::Shield)), None, None),
    ps("Politician", egui_phosphor::regular::MICROPHONE_STAGE, EntityKind::Person, None, Some(("Politician", OccupationIcon::Microphone)), None, None),
    ps("Company", egui_phosphor::regular::BUILDING_OFFICE, EntityKind::Organization, None, None, Some((Facet::OrgType, 0)), None),
    ps("Shell company", egui_phosphor::regular::CUBE_TRANSPARENT, EntityKind::Organization, None, None, Some((Facet::OrgType, 1)), None),
    ps("NGO", egui_phosphor::regular::HAND_HEART, EntityKind::Organization, None, None, Some((Facet::OrgType, 2)), None),
    ps("Government agency", egui_phosphor::regular::BANK, EntityKind::Organization, None, None, Some((Facet::OrgType, 3)), None),
    ps("Registrar", egui_phosphor::regular::IDENTIFICATION_CARD, EntityKind::Organization, None, None, Some((Facet::OrgType, 4)), None),
    ps("Hosting provider", egui_phosphor::regular::HARD_DRIVES, EntityKind::Organization, None, None, Some((Facet::OrgType, 5)), None),
    ps("CPA network", egui_phosphor::regular::HANDSHAKE, EntityKind::Organization, None, None, Some((Facet::OrgType, 6)), None),
    ps("Law firm", egui_phosphor::regular::BRIEFCASE, EntityKind::Organization, None, None, Some((Facet::OrgType, 7)), None),
    ps("Media outlet", egui_phosphor::regular::BROADCAST, EntityKind::Organization, None, None, Some((Facet::OrgType, 8)), None),
    ps("Political party", egui_phosphor::regular::FLAG_BANNER, EntityKind::Organization, None, None, Some((Facet::OrgType, 9)), None),
    ps("Criminal org", egui_phosphor::regular::SKULL, EntityKind::Organization, None, None, Some((Facet::OrgType, 10)), None),
    ps("Hospital", egui_phosphor::regular::HOSPITAL, EntityKind::Organization, None, None, Some((Facet::OrgType, 11)), None),
    ps("Clinic", egui_phosphor::regular::STETHOSCOPE, EntityKind::Organization, None, None, Some((Facet::OrgType, 12)), None),
    ps("Pharmacy", egui_phosphor::regular::PILL, EntityKind::Organization, None, None, Some((Facet::OrgType, 13)), None),
    ps("Esoteric order", egui_phosphor::regular::EYE, EntityKind::Organization, None, None, Some((Facet::OrgType, 14)), None),
    ps("Ad platform", egui_phosphor::regular::TARGET, EntityKind::Organization, None, None, Some((Facet::OrgType, 15)), None),
    ps("Burner", egui_phosphor::regular::FLAME, EntityKind::SocialAccount, None, None, Some((Facet::AccountType, 0)), None),
    ps("Sockpuppet", egui_phosphor::regular::MASK_HAPPY, EntityKind::SocialAccount, None, None, Some((Facet::AccountType, 1)), None),
    ps("Parked", egui_phosphor::regular::PARK, EntityKind::Domain, None, None, Some((Facet::DomainType, 0)), None),
    ps("Typosquat", egui_phosphor::regular::WARNING, EntityKind::Domain, None, None, Some((Facet::DomainType, 1)), None),
    ps("Drop-caught", egui_phosphor::regular::HAND_GRABBING, EntityKind::Domain, None, None, Some((Facet::DomainType, 2)), None),
    ps("A", egui_phosphor::regular::NETWORK, EntityKind::DnsRecord, None, None, None, Some(("record_type", "A"))),
    ps("AAAA", egui_phosphor::regular::NETWORK, EntityKind::DnsRecord, None, None, None, Some(("record_type", "AAAA"))),
    ps("MX", egui_phosphor::regular::ENVELOPE, EntityKind::DnsRecord, None, None, None, Some(("record_type", "MX"))),
    ps("TXT", egui_phosphor::regular::TEXT_T, EntityKind::DnsRecord, None, None, None, Some(("record_type", "TXT"))),
    ps("SPF", egui_phosphor::regular::SHIELD_CHECK, EntityKind::DnsRecord, None, None, None, Some(("record_type", "SPF"))),
    ps("NS", egui_phosphor::regular::HARD_DRIVES, EntityKind::DnsRecord, None, None, None, Some(("record_type", "NS"))),
    ps("CNAME", egui_phosphor::regular::ARROW_BEND_UP_RIGHT, EntityKind::DnsRecord, None, None, None, Some(("record_type", "CNAME"))),
    ps("Google Analytics", egui_phosphor::regular::GOOGLE_LOGO, EntityKind::TrackerId, None, None, None, Some(("vendor", "Google Analytics"))),
    ps("Google Tag Manager", egui_phosphor::regular::GOOGLE_LOGO, EntityKind::TrackerId, None, None, None, Some(("vendor", "Google Tag Manager"))),
    ps("Meta Pixel", egui_phosphor::regular::META_LOGO, EntityKind::TrackerId, None, None, None, Some(("vendor", "Meta"))),
    ps("Microsoft Clarity", egui_phosphor::regular::EYE, EntityKind::TrackerId, None, None, None, Some(("vendor", "Microsoft"))),
    ps("Yandex Metrica", egui_phosphor::regular::CHART_LINE, EntityKind::TrackerId, None, None, None, Some(("vendor", "Yandex"))),
    ps("WordPress", egui_phosphor::regular::ARTICLE, EntityKind::Technology, None, None, None, Some(("vendor", "WordPress"))),
    ps("WooCommerce", egui_phosphor::regular::SHOPPING_CART, EntityKind::Technology, None, None, None, Some(("vendor", "WooCommerce"))),
    ps("nginx", egui_phosphor::regular::HARD_DRIVES, EntityKind::Technology, None, None, None, Some(("vendor", "nginx"))),
    ps("Cloudflare", egui_phosphor::regular::CLOUD, EntityKind::Technology, None, None, None, Some(("vendor", "Cloudflare"))),
    ps("PHP", egui_phosphor::regular::FILE_CODE, EntityKind::Technology, None, None, None, Some(("vendor", "PHP"))),
    ps("React", egui_phosphor::regular::ATOM, EntityKind::Technology, None, None, None, Some(("vendor", "React"))),
    ps("String", egui_phosphor::regular::TEXT_T, EntityKind::Parameter, None, None, None, Some(("datatype", "String"))),
    ps("Integer", egui_phosphor::regular::HASH, EntityKind::Parameter, None, None, None, Some(("datatype", "Integer"))),
    ps("Email", egui_phosphor::regular::ENVELOPE, EntityKind::Parameter, None, None, None, Some(("datatype", "Email"))),
    ps("Phone", egui_phosphor::regular::PHONE, EntityKind::Parameter, None, None, None, Some(("datatype", "Phone"))),
    ps("File", egui_phosphor::regular::FILE, EntityKind::Parameter, None, None, None, Some(("datatype", "File"))),
    ps("JSON", egui_phosphor::regular::BRACKETS_CURLY, EntityKind::Parameter, None, None, None, Some(("datatype", "JSON"))),
    ps("Boolean", egui_phosphor::regular::TOGGLE_LEFT, EntityKind::Parameter, None, None, None, Some(("datatype", "Boolean"))),
    ps("Login", egui_phosphor::regular::SIGN_IN, EntityKind::Endpoint, None, None, None, Some(("purpose", "Login"))),
    ps("Checkout", egui_phosphor::regular::SHOPPING_CART, EntityKind::Endpoint, None, None, None, Some(("purpose", "Checkout"))),
    ps("Upload", egui_phosphor::regular::UPLOAD_SIMPLE, EntityKind::Endpoint, None, None, None, Some(("purpose", "Upload"))),
    ps("Search", egui_phosphor::regular::MAGNIFYING_GLASS, EntityKind::Endpoint, None, None, None, Some(("purpose", "Search"))),
    ps("API call", egui_phosphor::regular::WEBHOOKS_LOGO, EntityKind::Endpoint, None, None, None, Some(("purpose", "API call"))),
    ps("WooCommerce store", egui_phosphor::regular::SHOPPING_CART, EntityKind::Website, None, None, Some((Facet::SiteType, 0)), None),
    ps("Cloaker", egui_phosphor::regular::EYE_SLASH, EntityKind::Website, None, None, Some((Facet::SiteType, 1)), None),
    ps("Landing page", egui_phosphor::regular::ROCKET_LAUNCH, EntityKind::Website, None, None, Some((Facet::SiteType, 2)), None),
    ps("Storefront", egui_phosphor::regular::STOREFRONT, EntityKind::Website, None, None, Some((Facet::SiteType, 3)), None),
    ps("Blog", egui_phosphor::regular::ARTICLE, EntityKind::Website, None, None, Some((Facet::SiteType, 4)), None),
];

pub fn apply_preset(e: &mut Entity, preset: &Preset, author: &Author) -> Result<(), Error> {
    for r in preset.role.iter() {
        set_role(&mut e.facets, *r)?;
    }
    for occ in preset.occupation.iter() {
        if occupation_of(&e.facets)?.is_none() {
            set_occupation(
                &mut e.facets,
                Some(Occupation {
                    label: occ.0.to_owned(),
                    icon: occ.1,
                }),
            )?;
        }
    }
    for (facet, index) in preset.facet.iter() {
        set_subtype(&mut e.facets, *facet, Some(*index))?;
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
    use crate::model::{
        Facet, KIND_META, OCCUPATION_ICONS, role_of, subtype_of, subtype_value,
    };
    use crate::ui::testkit::nodes::person;

    #[test]
    fn role_meta_mirrors_declaration_order() {
        for (i, m) in ROLE_META.iter().enumerate() {
            assert_eq!(m.role as usize, i, "ROLE_META[{i}] is {:?}", m.role);
            assert!(!m.label.is_empty(), "{:?} label", m.role);
            assert!(!m.icon.is_empty(), "{:?} icon", m.role);
        }
    }

    #[test]
    fn impersonated_role_present_and_distinct_from_victim() {
        let m = &ROLE_META[Role::Impersonated as usize];
        assert_eq!(m.role, Role::Impersonated, "discriminant indexes its own ROLE_META row");
        assert_eq!(m.label, "impersonated");
        assert!(!m.icon.is_empty(), "impersonated needs a glyph");
        assert_ne!(Role::Impersonated, Role::Victim, "abused identities are not victims");
        assert_ne!(Role::Impersonated, Role::default(), "impersonated is not the default role");
    }

    #[test]
    fn wave2_network_roles_present_and_indexed() {
        for (role, label) in [
            (Role::Associate, "associate"),
            (Role::Enabler, "enabler"),
            (Role::Broker, "broker"),
            (Role::Facilitator, "facilitator"),
        ] {
            let m = &ROLE_META[role as usize];
            assert_eq!(m.role, role, "discriminant indexes its own ROLE_META row");
            assert_eq!(m.label, label);
            assert!(!m.icon.is_empty(), "{role:?} needs a glyph");
            assert_ne!(role, Role::default(), "{role:?} is a network role, not the default");
        }
    }

    #[test]
    fn round2_investigation_roles_present_and_indexed() {
        for (role, label) in [
            (Role::Patron, "patron"),
            (Role::Frontman, "frontman"),
            (Role::Informant, "informant"),
            (Role::Whistleblower, "whistleblower"),
        ] {
            let m = &ROLE_META[role as usize];
            assert_eq!(m.role, role, "discriminant indexes its own ROLE_META row");
            assert_eq!(m.label, label);
            assert!(!m.icon.is_empty(), "{role:?} needs a glyph");
            assert_ne!(role, Role::default(), "{role:?} is a real role, not the default");
        }
    }

    #[test]
    fn presets_have_valid_base_kind_and_a_facet() {
        assert!(!PRESETS.is_empty());
        for p in &PRESETS {
            assert!((p.base_kind as usize) < KIND_META.len(), "{} base_kind out of range", p.label);
            assert!(
                p.role.is_some() || p.occupation.is_some() || p.facet.is_some() || p.attr.is_some(),
                "{} sets no facet",
                p.label
            );
            if let Some(r) = p.role {
                assert!((r as usize) < ROLE_META.len(), "{} bad role index", p.label);
            }
            if let Some((facet, index)) = p.facet {
                assert!(
                    subtype_value(facet, index).unwrap().is_some(),
                    "{} facet index {index} out of range for {facet:?}",
                    p.label
                );
                assert_eq!(p.attr, None, "{} sets a subtype facet AND an attr", p.label);
            }
            if let Some((label, icon)) = p.occupation {
                assert!(!label.is_empty(), "{} occupation label empty", p.label);
                assert!((icon as usize) < OCCUPATION_ICONS.len(), "{} bad occupation icon", p.label);
                assert_eq!(
                    OCCUPATION_ICONS[icon as usize].glyph, p.icon,
                    "{} palette icon must equal its canonical occupation glyph",
                    p.label
                );
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
        ];
        for (label, base, key, value) in cases {
            let p = PRESETS.iter().find(|p| p.label == label).expect("preset present");
            assert_eq!(p.base_kind, base, "{label} base kind");
            assert_eq!(p.role, None, "{label} sets no role");
            assert_eq!(p.facet, None, "{label} is a record-classification, not a kind subtype");
            assert_eq!(p.attr, Some((key, value)), "{label} attr");
        }
    }

    #[test]
    fn subtype_presets_set_typed_facet_not_free_text() {
        let cases = [
            ("Company", EntityKind::Organization, Facet::OrgType, 0u16),
            ("Shell company", EntityKind::Organization, Facet::OrgType, 1),
            ("Registrar", EntityKind::Organization, Facet::OrgType, 4),
            ("Hosting provider", EntityKind::Organization, Facet::OrgType, 5),
            ("CPA network", EntityKind::Organization, Facet::OrgType, 6),
            ("Law firm", EntityKind::Organization, Facet::OrgType, 7),
            ("Criminal org", EntityKind::Organization, Facet::OrgType, 10),
            ("Burner", EntityKind::SocialAccount, Facet::AccountType, 0),
            ("Sockpuppet", EntityKind::SocialAccount, Facet::AccountType, 1),
            ("Parked", EntityKind::Domain, Facet::DomainType, 0),
            ("Typosquat", EntityKind::Domain, Facet::DomainType, 1),
            ("WooCommerce store", EntityKind::Website, Facet::SiteType, 0),
            ("Cloaker", EntityKind::Website, Facet::SiteType, 1),
            ("Blog", EntityKind::Website, Facet::SiteType, 4),
        ];
        for (label, base, facet, index) in cases {
            let p = PRESETS.iter().find(|p| p.label == label).expect("preset present");
            assert_eq!(p.base_kind, base, "{label} base kind");
            assert_eq!(p.role, None, "{label} sets no role");
            assert_eq!(p.attr, None, "{label} no longer stamps a free-text attr");
            assert_eq!(p.facet, Some((facet, index)), "{label} sets its typed subtype facet");
            let v = subtype_value(facet, index).unwrap().expect("vocab row present");
            assert_eq!(v.label, label, "{label} preset label matches its vocab value label");
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
    fn apply_preset_sets_occupation_facet_with_canonical_icon() {
        let mut e = person("x");
        let judge = PRESETS.iter().find(|p| p.label == "Judge").expect("judge preset");
        apply_preset(&mut e, judge, &Author::default()).unwrap();
        assert_eq!(role_of(&e.facets).unwrap(), Role::Unset);
        assert!(e.observations.is_empty(), "occupation is a facet — not a stray observation");
        let occ = occupation_of(&e.facets).unwrap().expect("occupation facet set");
        assert_eq!(occ.label, "Judge");
        assert_eq!(occ.icon, OccupationIcon::Scales);
        assert_eq!(
            OCCUPATION_ICONS[occ.icon as usize].glyph,
            egui_phosphor::regular::SCALES,
            "Judge must carry the Scales glyph"
        );
    }

    #[test]
    fn occupation_presets_carry_canonical_icons() {
        let expected = [
            ("Judge", OccupationIcon::Scales, egui_phosphor::regular::SCALES),
            ("Journalist", OccupationIcon::PenNib, egui_phosphor::regular::PEN_NIB),
            ("Lawyer", OccupationIcon::Briefcase, egui_phosphor::regular::BRIEFCASE),
            ("Police officer", OccupationIcon::Shield, egui_phosphor::regular::SHIELD_STAR),
            ("Politician", OccupationIcon::Microphone, egui_phosphor::regular::MICROPHONE_STAGE),
        ];
        for (label, icon, glyph) in expected {
            let p = PRESETS.iter().find(|p| p.label == label).expect("preset present");
            assert_eq!(p.role, None, "{label} sets no role");
            assert_eq!(p.attr, None, "{label} no longer stamps an occupation attr");
            assert_eq!(p.occupation, Some((label, icon)), "{label} occupation facet");
            assert_eq!(OCCUPATION_ICONS[icon as usize].glyph, glyph, "{label} glyph");
            assert_eq!(p.icon, glyph, "{label} palette icon equals its occupation glyph");
        }
    }

    #[test]
    fn apply_preset_sets_role_without_occupation_for_role_presets() {
        let mut e = person("y");
        let suspect = PRESETS
            .iter()
            .find(|p| p.role == Some(Role::Suspect))
            .expect("suspect preset");
        apply_preset(&mut e, suspect, &Author::default()).unwrap();
        assert_eq!(role_of(&e.facets).unwrap(), Role::Suspect);
        assert!(e.observations.is_empty(), "role preset must not add an attr");
    }

    #[test]
    fn apply_preset_sets_org_type_facet_for_org_presets() {
        let mut e = person("acme");
        e.kind = EntityKind::Organization;
        let company = PRESETS.iter().find(|p| p.label == "Company").expect("company preset");
        apply_preset(&mut e, company, &Author::default()).unwrap();
        assert_eq!(role_of(&e.facets).unwrap(), Role::Unset, "org preset sets no role");
        assert_eq!(
            subtype_of(&e.facets, Facet::OrgType).unwrap(),
            Some(0),
            "Company sets the OrgType facet at its vocab index"
        );
        assert!(
            !e.observations.iter().any(|o| o.name == "org_type"),
            "org type is a typed facet now — no stray org_type observation"
        );
    }

    #[test]
    fn apply_preset_shell_company_sets_org_type_facet_not_observation() {
        let mut e = person("acme");
        e.kind = EntityKind::Organization;
        let shell = PRESETS.iter().find(|p| p.label == "Shell company").expect("shell preset");
        apply_preset(&mut e, shell, &Author::default()).unwrap();
        assert_eq!(
            subtype_of(&e.facets, Facet::OrgType).unwrap(),
            Some(1),
            "Shell company sets Facet::OrgType at index 1, not a free-text org_type observation"
        );
        assert!(e.observations.is_empty(), "no org_type observation is stamped");
        let badge = crate::model::facet_badge(Facet::OrgType, &e.facets[&Facet::OrgType])
            .unwrap()
            .expect("shell company paints a subtype badge");
        assert_eq!(
            badge.glyph,
            egui_phosphor::regular::CUBE_TRANSPARENT,
            "shell company keeps its cube icon on the badge"
        );
    }

    #[test]
    fn apply_preset_never_clobbers_existing_occupation() {
        let mut e = person("z");
        set_occupation(
            &mut e.facets,
            Some(Occupation {
                label: "Existing".to_owned(),
                icon: OccupationIcon::Detective,
            }),
        )
        .unwrap();
        let lawyer = PRESETS.iter().find(|p| p.label == "Lawyer").expect("lawyer preset");
        apply_preset(&mut e, lawyer, &Author::default()).unwrap();
        let occ = occupation_of(&e.facets).unwrap().expect("occupation retained");
        assert_eq!(occ.label, "Existing", "preset must not overwrite an existing occupation");
        assert_eq!(occ.icon, OccupationIcon::Detective, "existing icon preserved");
        assert!(e.observations.is_empty(), "no occupation observation is created");
    }
}
