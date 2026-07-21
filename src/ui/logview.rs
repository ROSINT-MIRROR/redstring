use std::sync::atomic::Ordering;

use egui::{Color32, RichText, Vec2, Visuals};

use crate::error::Error;
use crate::log::{Level, Record, Snapshot};

struct Intents {
    filters: Vec<(Level, bool)>,
    row_clicks: Vec<(u64, bool)>,
    copy_all: bool,
    copy_selected: bool,
    clear_selection: bool,
    autoscroll: bool,
}

pub fn window(ctx: &egui::Context) -> Result<(), Error> {
    if !crate::log::ENABLED.load(Ordering::Relaxed) {
        return Ok(());
    }
    let snap = crate::log::snapshot()?;
    if !snap.open {
        return Ok(());
    }
    let rows = crate::log::visible(&snap)?;

    let mut open = true;
    let mut it = Intents {
        filters: Vec::new(),
        row_clicks: Vec::new(),
        copy_all: false,
        copy_selected: false,
        clear_selection: false,
        autoscroll: snap.autoscroll,
    };

    egui::Window::new(format!("{} debug log", egui_phosphor::regular::BUG))
        .open(&mut open)
        .default_size(Vec2::new(820.0, 460.0))
        .show(ctx, |ui| {
            controls(ui, &snap, &rows, &mut it);
            ui.separator();
            rows_view(ui, &snap, &rows, &mut it);
        });

    apply(ctx, &snap, open, &it)
}

fn controls(ui: &mut egui::Ui, snap: &Snapshot, rows: &[Record], it: &mut Intents) {
    ui.horizontal_wrapped(|ui| {
        level_toggle(ui, snap, Level::Debug, "Debug", &mut it.filters);
        level_toggle(ui, snap, Level::Info, "Info", &mut it.filters);
        level_toggle(ui, snap, Level::Warn, "Warn", &mut it.filters);
        level_toggle(ui, snap, Level::Error, "Error", &mut it.filters);
        ui.separator();
        if ui
            .button(format!("{} Copy all", egui_phosphor::regular::COPY))
            .on_hover_text("copy the whole buffer to the clipboard")
            .clicked()
        {
            it.copy_all = true;
        }
        if ui
            .button(format!("{} Copy selected", egui_phosphor::regular::COPY))
            .on_hover_text("copy only the highlighted rows")
            .clicked()
        {
            it.copy_selected = true;
        }
        if ui.button("Clear selection").clicked() {
            it.clear_selection = true;
        }
        ui.checkbox(&mut it.autoscroll, "auto-scroll");
        ui.separator();
        ui.label(
            RichText::new(format!(
                "{} shown / {} total / {} selected",
                rows.len(),
                snap.records.len(),
                snap.selected.len()
            ))
            .weak(),
        );
    });
}

fn level_toggle(
    ui: &mut egui::Ui,
    snap: &Snapshot,
    level: Level,
    label: &str,
    out: &mut Vec<(Level, bool)>,
) {
    let cur = shown(snap, level);
    let color = level_color(level, ui.visuals());
    let resp = ui.selectable_label(cur, RichText::new(label).color(color));
    if resp.clicked() {
        out.push((level, !cur));
    }
}

fn rows_view(ui: &mut egui::Ui, snap: &Snapshot, rows: &[Record], it: &mut Intents) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(it.autoscroll)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            for r in rows {
                let selected = snap.selected.contains(&r.seq);
                let color = level_color(r.level, ui.visuals());
                let resp =
                    ui.selectable_label(selected, RichText::new(row_text(r)).monospace().color(color));
                if resp.clicked() {
                    it.row_clicks.push((r.seq, !selected));
                }
            }
        });
}

fn apply(ctx: &egui::Context, snap: &Snapshot, open: bool, it: &Intents) -> Result<(), Error> {
    if !open {
        crate::log::set_open(false)?;
    }
    for (level, on) in &it.filters {
        crate::log::set_filter(*level, *on)?;
    }
    if it.autoscroll != snap.autoscroll {
        crate::log::set_autoscroll(it.autoscroll)?;
    }
    if it.clear_selection {
        crate::log::clear_selection()?;
    }
    for (seq, on) in &it.row_clicks {
        crate::log::select(*seq, *on)?;
    }
    if it.copy_all {
        ctx.copy_text(crate::log::format_records(&snap.records)?);
    }
    if it.copy_selected {
        let mut sel: Vec<Record> = Vec::new();
        for r in &snap.records {
            if snap.selected.contains(&r.seq) {
                sel.push(r.clone());
            }
        }
        ctx.copy_text(crate::log::format_records(&sel)?);
    }
    Ok(())
}

fn row_text(r: &Record) -> String {
    format!(
        "{:>6} {:>8}ms {:<5} {:<14} {}",
        r.seq,
        r.elapsed_ms,
        tag(r.level),
        r.target,
        r.msg
    )
}

fn shown(snap: &Snapshot, level: Level) -> bool {
    match level {
        Level::Debug => snap.show_debug,
        Level::Info => snap.show_info,
        Level::Warn => snap.show_warn,
        Level::Error => snap.show_error,
    }
}

fn tag(level: Level) -> &'static str {
    match level {
        Level::Debug => "DBG",
        Level::Info => "INF",
        Level::Warn => "WRN",
        Level::Error => "ERR",
    }
}

fn level_color(level: Level, visuals: &Visuals) -> Color32 {
    match level {
        Level::Debug => visuals.weak_text_color(),
        Level::Info => visuals.text_color(),
        Level::Warn => Color32::from_rgb(235, 170, 60),
        Level::Error => Color32::from_rgb(235, 80, 80),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn window_renders_without_panic() {
        crate::log::enable().unwrap();
        crate::log::set_open(true).unwrap();
        crate::log::record(crate::log::Level::Info, "smoke", "hello world".to_owned()).unwrap();
        crate::log::record(crate::log::Level::Error, "smoke", "boom".to_owned()).unwrap();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                super::window(ctx).unwrap();
            });
        }
    }
}
