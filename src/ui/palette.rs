use egui::{Color32, RichText, Vec2};

use crate::error::Error;
use crate::model::{
    CATEGORY_META, CategoryMeta, KIND_META, KindMeta, PRESETS, Preset, ROLE_META,
};

use super::app::RedstringApp;
use super::style;

const SEARCH_TEXT: f32 = 19.0;

const CHEVRON_W: f32 = 20.0;

const PRESET_STEP: f32 = 24.0;

impl RedstringApp {
    pub fn sidebar_strip(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        ui.vertical_centered(|ui| -> Result<(), Error> {
            if style::square_btn(ui, egui_phosphor::regular::CARET_RIGHT, "expand palette")? {
                self.prefs.sidebar_collapsed = false;
                self.prefs.save()?;
            }
            Ok(())
        })
        .inner
    }

    pub fn left_panel(&mut self, ui: &mut egui::Ui) -> Result<(), Error> {
        ui.horizontal(|ui| -> Result<(), Error> {
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| -> Result<(), Error> {
                    if style::square_btn(ui, egui_phosphor::regular::CARET_LEFT, "collapse palette")? {
                        self.prefs.sidebar_collapsed = true;
                        self.prefs.save()?;
                    }
                    if style::square_btn(ui, egui_phosphor::regular::X, "clear search")? {
                        self.kind_search.clear();
                    }
                    ui.add_sized(
                        Vec2::new(ui.available_width(), style::ROW_H),
                        egui::TextEdit::singleline(&mut self.kind_search)
                            .font(egui::FontId::proportional(SEARCH_TEXT))
                            .vertical_align(egui::Align::Center)
                            .hint_text(
                                RichText::new(format!(
                                    "{} search",
                                    egui_phosphor::regular::MAGNIFYING_GLASS
                                ))
                                .size(SEARCH_TEXT),
                            ),
                    );
                    Ok(())
                },
            )
            .inner
        })
        .inner?;
        ui.separator();
        self.kind_picker(ui);
        Ok(())
    }

    fn kind_picker(&mut self, ui: &mut egui::Ui) {
        let needle = self.kind_search.trim().to_lowercase();
        let searching = !needle.is_empty();
        let dark = ui.visuals().dark_mode;
        egui::ScrollArea::vertical()
            .id_salt("ne_kind_grid")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for cat in &CATEGORY_META {
                    let ci = cat.category as usize;
                    let hits: Vec<&KindMeta> = KIND_META
                        .iter()
                        .filter(|m| {
                            m.category == cat.category
                                && kind_or_preset_matches(m, cat.label, &needle)
                        })
                        .collect();
                    if hits.is_empty() {
                        continue;
                    }
                    let open = searching || self.palette_open[ci];
                    if category_header(ui, cat, open, hits.len(), dark) {
                        self.palette_open[ci] = !self.palette_open[ci];
                    }
                    if !open {
                        continue;
                    }
                    for m in hits {
                        self.kind_tree_row(ui, m, cat, &needle, searching, dark);
                    }
                }
            });
    }

    fn kind_tree_row(
        &mut self,
        ui: &mut egui::Ui,
        m: &KindMeta,
        cat: &CategoryMeta,
        needle: &str,
        searching: bool,
        dark: bool,
    ) {
        let ki = m.kind as usize;
        let has_presets = PRESETS.iter().any(|p| p.base_kind == m.kind);
        let presets: Vec<&Preset> = PRESETS
            .iter()
            .filter(|p| p.base_kind == m.kind && (needle.is_empty() || preset_matches(p, needle)))
            .collect();
        let expanded = if searching {
            !presets.is_empty()
        } else {
            self.kind_expanded[ki]
        };
        let selected = self.ne_kind == m.kind && self.ne_preset.is_none();
        let (armed, toggled) = kind_row(ui, m, cat, selected, has_presets, expanded, dark);
        if toggled {
            self.kind_expanded[ki] = !self.kind_expanded[ki];
        }
        if armed {
            self.ne_kind = m.kind;
            self.ne_preset = None;
        }
        if !expanded {
            return;
        }
        for &p in &presets {
            if preset_row(ui, p, cat, self.ne_preset == Some(*p), dark) {
                self.ne_preset = Some(*p);
            }
        }
    }
}

fn adj(raw: Color32, dark: bool) -> Color32 {
    let Ok(c) = crate::ui::board_color(raw, dark) else {
        return raw;
    };
    c
}

