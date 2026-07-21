use egui::Color32;

use crate::error::Error;

use super::facet::Facet;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SubtypeValue {
    pub label: &'static str,
    pub icon: &'static str,
    pub color: Color32,
}

const fn sv(label: &'static str, icon: &'static str, rgb: (u8, u8, u8)) -> SubtypeValue {
    SubtypeValue {
        label,
        icon,
        color: Color32::from_rgb(rgb.0, rgb.1, rgb.2),
    }
}

#[rustfmt::skip]
pub const ORG_TYPE_VALUES: [SubtypeValue; 16] = [
    sv("Company", egui_phosphor::regular::BUILDING_OFFICE, (120, 160, 210)),
    sv("Shell company", egui_phosphor::regular::CUBE_TRANSPARENT, (215, 95, 95)),
    sv("NGO", egui_phosphor::regular::HAND_HEART, (110, 190, 140)),
    sv("Government agency", egui_phosphor::regular::BANK, (150, 140, 205)),
    sv("Registrar", egui_phosphor::regular::IDENTIFICATION_CARD, (210, 175, 95)),
    sv("Hosting provider", egui_phosphor::regular::HARD_DRIVES, (95, 180, 195)),
    sv("CPA network", egui_phosphor::regular::HANDSHAKE, (215, 150, 110)),
    sv("Law firm", egui_phosphor::regular::BRIEFCASE, (240, 190, 110)),
    sv("Media outlet", egui_phosphor::regular::BROADCAST, (255, 185, 125)),
    sv("Political party", egui_phosphor::regular::FLAG_BANNER, (255, 135, 85)),
    sv("Criminal org", egui_phosphor::regular::SKULL, (180, 105, 45)),
    sv("Hospital", egui_phosphor::regular::HOSPITAL, (225, 100, 110)),
    sv("Clinic", egui_phosphor::regular::STETHOSCOPE, (110, 190, 180)),
    sv("Pharmacy", egui_phosphor::regular::PILL, (120, 185, 120)),
    sv("Esoteric order", egui_phosphor::regular::EYE, (150, 110, 190)),
    sv("Ad platform", egui_phosphor::regular::TARGET, (235, 155, 90)),
];

#[rustfmt::skip]
pub const ACCOUNT_TYPE_VALUES: [SubtypeValue; 2] = [
    sv("Burner", egui_phosphor::regular::FLAME, (225, 120, 70)),
    sv("Sockpuppet", egui_phosphor::regular::MASK_HAPPY, (200, 130, 195)),
];

#[rustfmt::skip]
pub const DOMAIN_TYPE_VALUES: [SubtypeValue; 3] = [
    sv("Parked", egui_phosphor::regular::PARK, (130, 175, 120)),
    sv("Typosquat", egui_phosphor::regular::WARNING, (225, 165, 60)),
    sv("Drop-caught", egui_phosphor::regular::HAND_GRABBING, (200, 150, 90)),
];

#[rustfmt::skip]
pub const SITE_TYPE_VALUES: [SubtypeValue; 5] = [
    sv("WooCommerce store", egui_phosphor::regular::SHOPPING_CART, (150, 100, 185)),
    sv("Cloaker", egui_phosphor::regular::EYE_SLASH, (120, 120, 130)),
    sv("Landing page", egui_phosphor::regular::ROCKET_LAUNCH, (95, 170, 230)),
    sv("Storefront", egui_phosphor::regular::STOREFRONT, (224, 120, 160)),
    sv("Blog", egui_phosphor::regular::ARTICLE, (200, 150, 90)),
];

#[rustfmt::skip]
pub const CLAIM_TYPE_VALUES: [SubtypeValue; 6] = [
    sv("Fact", egui_phosphor::regular::SEAL_CHECK, (110, 190, 140)),
    sv("Documentary", egui_phosphor::regular::FILES, (120, 160, 210)),
    sv("Historical", egui_phosphor::regular::HOURGLASS, (210, 175, 95)),
    sv("Traditional", egui_phosphor::regular::BOOK_OPEN, (200, 150, 110)),
    sv("Interpretation", egui_phosphor::regular::MAGNIFYING_GLASS, (175, 140, 215)),
    sv("Thesis", egui_phosphor::regular::LIGHTBULB, (110, 195, 195)),
];

#[derive(Clone, Copy)]
pub struct SubtypeVocab {
    pub facet: Facet,
    pub values: &'static [SubtypeValue],
}

pub const SUBTYPE_VOCABS: [SubtypeVocab; 5] = [
    SubtypeVocab { facet: Facet::OrgType, values: &ORG_TYPE_VALUES },
    SubtypeVocab { facet: Facet::AccountType, values: &ACCOUNT_TYPE_VALUES },
    SubtypeVocab { facet: Facet::DomainType, values: &DOMAIN_TYPE_VALUES },
    SubtypeVocab { facet: Facet::SiteType, values: &SITE_TYPE_VALUES },
    SubtypeVocab { facet: Facet::ClaimType, values: &CLAIM_TYPE_VALUES },
];

pub fn subtype_vocab(facet: Facet) -> Result<Option<SubtypeVocab>, Error> {
    Ok(SUBTYPE_VOCABS.iter().find(|v| v.facet == facet).copied())
}

