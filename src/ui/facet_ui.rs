use std::collections::BTreeMap;

use egui::RichText;

use crate::error::Error;
use crate::model::{
    EntityKind, FACET_META, Facet, FacetValue, KIND_META, OCCUPATION_ICONS, Occupation,
    OccupationIcon, ROLE_META, Role, SubtypeVocab, occupation_of, role_of, set_occupation, set_role,
    set_subtype, subtype_of, subtype_value, subtype_vocab,
};

use super::style;

pub fn facet_rows(
    ui: &mut egui::Ui,
    kind: EntityKind,
    facets: &mut BTreeMap<Facet, FacetValue>,
) -> Result<(), Error> {
    for facet in KIND_META[kind as usize].facets {
        let Some(meta) = FACET_META.iter().find(|m| m.facet == *facet) else {
            continue;
        };
        match facet {
            Facet::Role => {
                ui.horizontal(|ui| -> Result<(), Error> {
                    ui.label(meta.label).on_hover_text(meta.hint);
                    role_field(ui, facets)
                })
                .inner?;
            }
            Facet::Occupation => {
                ui.horizontal(|ui| -> Result<(), Error> {
                    ui.label(meta.label).on_hover_text(meta.hint);
                    occupation_field(ui, facets)
                })
                .inner?;
            }
            Facet::OrgType
            | Facet::AccountType
            | Facet::DomainType
            | Facet::SiteType
            | Facet::ClaimType => {
                ui.horizontal(|ui| -> Result<(), Error> {
                    ui.label(meta.label).on_hover_text(meta.hint);
                    subtype_field(ui, facets, *facet)
                })
                .inner?;
            }
        }
    }
    Ok(())
}

fn subtype_field(
    ui: &mut egui::Ui,
    facets: &mut BTreeMap<Facet, FacetValue>,
    facet: Facet,
) -> Result<(), Error> {
    let Some(vocab) = subtype_vocab(facet)? else {
        return Ok(());
    };
    let mut sel = subtype_of(facets, facet)?;
    subtype_combo(ui, facet, vocab, &mut sel)?;
    set_subtype(facets, facet, sel)?;
    Ok(())
}

fn subtype_combo(
    ui: &mut egui::Ui,
    facet: Facet,
    vocab: SubtypeVocab,
    sel: &mut Option<u16>,
) -> Result<(), Error> {
    let text = subtype_selected_text(facet, *sel)?;
    egui::ComboBox::from_id_salt(("insp_subtype", facet as usize))
        .selected_text(text)
        .show_ui(ui, |ui| {
            ui.selectable_value(sel, None, "none");
            for (i, v) in vocab.values.iter().enumerate() {
                ui.selectable_value(
                    sel,
                    Some(i as u16),
                    RichText::new(format!("{} {}", v.icon, v.label)).color(v.color),
                );
            }
        });
    Ok(())
}

fn subtype_selected_text(facet: Facet, sel: Option<u16>) -> Result<RichText, Error> {
    let Some(i) = sel else {
        return Ok(RichText::new("none"));
    };
    let Some(v) = subtype_value(facet, i)? else {
        return Ok(RichText::new("none"));
    };
    Ok(RichText::new(format!("{} {}", v.icon, v.label)).color(v.color))
}

fn role_field(ui: &mut egui::Ui, facets: &mut BTreeMap<Facet, FacetValue>) -> Result<(), Error> {
    let mut role = role_of(facets)?;
    role_combo(ui, "insp_role", &mut role);
    set_role(facets, role)?;
    Ok(())
}

fn occupation_field(
    ui: &mut egui::Ui,
    facets: &mut BTreeMap<Facet, FacetValue>,
) -> Result<(), Error> {
    let mut occ = occupation_of(facets)?;
    let mut label = String::new();
    for o in occ.iter() {
        label = o.label.clone();
    }
    let resp = ui.add(
        egui::TextEdit::singleline(&mut label)
            .desired_width(140.0)
            .hint_text("occupation"),
    );
    if resp.changed() {
        let trimmed = label.trim().to_owned();
        if trimmed.is_empty() {
            occ = None;
        } else {
            let mut icon = OccupationIcon::default();
            for o in occ.iter() {
                icon = o.icon;
            }
            occ = Some(Occupation { label: trimmed, icon });
        }
    }
    let Some(o) = occ.as_mut() else {
        set_occupation(facets, None)?;
        return Ok(());
    };
    occupation_icon_combo(ui, &mut o.icon)?;
    if style::danger_btn(ui, egui_phosphor::regular::TRASH, "", "clear occupation")? {
        set_occupation(facets, None)?;
        return Ok(());
    }
    set_occupation(facets, occ)?;
    Ok(())
}

