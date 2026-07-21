use egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum OccupationIcon {
    #[default]
    Briefcase,
    Scales,
    Gavel,
    PenNib,
    Shield,
    Microphone,
    Stethoscope,
    FirstAid,
    Bank,
    ChartLine,
    GraduationCap,
    Chalkboard,
    Code,
    Wrench,
    HardHat,
    Camera,
    FilmSlate,
    Palette,
    Flask,
    Storefront,
    Truck,
    Airplane,
    Anchor,
    ForkKnife,
    Tractor,
    Broadcast,
    Detective,
    Sword,
}

pub struct OccIconMeta {
    pub icon: OccupationIcon,
    pub glyph: &'static str,
    pub label: &'static str,
}

const fn oi(icon: OccupationIcon, glyph: &'static str, label: &'static str) -> OccIconMeta {
    OccIconMeta { icon, glyph, label }
}

pub const OCC_BADGE: Color32 = Color32::from_rgb(96, 116, 138);

#[rustfmt::skip]
pub const OCCUPATION_ICONS: [OccIconMeta; 28] = [
    oi(OccupationIcon::Briefcase, egui_phosphor::regular::BRIEFCASE, "Professional / office"),
    oi(OccupationIcon::Scales, egui_phosphor::regular::SCALES, "Judge / legal"),
    oi(OccupationIcon::Gavel, egui_phosphor::regular::GAVEL, "Judiciary / court"),
    oi(OccupationIcon::PenNib, egui_phosphor::regular::PEN_NIB, "Journalist / writer"),
    oi(OccupationIcon::Shield, egui_phosphor::regular::SHIELD_STAR, "Police / law enforcement"),
    oi(OccupationIcon::Microphone, egui_phosphor::regular::MICROPHONE_STAGE, "Politician / speaker"),
    oi(OccupationIcon::Stethoscope, egui_phosphor::regular::STETHOSCOPE, "Doctor / physician"),
    oi(OccupationIcon::FirstAid, egui_phosphor::regular::FIRST_AID, "Medic / nurse"),
    oi(OccupationIcon::Bank, egui_phosphor::regular::BANK, "Banker / finance"),
    oi(OccupationIcon::ChartLine, egui_phosphor::regular::CHART_LINE_UP, "Analyst / trader"),
    oi(OccupationIcon::GraduationCap, egui_phosphor::regular::GRADUATION_CAP, "Academic / student"),
    oi(OccupationIcon::Chalkboard, egui_phosphor::regular::CHALKBOARD_TEACHER, "Teacher / educator"),
    oi(OccupationIcon::Code, egui_phosphor::regular::CODE, "Developer / engineer"),
    oi(OccupationIcon::Wrench, egui_phosphor::regular::WRENCH, "Mechanic / technician"),
    oi(OccupationIcon::HardHat, egui_phosphor::regular::HARD_HAT, "Construction / labor"),
    oi(OccupationIcon::Camera, egui_phosphor::regular::CAMERA, "Photographer"),
    oi(OccupationIcon::FilmSlate, egui_phosphor::regular::FILM_SLATE, "Filmmaker / producer"),
    oi(OccupationIcon::Palette, egui_phosphor::regular::PALETTE, "Artist / designer"),
    oi(OccupationIcon::Flask, egui_phosphor::regular::FLASK, "Scientist / researcher"),
    oi(OccupationIcon::Storefront, egui_phosphor::regular::STOREFRONT, "Merchant / shopkeeper"),
    oi(OccupationIcon::Truck, egui_phosphor::regular::TRUCK, "Driver / logistics"),
    oi(OccupationIcon::Airplane, egui_phosphor::regular::AIRPLANE_TILT, "Pilot / aviation"),
    oi(OccupationIcon::Anchor, egui_phosphor::regular::ANCHOR, "Maritime / navy"),
    oi(OccupationIcon::ForkKnife, egui_phosphor::regular::FORK_KNIFE, "Chef / hospitality"),
    oi(OccupationIcon::Tractor, egui_phosphor::regular::TRACTOR, "Farmer / agriculture"),
    oi(OccupationIcon::Broadcast, egui_phosphor::regular::BROADCAST, "Broadcaster / media"),
    oi(OccupationIcon::Detective, egui_phosphor::regular::DETECTIVE, "Investigator / intel"),
    oi(OccupationIcon::Sword, egui_phosphor::regular::SWORD, "Military / armed forces"),
];

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Occupation {
    pub label: String,
    #[serde(default)]
    pub icon: OccupationIcon,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occupation_icons_mirror_declaration_order() {
        for (i, m) in OCCUPATION_ICONS.iter().enumerate() {
            assert_eq!(m.icon as usize, i, "OCCUPATION_ICONS[{i}] is {:?}", m.icon);
            assert!(!m.glyph.is_empty(), "{:?} glyph empty", m.icon);
            assert!(!m.label.is_empty(), "{:?} label empty", m.icon);
        }
    }

    #[test]
    fn default_icon_is_briefcase() {
        assert_eq!(OccupationIcon::default(), OccupationIcon::Briefcase);
        assert_eq!(
            OCCUPATION_ICONS[OccupationIcon::default() as usize].glyph,
            egui_phosphor::regular::BRIEFCASE
        );
    }

    #[test]
    fn glyphs_are_distinct_phosphor_codepoints() {
        for (i, a) in OCCUPATION_ICONS.iter().enumerate() {
            let mut chars = a.glyph.chars();
            let c = chars.next().expect("glyph has at least one char");
            assert!(chars.next().is_none(), "{:?} glyph must be a single codepoint", a.icon);
            assert!(c as u32 >= 0xE000, "{:?} glyph must be a phosphor PUA codepoint", a.icon);
            for b in OCCUPATION_ICONS.iter().skip(i + 1) {
                assert_ne!(a.glyph, b.glyph, "duplicate glyph {:?}/{:?}", a.icon, b.icon);
            }
        }
    }

    #[test]
    fn occupation_round_trips_through_ron() {
        let o = Occupation {
            label: "Judge".to_owned(),
            icon: OccupationIcon::Scales,
        };
        let s = ron::to_string(&o).expect("serialize");
        let back: Occupation = ron::from_str(&s).expect("deserialize");
        assert_eq!(o, back);
    }

    #[test]
    fn occupation_icon_defaults_when_absent() {
        let back: Occupation = ron::from_str("(label: \"Spy\")").expect("deserialize without icon");
        assert_eq!(back.icon, OccupationIcon::Briefcase);
    }
}
