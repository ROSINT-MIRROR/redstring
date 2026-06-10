use egui::{Color32, RichText, Vec2};

use crate::error::Error;




pub const GAP: f32 = 8.0;

pub const GAP_SM: f32 = 4.0;


pub const ROW_H: f32 = 30.0;

pub const BTN_SQ: f32 = 30.0;

pub const ICON_BTN: f32 = 18.0;

pub const ICON_SECTION: f32 = 15.0;

pub const STRIP_W: f32 = 30.0;


pub const DANGER: Color32 = Color32::from_rgb(235, 80, 80);



pub fn square_btn(ui: &mut egui::Ui, icon: &str, hover: &str) -> Result<bool, Error> {
    let resp = ui
        .add_sized(
            Vec2::splat(BTN_SQ),
            egui::Button::new(RichText::new(icon).size(ICON_BTN)),
        )
        .on_hover_text(hover.to_owned());
    Ok(resp.clicked())
}


pub fn danger_btn(ui: &mut egui::Ui, icon: &str, label: &str, hover: &str) -> Result<bool, Error> {
    let text = if label.is_empty() {
        RichText::new(icon).color(DANGER).size(ICON_BTN)
    } else {
        RichText::new(format!("{icon} {label}")).color(DANGER)
    };
    Ok(ui.button(text).on_hover_text(hover.to_owned()).clicked())
}


pub fn action_btn(ui: &mut egui::Ui, icon: &str, label: &str) -> Result<bool, Error> {
    Ok(ui
        .button(RichText::new(format!("{icon} {label}")))
        .clicked())
}


pub fn section(ui: &mut egui::Ui, icon: &str, title: &str) -> Result<(), Error> {
    ui.add_space(GAP);
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon).size(ICON_SECTION).strong());
        ui.label(RichText::new(title).strong());
    });
    ui.add_space(GAP_SM);
    Ok(())
}