fn occupation_icon_combo(ui: &mut egui::Ui, icon: &mut OccupationIcon) -> Result<(), Error> {
    let cur = &OCCUPATION_ICONS[*icon as usize];
    egui::ComboBox::from_id_salt("insp_occ_icon")
        .selected_text(RichText::new(cur.glyph).size(16.0))
        .show_ui(ui, |ui| {
            for m in &OCCUPATION_ICONS {
                ui.selectable_value(icon, m.icon, RichText::new(format!("{}  {}", m.glyph, m.label)))
                    .on_hover_text(m.label);
            }
        })
        .response
        .on_hover_text("pick the occupation badge icon");
    Ok(())
}

fn role_combo(ui: &mut egui::Ui, id: &str, role: &mut Role) {
    let cur = &ROLE_META[*role as usize];
    egui::ComboBox::from_id_salt(id)
        .selected_text(RichText::new(format!("{} {}", cur.icon, cur.label)).color(cur.color))
        .show_ui(ui, |ui| {
            for m in &ROLE_META {
                ui.selectable_value(
                    role,
                    m.role,
                    RichText::new(format!("{} {}", m.icon, m.label)).color(m.color),
                );
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn painted_texts(kind: EntityKind) -> Vec<String> {
        let ctx = egui::Context::default();
        let mut facets: BTreeMap<Facet, FacetValue> = BTreeMap::new();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                facet_rows(ui, kind, &mut facets).expect("facet rows render headless");
            });
        });
        let mut texts = Vec::new();
        for clipped in &out.shapes {
            collect_text(&clipped.shape, &mut texts);
        }
        texts
    }

    fn collect_text(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_owned()),
            egui::epaint::Shape::Vec(v) => {
                for s in v {
                    collect_text(s, out);
                }
            }
            _ => {}
        }
    }

    fn has(texts: &[String], needle: &str) -> bool {
        texts.iter().any(|t| t.contains(needle))
    }

    #[test]
    fn domain_form_renders_neither_role_nor_occupation() {
        let texts = painted_texts(EntityKind::Domain);
        assert!(!has(&texts, "role"), "Domain must not render a role row: {texts:?}");
        assert!(
            !has(&texts, "occupation"),
            "Domain must not render an occupation field: {texts:?}"
        );
    }

    #[test]
    fn person_form_renders_both_role_and_occupation() {
        let texts = painted_texts(EntityKind::Person);
        assert!(has(&texts, "role"), "Person must render a role row: {texts:?}");
        assert!(
            has(&texts, "occupation"),
            "Person must render an occupation field: {texts:?}"
        );
    }

    #[test]
    fn organization_form_renders_role_but_not_occupation() {
        let texts = painted_texts(EntityKind::Organization);
        assert!(has(&texts, "role"), "Organization must render a role row: {texts:?}");
        assert!(
            !has(&texts, "occupation"),
            "Organization must not render an occupation field: {texts:?}"
        );
    }

    #[test]
    fn organization_form_renders_org_type_row() {
        let texts = painted_texts(EntityKind::Organization);
        assert!(has(&texts, "org type"), "Organization must render an org-type row: {texts:?}");
    }

    #[test]
    fn social_account_form_renders_account_type_row() {
        let texts = painted_texts(EntityKind::SocialAccount);
        assert!(
            has(&texts, "account type"),
            "SocialAccount must render an account-type row: {texts:?}"
        );
        assert!(!has(&texts, "role"), "SocialAccount is not an actor — no role row: {texts:?}");
    }

    #[test]
    fn domain_form_renders_domain_type_row() {
        let texts = painted_texts(EntityKind::Domain);
        assert!(has(&texts, "domain type"), "Domain must render a domain-type row: {texts:?}");
    }

    #[test]
    fn website_form_renders_site_type_row() {
        let texts = painted_texts(EntityKind::Website);
        assert!(has(&texts, "site type"), "Website must render a site-type row: {texts:?}");
    }
}
