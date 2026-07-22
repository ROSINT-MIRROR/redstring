use egui::RichText;
use petgraph::graph::NodeIndex;

use crate::error::Error;

use super::app::RedstringApp;
use super::style;

impl RedstringApp {
    pub fn multi_inspector(&mut self, ui: &mut egui::Ui, sel: Vec<NodeIndex>) -> Result<(), Error> {
        style::section(ui, egui_phosphor::regular::USERS_THREE, "selection")?;
        ui.label(RichText::new(format!("{} entities selected", sel.len())).strong());
        ui.label(
            RichText::new("Delete key asks to remove them — Enter confirms")
                .weak()
                .small(),
        );
        super::pathview::section(self, ui, &sel)?;
        ui.add_space(style::GAP);
        if sel.len() == 2 {
            let a = sel[0];
            let b = sel[1];
            if style::action_btn(
                ui,
                egui_phosphor::regular::ARROWS_MERGE,
                "merge into one entity",
            )? {
                self.merge_confirm = Some((a, b));
            }
        }
        if style::action_btn(
            ui,
            egui_phosphor::regular::EYE_SLASH,
            &format!("seal {} entities", sel.len()),
        )? {
            self.seal_confirm = Some((sel.clone(), true));
        }
        if style::action_btn(
            ui,
            egui_phosphor::regular::EYE,
            &format!("unseal {} entities", sel.len()),
        )? {
            self.seal_confirm = Some((sel.clone(), false));
        }
        ui.add_space(style::GAP);
        if style::danger_btn(
            ui,
            egui_phosphor::regular::TRASH,
            &format!("delete {} entities", sel.len()),
            "remove the selected entities and their relations",
        )? {
            self.delete_confirm = Some(sel);
        }
        Ok(())
    }
}
