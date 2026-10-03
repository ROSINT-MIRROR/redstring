use egui::RichText;

use crate::error::Error;
use crate::model::FlagMeta;

use super::style;

enum Pick {
    Keep,
    Set(&'static str),
    Clear,
}

pub(super) fn flag_row(
    ui: &mut egui::Ui,
    salt: usize,
    flag: &mut Option<String>,
    has_image: bool,
) -> Result<(), Error> {
    let filter_id = egui::Id::new(("flag_filter", salt));
    let mut filter = ui.data_mut(|d| d.get_temp_mut_or_default::<String>(filter_id).clone());
    let mut pick = Pick::Keep;
    let shown = current_label(flag)?;
    ui.horizontal(|ui| -> Result<(), Error> {
        ui.label("flag");
        egui::ComboBox::from_id_salt(("flag", salt))
            .selected_text(shown)
            .height(320.0)
            .show_ui(ui, |ui| {
                pick = flag_list(ui, flag, &mut filter);
            });
        if flag.is_some()
            && style::danger_btn(
                ui,
                egui_phosphor::regular::TRASH,
                "",
                "clear the flag — the node shows its image or kind icon again",
            )?
        {
            pick = Pick::Clear;
        }
        Ok(())
    })
    .inner?;
    ui.data_mut(|d| d.insert_temp(filter_id, filter));
    match pick {
        Pick::Set(code) => *flag = Some(code.to_owned()),
        Pick::Clear => *flag = None,
        Pick::Keep => {}
    }
    if flag.is_some() && has_image {
        ui.label(
            RichText::new("the flag is drawn instead of the image; the image stays linked — clear the flag to show it again")
                .weak()
                .small(),
        );
    }
    Ok(())
}

fn flag_list(ui: &mut egui::Ui, flag: &Option<String>, filter: &mut String) -> Pick {
    let mut pick = Pick::Keep;
    ui.label(
        RichText::new("ISO 3166-1 alpha-2 · countries and other territories, by ISO short name")
            .weak()
            .small(),
    );
    ui.add(
        egui::TextEdit::singleline(filter)
            .hint_text("filter: code or name")
            .desired_width(f32::INFINITY),
    );
    if ui
        .selectable_label(flag.is_none(), "none — image or kind icon")
        .clicked()
    {
        pick = Pick::Clear;
    }
    let needle = filter.trim().to_lowercase();
    for m in FlagMeta::ALL.iter() {
        let hit = needle.is_empty()
            || m.code.to_lowercase().starts_with(&needle)
            || m.name.to_lowercase().contains(&needle);
        if !hit {
            continue;
        }
        let on = flag.as_deref() == Some(m.code);
        if ui.selectable_label(on, format!("{}  {}", m.code, m.name)).clicked() {
            pick = Pick::Set(m.code);
        }
    }
    pick
}

fn current_label(flag: &Option<String>) -> Result<String, Error> {
    let Some(code) = flag.as_deref() else {
        return Ok("none".to_owned());
    };
    let Some(meta) = FlagMeta::of(code)? else {
        return Ok(format!("{code} (not in the bundled ISO set)"));
    };
    Ok(format!("{}  {}", meta.code, meta.name))
}
