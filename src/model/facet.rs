use std::collections::BTreeMap;

use egui::Color32;
use serde::{Deserialize, Serialize};

use crate::error::Error;

use super::entity::{EntityKind, KIND_META};
use super::occupation::{OCC_BADGE, OCCUPATION_ICONS, Occupation};
use super::role::{ROLE_META, Role};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Facet {
    Role,
    Occupation,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum FacetValue {
    Indexed(u16),
    Freeform(Occupation),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BadgeCorner {
    TopRight,
    BottomLeft,
}

pub struct FacetBadge {
    pub color: Color32,
    pub glyph: &'static str,
}

pub struct FacetMeta {
    pub facet: Facet,
    pub label: &'static str,
    pub hint: &'static str,
    pub corner: BadgeCorner,
}

pub const FACET_META: [FacetMeta; 2] = [
    FacetMeta {
        facet: Facet::Role,
        label: "role",
        hint: "case-role — painted as a colored badge at the node's top-right",
        corner: BadgeCorner::TopRight,
    },
    FacetMeta {
        facet: Facet::Occupation,
        label: "occupation",
        hint: "job / profession — a steel badge at the node's bottom-left; pick its icon on the right",
        corner: BadgeCorner::BottomLeft,
    },
];

pub fn facet_badge(facet: Facet, value: &FacetValue) -> Result<Option<FacetBadge>, Error> {
    match (facet, value) {
        (Facet::Role, FacetValue::Indexed(i)) => {
            let idx = *i as usize;
            if idx == 0 || idx >= ROLE_META.len() {
                return Ok(None);
            }
            let m = &ROLE_META[idx];
            Ok(Some(FacetBadge {
                color: m.color,
                glyph: m.icon,
            }))
        }
        (Facet::Occupation, FacetValue::Freeform(occ)) => Ok(Some(FacetBadge {
            color: OCC_BADGE,
            glyph: OCCUPATION_ICONS[occ.icon as usize].glyph,
        })),
        (Facet::Role, FacetValue::Freeform(_)) => Ok(None),
        (Facet::Occupation, FacetValue::Indexed(_)) => Ok(None),
    }
}

pub fn role_of(facets: &BTreeMap<Facet, FacetValue>) -> Result<Role, Error> {
    let Some(FacetValue::Indexed(i)) = facets.get(&Facet::Role) else {
        return Ok(Role::Unset);
    };
    Ok(role_from_index(*i))
}

pub fn set_role(facets: &mut BTreeMap<Facet, FacetValue>, role: Role) -> Result<(), Error> {
    if role == Role::Unset {
        facets.remove(&Facet::Role);
        return Ok(());
    }
    facets.insert(Facet::Role, FacetValue::Indexed(role as u16));
    Ok(())
}

pub fn occupation_of(facets: &BTreeMap<Facet, FacetValue>) -> Result<Option<Occupation>, Error> {
    let Some(FacetValue::Freeform(occ)) = facets.get(&Facet::Occupation) else {
        return Ok(None);
    };
    Ok(Some(occ.clone()))
}

pub fn set_occupation(
    facets: &mut BTreeMap<Facet, FacetValue>,
    occupation: Option<Occupation>,
) -> Result<(), Error> {
    let Some(occ) = occupation else {
        facets.remove(&Facet::Occupation);
        return Ok(());
    };
    facets.insert(Facet::Occupation, FacetValue::Freeform(occ));
    Ok(())
}

pub fn retain_declared_facets(
    facets: &mut BTreeMap<Facet, FacetValue>,
    kind: EntityKind,
) -> Result<(), Error> {
    let declared = KIND_META[kind as usize].facets;
    facets.retain(|facet, _slot| declared.contains(facet));
    Ok(())
}

fn role_from_index(i: u16) -> Role {
    let Some(m) = ROLE_META.get(i as usize) else {
        return Role::Unset;
    };
    m.role
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::OccupationIcon;

    #[test]
    fn facet_meta_mirrors_declaration_order() {
        for (i, m) in FACET_META.iter().enumerate() {
            assert_eq!(m.facet as usize, i, "FACET_META[{i}] is {:?}", m.facet);
            assert!(!m.label.is_empty(), "{:?} label", m.facet);
            assert!(!m.hint.is_empty(), "{:?} hint", m.facet);
        }
    }

    #[test]
    fn role_facet_round_trips_through_the_store() {
        let mut facets = BTreeMap::new();
        assert_eq!(role_of(&facets).unwrap(), Role::Unset, "empty store has no role");
        set_role(&mut facets, Role::Perpetrator).unwrap();
        assert_eq!(role_of(&facets).unwrap(), Role::Perpetrator);
        assert_eq!(facets.get(&Facet::Role), Some(&FacetValue::Indexed(Role::Perpetrator as u16)));
        set_role(&mut facets, Role::Unset).unwrap();
        assert!(!facets.contains_key(&Facet::Role), "clearing to Unset removes the entry");
    }

    #[test]
    fn occupation_facet_round_trips_through_the_store() {
        let mut facets = BTreeMap::new();
        assert!(occupation_of(&facets).unwrap().is_none());
        let occ = Occupation {
            label: "Judge".to_owned(),
            icon: OccupationIcon::Scales,
        };
        set_occupation(&mut facets, Some(occ.clone())).unwrap();
        assert_eq!(occupation_of(&facets).unwrap(), Some(occ));
        set_occupation(&mut facets, None).unwrap();
        assert!(!facets.contains_key(&Facet::Occupation), "clearing removes the entry");
    }

    #[test]
    fn role_and_occupation_coexist_and_map_to_their_badges() {
        let mut facets = BTreeMap::new();
        set_role(&mut facets, Role::Perpetrator).unwrap();
        set_occupation(
            &mut facets,
            Some(Occupation {
                label: "Judge".to_owned(),
                icon: OccupationIcon::Scales,
            }),
        )
        .unwrap();
        let role_badge = facet_badge(Facet::Role, &facets[&Facet::Role]).unwrap().expect("role badge");
        assert_eq!(role_badge.color, ROLE_META[Role::Perpetrator as usize].color);
        assert_eq!(role_badge.glyph, ROLE_META[Role::Perpetrator as usize].icon);
        let occ_badge =
            facet_badge(Facet::Occupation, &facets[&Facet::Occupation]).unwrap().expect("occ badge");
        assert_eq!(occ_badge.color, OCC_BADGE);
        assert_eq!(occ_badge.glyph, egui_phosphor::regular::SCALES);
    }

    #[test]
    fn unset_role_index_paints_no_badge() {
        assert!(
            facet_badge(Facet::Role, &FacetValue::Indexed(Role::Unset as u16)).unwrap().is_none(),
            "an Unset role index must never draw the dashed 'none' badge"
        );
    }

    #[test]
    fn changing_kind_clears_facets_the_new_kind_does_not_declare() {
        let mut facets = BTreeMap::new();
        set_role(&mut facets, Role::Perpetrator).unwrap();
        set_occupation(
            &mut facets,
            Some(Occupation {
                label: "Judge".to_owned(),
                icon: OccupationIcon::Scales,
            }),
        )
        .unwrap();

        let mut to_org = facets.clone();
        retain_declared_facets(&mut to_org, EntityKind::Organization).unwrap();
        assert_eq!(
            role_of(&to_org).unwrap(),
            Role::Perpetrator,
            "an org still carries the role facet"
        );
        assert!(
            occupation_of(&to_org).unwrap().is_none(),
            "an org is not an individual — its occupation must be cleared at the data layer"
        );

        let mut to_domain = facets.clone();
        retain_declared_facets(&mut to_domain, EntityKind::Domain).unwrap();
        assert!(
            to_domain.is_empty(),
            "a Domain declares no facets — Person→Domain must clear role AND occupation from the store"
        );
    }

    #[test]
    fn store_serde_round_trips_json_and_ron() {
        let mut facets: BTreeMap<Facet, FacetValue> = BTreeMap::new();
        set_role(&mut facets, Role::Enabler).unwrap();
        set_occupation(
            &mut facets,
            Some(Occupation {
                label: "Detective".to_owned(),
                icon: OccupationIcon::Detective,
            }),
        )
        .unwrap();

        let json = serde_json::to_string(&facets).unwrap();
        let from_json: BTreeMap<Facet, FacetValue> = serde_json::from_str(&json).unwrap();
        assert_eq!(facets, from_json, "facet store must survive JSON (case.json / fingerprint)");

        let ron_s = ron::to_string(&facets).unwrap();
        let from_ron: BTreeMap<Facet, FacetValue> = ron::from_str(&ron_s).unwrap();
        assert_eq!(facets, from_ron, "facet store must survive RON (entity.ron)");
    }
}
