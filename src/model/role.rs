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
    pub color: Color32,
    pub base_kind: EntityKind,
    pub occupation: Option<&'static str>,
    pub role: Role,
}

const fn ps(
    label: &'static str,
    icon: &'static str,
    rgb: (u8, u8, u8),
    base_kind: EntityKind,
    occupation: Option<&'static str>,
    role: Role,
) -> Preset {
    Preset {
        label,
        icon,
        color: Color32::from_rgb(rgb.0, rgb.1, rgb.2),
        base_kind,
        occupation,
        role,
    }
}

#[rustfmt::skip]
pub const PRESETS: [Preset; 9] = [
    ps("Judge", egui_phosphor::regular::SCALES, (110, 120, 215), EntityKind::Person, Some("Judge"), Role::Unset),
    ps("Journalist", egui_phosphor::regular::PEN_NIB, (120, 160, 240), EntityKind::Person, Some("Journalist"), Role::Unset),
    ps("Lawyer", egui_phosphor::regular::BRIEFCASE, (90, 140, 220), EntityKind::Person, Some("Lawyer"), Role::Unset),
    ps("Police officer", egui_phosphor::regular::SHIELD_STAR, (70, 135, 225), EntityKind::Person, Some("Police officer"), Role::Unset),
    ps("Politician", egui_phosphor::regular::MICROPHONE_STAGE, (140, 150, 235), EntityKind::Person, Some("Politician"), Role::Unset),
    ps("Suspect", egui_phosphor::regular::USER_FOCUS, (235, 165, 60), EntityKind::Person, None, Role::Suspect),
    ps("Victim", egui_phosphor::regular::BANDAIDS, (95, 155, 240), EntityKind::Person, None, Role::Victim),
    ps("Witness", egui_phosphor::regular::EYE, (90, 195, 165), EntityKind::Person, None, Role::Witness),
    ps("Perpetrator", egui_phosphor::regular::HAND_FIST, (225, 70, 70), EntityKind::Person, None, Role::Perpetrator),
];

pub fn apply_preset(e: &mut Entity, preset: &Preset, author: &Author) -> Result<(), Error> {
    e.role = preset.role;
    let Some(occ) = preset.occupation else {
        return Ok(());
    };
    if e.observations.iter().any(|o| o.name == "occupation") {
        return Ok(());
    }
    e.observations.push(Observation::stamp(
        "occupation".to_owned(),
        ObsValue::Text(occ.to_owned()),
        Origin::Manual,
        Verification::AnalystAsserted,
        author.clone(),
    )?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DEFAULT_NODE_SIZE;

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
    fn presets_are_person_based_with_a_facet() {
        assert!(!PRESETS.is_empty());
        for p in &PRESETS {
            assert_eq!(p.base_kind, EntityKind::Person, "{} is not Person-based", p.label);
            assert!(
                p.occupation.is_some() || p.role != Role::Unset,
                "{} sets no facet",
                p.label
            );
            assert!((p.role as usize) < ROLE_META.len(), "{} bad role index", p.label);
            assert!(!p.icon.is_empty(), "{} icon", p.label);
        }
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
            .find(|p| p.role == Role::Suspect)
            .expect("suspect preset");
        apply_preset(&mut e, suspect, &Author::default()).unwrap();
        assert_eq!(e.role, Role::Suspect);
        assert!(e.observations.is_empty(), "role preset must not add an occupation");
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