fn category_header(
    ui: &mut egui::Ui,
    cat: &CategoryMeta,
    open: bool,
    count: usize,
    dark: bool,
) -> bool {
    let caret = if open {
        egui_phosphor::regular::CARET_DOWN
    } else {
        egui_phosphor::regular::CARET_RIGHT
    };
    let hc = adj(cat.color, dark);
    let resp = ui.add(
        egui::Button::new((
            RichText::new(format!("{caret}  {}", cat.icon))
                .color(hc)
                .size(16.0f32),
            RichText::new(cat.label).color(hc).strong().size(14.0f32),
            egui::Atom::grow(),
            RichText::new(count.to_string())
                .color(hc.gamma_multiply(0.7f32))
                .size(12.0f32),
        ))
        .fill(hc.gamma_multiply(0.10f32))
        .stroke(egui::Stroke::new(1.0f32, hc.gamma_multiply(0.6f32)))
        .min_size(Vec2::new(ui.available_width(), style::ROW_H)),
    );
    resp.clicked()
}

fn kind_row(
    ui: &mut egui::Ui,
    m: &KindMeta,
    cat: &CategoryMeta,
    selected: bool,
    has_presets: bool,
    expanded: bool,
    dark: bool,
) -> (bool, bool) {
    let mut armed = false;
    let mut toggled = false;
    let kc = adj(m.color, dark);
    ui.horizontal(|ui| {
        toggled = caret_gutter(ui, cat.color, has_presets, expanded, dark);
        let resp = ui.add(
            egui::Button::selectable(
                selected,
                (
                    RichText::new(m.icon).color(kc).size(24.0f32),
                    RichText::new(m.label).color(kc).size(12.0f32),
                    egui::Atom::grow(),
                ),
            )
            .stroke(egui::Stroke::new(1.0f32, adj(cat.color, dark).gamma_multiply(0.5f32)))
            .sense(egui::Sense::click_and_drag())
            .min_size(Vec2::new(ui.available_width(), 40.0f32)),
        );
        resp.dnd_set_drag_payload(m.kind);
        if resp.clicked() {
            armed = true;
        }
    });
    (armed, toggled)
}

fn caret_gutter(
    ui: &mut egui::Ui,
    color: Color32,
    has_presets: bool,
    expanded: bool,
    dark: bool,
) -> bool {
    if !has_presets {
        return false;
    }
    let glyph = if expanded {
        egui_phosphor::regular::CARET_DOWN
    } else {
        egui_phosphor::regular::CARET_RIGHT
    };
    let resp = ui.add(
        egui::Button::new(RichText::new(glyph).color(adj(color, dark)).size(16.0f32))
            .frame(false)
            .min_size(Vec2::new(CHEVRON_W, 40.0f32)),
    );
    resp.clicked()
}

fn preset_row(ui: &mut egui::Ui, p: &Preset, cat: &CategoryMeta, selected: bool, dark: bool) -> bool {
    let color = adj(preset_color(p), dark);
    let mut armed = false;
    ui.horizontal(|ui| {
        let indent = CHEVRON_W + ui.spacing().item_spacing.x + PRESET_STEP;
        ui.add_space(indent);
        let resp = ui.add(
            egui::Button::selectable(
                selected,
                (
                    RichText::new(p.icon).color(color).size(20.0f32),
                    RichText::new(p.label).color(color).size(12.0f32),
                    egui::Atom::grow(),
                ),
            )
            .stroke(egui::Stroke::new(1.0f32, adj(cat.color, dark).gamma_multiply(0.35f32)))
            .sense(egui::Sense::click_and_drag())
            .min_size(Vec2::new(ui.available_width(), 34.0f32)),
        );
        resp.dnd_set_drag_payload(*p);
        if resp.clicked() {
            armed = true;
        }
    });
    armed
}

fn preset_color(p: &Preset) -> Color32 {
    let mut color = KIND_META[p.base_kind as usize].color;
    for r in p.role.iter() {
        color = ROLE_META[*r as usize].color;
    }
    color
}

fn kind_or_preset_matches(m: &KindMeta, cat_label: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    kind_matches(m, cat_label, needle)
        || PRESETS
            .iter()
            .any(|p| p.base_kind == m.kind && preset_matches(p, needle))
}

fn preset_matches(p: &Preset, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    fuzzy(needle, p.label) || p.attr.is_some_and(|(_, val)| fuzzy(needle, val))
}