pub fn subtype_value(facet: Facet, index: u16) -> Result<Option<SubtypeValue>, Error> {
    let Some(vocab) = subtype_vocab(facet)? else {
        return Ok(None);
    };
    Ok(vocab.values.get(index as usize).copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_glyphs_distinct(values: &[SubtypeValue]) {
        for (i, a) in values.iter().enumerate() {
            assert!(!a.label.is_empty(), "value label empty");
            let mut chars = a.icon.chars();
            let c = chars.next().expect("glyph has at least one char");
            assert!(chars.next().is_none(), "{} glyph must be a single codepoint", a.label);
            assert!(c as u32 >= 0xE000, "{} glyph must be a phosphor PUA codepoint", a.label);
            assert!(a.color.a() > 0, "{} needs a visible color", a.label);
            for b in values.iter().skip(i + 1) {
                assert_ne!(a.icon, b.icon, "duplicate glyph {} / {}", a.label, b.label);
            }
        }
    }

    #[test]
    fn org_type_vocab_is_pinned() {
        let labels: Vec<&str> = ORG_TYPE_VALUES.iter().map(|v| v.label).collect();
        assert_eq!(
            labels,
            vec![
                "Company",
                "Shell company",
                "NGO",
                "Government agency",
                "Registrar",
                "Hosting provider",
                "CPA network",
                "Law firm",
                "Media outlet",
                "Political party",
                "Criminal org",
                "Hospital",
                "Clinic",
                "Pharmacy",
                "Esoteric order",
                "Ad platform",
            ],
            "org-type vocab order pins the preset indices"
        );
        assert_glyphs_distinct(&ORG_TYPE_VALUES);
    }

    #[test]
    fn account_type_vocab_is_pinned() {
        let labels: Vec<&str> = ACCOUNT_TYPE_VALUES.iter().map(|v| v.label).collect();
        assert_eq!(labels, vec!["Burner", "Sockpuppet"]);
        assert_eq!(ACCOUNT_TYPE_VALUES[0].icon, egui_phosphor::regular::FLAME);
        assert_eq!(ACCOUNT_TYPE_VALUES[1].icon, egui_phosphor::regular::MASK_HAPPY);
        assert_glyphs_distinct(&ACCOUNT_TYPE_VALUES);
    }

    #[test]
    fn domain_type_vocab_is_pinned() {
        let labels: Vec<&str> = DOMAIN_TYPE_VALUES.iter().map(|v| v.label).collect();
        assert_eq!(labels, vec!["Parked", "Typosquat", "Drop-caught"]);
        assert_eq!(DOMAIN_TYPE_VALUES[0].icon, egui_phosphor::regular::PARK);
        assert_eq!(DOMAIN_TYPE_VALUES[1].icon, egui_phosphor::regular::WARNING);
        assert_eq!(DOMAIN_TYPE_VALUES[2].icon, egui_phosphor::regular::HAND_GRABBING);
        assert_glyphs_distinct(&DOMAIN_TYPE_VALUES);
    }

    #[test]
    fn site_type_vocab_is_pinned() {
        let labels: Vec<&str> = SITE_TYPE_VALUES.iter().map(|v| v.label).collect();
        assert_eq!(
            labels,
            vec!["WooCommerce store", "Cloaker", "Landing page", "Storefront", "Blog"]
        );
        assert_glyphs_distinct(&SITE_TYPE_VALUES);
    }

    #[test]
    fn subtype_vocabs_mirror_facet_order() {
        let facets: Vec<Facet> = SUBTYPE_VOCABS.iter().map(|v| v.facet).collect();
        assert_eq!(
            facets,
            vec![
                Facet::OrgType,
                Facet::AccountType,
                Facet::DomainType,
                Facet::SiteType,
                Facet::ClaimType,
            ],
            "SUBTYPE_VOCABS order mirrors the subtype Facet declaration order"
        );
        for v in &SUBTYPE_VOCABS {
            assert!(!v.values.is_empty(), "{:?} vocab is empty", v.facet);
        }
    }

    #[test]
    fn claim_type_vocab_is_pinned() {
        let labels: Vec<&str> = CLAIM_TYPE_VALUES.iter().map(|v| v.label).collect();
        assert_eq!(
            labels,
            vec!["Fact", "Documentary", "Historical", "Traditional", "Interpretation", "Thesis"],
            "claim-type epistemic taxonomy order is pinned"
        );
        assert_glyphs_distinct(&CLAIM_TYPE_VALUES);
        let vocab = subtype_vocab(Facet::ClaimType).unwrap().expect("claim-type vocab");
        assert_eq!(vocab.values.len(), 6);
        assert_eq!(subtype_value(Facet::ClaimType, 5).unwrap().expect("thesis row").label, "Thesis");
    }

    #[test]
    fn lookup_resolves_and_bounds_check() {
        let vocab = subtype_vocab(Facet::OrgType).unwrap().expect("org-type vocab");
        assert_eq!(vocab.values.len(), 16);
        let shell = subtype_value(Facet::OrgType, 1).unwrap().expect("index 1");
        assert_eq!(shell.label, "Shell company");
        let crim = subtype_value(Facet::OrgType, 10).unwrap().expect("index 10");
        assert_eq!(crim.label, "Criminal org");
        assert_eq!(crim.icon, egui_phosphor::regular::SKULL);
        let pharmacy = subtype_value(Facet::OrgType, 13).unwrap().expect("index 13");
        assert_eq!(pharmacy.label, "Pharmacy");
        assert!(subtype_value(Facet::OrgType, 99).unwrap().is_none(), "out-of-range index");
        assert!(subtype_vocab(Facet::Role).unwrap().is_none(), "Role is not a subtype facet");
        assert!(
            subtype_value(Facet::Occupation, 0).unwrap().is_none(),
            "Occupation is a freeform facet, not an indexed subtype"
        );
    }
}