fn kind_matches(m: &KindMeta, cat_label: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    fuzzy(needle, m.label)
        || fuzzy(needle, cat_label)
        || m.aliases.iter().any(|a| fuzzy(needle, a))
}

fn fuzzy(needle: &str, hay: &str) -> bool {
    let hay: Vec<char> = hay.to_lowercase().chars().collect();
    let mut i = 0usize;
    for n in needle.chars() {
        let mut found = false;
        while i < hay.len() {
            let h = hay[i];
            i += 1;
            if h == n {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EntityKind;

    fn icon_left_with(step: impl Fn(&mut egui::Ui)) -> f32 {
        let ctx = phosphor();
        ctx.all_styles_mut(|s| {
            s.spacing.button_padding = Vec2::new(8.0, 5.0);
            s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        });
        let mut icon_left = 0.0f32;
        let input = egui::RawInput::default();
        let _full = ctx.run_ui(input, |ui| {
            egui::Panel::left("p").exact_size(200.0).show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    step(ui);
                    let icon = ui.add(
                        egui::Button::new(RichText::new("X").size(24.0f32))
                            .min_size(Vec2::new(ui.available_width(), 40.0f32)),
                    );
                    icon_left = icon.rect.min.x;
                });
            });
        });
        icon_left
    }

    #[test]
    fn leaf_flush_left_expandable_reserves_caret() {
        let color = CATEGORY_META[0].color;
        let base = icon_left_with(|_ui| {});
        let leaf = icon_left_with(|ui| {
            caret_gutter(ui, color, false, false, true);
        });
        let collapsed = icon_left_with(|ui| {
            caret_gutter(ui, color, true, false, true);
        });
        let expanded = icon_left_with(|ui| {
            caret_gutter(ui, color, true, true, true);
        });
        assert_eq!(
            leaf, base,
            "leaf kind (no presets) must sit flush-left with no caret gutter: leaf {leaf} vs base {base}"
        );
        assert_eq!(
            collapsed, expanded,
            "caret gutter width must not change with expand state: collapsed {collapsed} vs expanded {expanded}"
        );
        assert!(
            collapsed > leaf + 0.5f32,
            "expandable kind icon must sit right of the flush-left leaf icon: expandable {collapsed} vs leaf {leaf}"
        );
        let gutter = collapsed - base - 8.0f32;
        assert!(
            (gutter - CHEVRON_W).abs() < 0.5f32,
            "expandable caret gutter {gutter} must equal CHEVRON_W {CHEVRON_W}"
        );
    }

    #[test]
    fn preset_icon_sits_right_of_base_kind_icon() {
        let color = CATEGORY_META[0].color;
        let kind_icon = icon_left_with(|ui| {
            caret_gutter(ui, color, true, false, true);
        });
        let leaf_icon = icon_left_with(|ui| {
            caret_gutter(ui, color, false, false, true);
        });
        assert!(
            leaf_icon < kind_icon - 0.5f32,
            "leaf base kind must sit flush-left, left of an expandable kind's icon: leaf {leaf_icon} vs expandable {kind_icon}"
        );
        let indent = CHEVRON_W + 8.0f32 + PRESET_STEP;
        let preset_icon = icon_left_with(|ui| {
            ui.add_space(indent);
        });
        let step = preset_icon - kind_icon;
        assert!(
            (step - PRESET_STEP).abs() < 0.5f32,
            "preset icon must sit PRESET_STEP right of the expandable base-kind icon: preset {preset_icon} vs base {kind_icon} (step {step})"
        );
    }

    #[test]
    fn fuzzy_is_subsequence_match() {
        assert!(fuzzy("prsn", "Person"));
        assert!(fuzzy("govbod", "Government body"));
        assert!(!fuzzy("xyz", "Person"));
        assert!(fuzzy("", "anything"));
    }

    fn phosphor() -> egui::Context {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        ctx.set_fonts(fonts);
        ctx
    }

    // mirror kind_picker exactly: phosphor icon + category stroke + app padding +
    // min_size 40. Fresh ctx, settle animations with a STABLE pointer, read row-2.
    fn settled_row2(sel: usize, hover_row2: bool) -> (f32, egui::Pos2) {
        let ctx = phosphor();
        ctx.all_styles_mut(|s| {
            s.spacing.button_padding = Vec2::new(8.0, 5.0);
            s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        });
        let icon = KIND_META[0].icon;
        let mut out = (0.0f32, egui::Pos2::ZERO);
        let mut pointer = None;
        for _ in 0..80 {
            let mut input = egui::RawInput::default();
            if let Some(p) = pointer {
                input.events.push(egui::Event::PointerMoved(p));
            }
            let _full = ctx.run_ui(input, |ui| {
                egui::Panel::left("p").exact_size(200.0).show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        for i in 0..12usize {
                            let resp = ui.add(
                                egui::Button::selectable(
                                    sel == i,
                                    (
                                        RichText::new(icon).size(24.0f32),
                                        RichText::new("Person").size(12.0f32),
                                        egui::Atom::grow(),
                                    ),
                                )
                                .stroke(egui::Stroke::new(1.0f32, egui::Color32::GRAY))
                                .sense(egui::Sense::click_and_drag())
                                .min_size(Vec2::new(ui.available_width(), 40.0)),
                            );
                            if i == 2 {
                                out = (resp.rect.height(), resp.rect.center());
                            }
                        }
                    });
                });
            });
            pointer = hover_row2.then_some(out.1);
        }
        out
    }

    // the stroke makes the selected-idle row 2px taller than min_size 36 allowed,
    // so hovering it (which drops the stroke's extra) shrank it. min_size 40
    // dominates the tallest state, pinning all states equal. (headless-verified)
    #[test]
    fn palette_row_height_constant_across_states() {
        let unsel = settled_row2(99, false).0;
        let sel_idle = settled_row2(2, false).0;
        let (sel_hover, _) = settled_row2(2, true);
        assert_eq!(
            (unsel, sel_idle, sel_hover),
            (unsel, unsel, unsel),
            "palette row height drifts: unsel {unsel}, sel_idle {sel_idle}, sel_hover {sel_hover}"
        );
    }

    #[test]
    fn grouping_covers_every_kind_once() {
        let mut total = 0usize;
        for cat in &CATEGORY_META {
            let n = KIND_META
                .iter()
                .filter(|m| m.category == cat.category)
                .count();
            assert!(n > 0, "empty category header would render: {}", cat.label);
            total += n;
        }
        assert_eq!(total, KIND_META.len(), "grouping drops or double-counts kinds");
    }

    #[test]
    fn search_hides_nonmatching_groups() {
        let needle = "person";
        let mut shown = 0usize;
        for cat in &CATEGORY_META {
            let hits = KIND_META
                .iter()
                .filter(|m| m.category == cat.category && kind_matches(m, cat.label, needle))
                .count();
            if hits > 0 {
                shown += 1;
            }
        }
        assert!(shown >= 1, "a matching group must survive");
        assert!(shown < CATEGORY_META.len(), "an active query must prune some groups");
    }

    #[test]
    fn aliases_surface_kinds() {
        let weapon = &KIND_META[EntityKind::Weapon as usize];
        assert!(kind_matches(weapon, "Physical", "gun"));
        let ip = &KIND_META[EntityKind::IpAddress as usize];
        assert!(kind_matches(ip, "Digital", "ipv4"));
        assert!(!kind_matches(weapon, "Physical", "newspaper"));
    }

    #[test]
    fn collapsed_subtypes_surface_as_org_presets() {
        let org = &KIND_META[EntityKind::Organization as usize];
        for term in ["shell company", "ngo", "government agency", "company"] {
            assert!(
                kind_or_preset_matches(org, "Organizations", term),
                "'{term}' must surface Organization"
            );
        }
        assert!(!kind_or_preset_matches(org, "Organizations", "airplane"));
    }

    #[test]
    fn preset_matches_hit_label_and_attr_value() {
        let shell = PRESETS.iter().find(|p| p.label == "Shell company").expect("shell preset");
        assert_eq!(shell.base_kind, EntityKind::Organization);
        assert!(preset_matches(shell, "shell"));
        let judge = PRESETS.iter().find(|p| p.label == "Judge").expect("judge preset");
        assert!(preset_matches(judge, "judge"), "occupation attr value matches");
        assert!(!preset_matches(judge, "zzzq"));
    }

    #[test]
    fn preset_color_indexes_safely() {
        let mut sum = 0u32;
        for p in &PRESETS {
            sum += preset_color(p).r() as u32;
        }
        assert!(sum > 0, "presets must carry a visible color");
    }
}
