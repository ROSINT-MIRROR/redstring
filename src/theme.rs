use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use egui::Color32;
use egui::epaint::Stroke;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::Error;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HexColor(pub Color32);

impl From<Color32> for HexColor {
    fn from(c: Color32) -> Self {
        Self(c)
    }
}

impl From<HexColor> for Color32 {
    fn from(h: HexColor) -> Self {
        h.0
    }
}

impl Serialize for HexColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format_hex_color(self.0))
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum HexRepr {
    Hex(String),
    Rgb(u8, u8, u8),
    Rgba(u8, u8, u8, u8),
}

impl<'de> Deserialize<'de> for HexColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let repr = HexRepr::deserialize(deserializer)?;
        match repr {
            HexRepr::Hex(s) => parse_hex_color(&s)
                .map(HexColor)
                .map_err(serde::de::Error::custom),
            HexRepr::Rgb(r, g, b) => Ok(HexColor(Color32::from_rgb(r, g, b))),
            HexRepr::Rgba(r, g, b, a) => Ok(HexColor(Color32::from_rgba_premultiplied(r, g, b, a))),
        }
    }
}

fn hex_nibble(b: u8) -> Result<u8, Error> {
    if (b'0'..=b'9').contains(&b) {
        return Ok(b - b'0');
    }
    if (b'a'..=b'f').contains(&b) {
        return Ok(b - b'a' + 10);
    }
    if (b'A'..=b'F').contains(&b) {
        return Ok(b - b'A' + 10);
    }
    Err(Error::Render {
        what: format!("invalid hex character in color byte: {b}"),
    })
}

pub fn parse_hex_color(s: &str) -> Result<Color32, Error> {
    let clean = s.trim().trim_start_matches('#');
    let bytes = clean.as_bytes();
    if bytes.len() == 3 {
        let r = hex_nibble(bytes[0])?;
        let g = hex_nibble(bytes[1])?;
        let b = hex_nibble(bytes[2])?;
        return Ok(Color32::from_rgb((r << 4) | r, (g << 4) | g, (b << 4) | b));
    }
    if bytes.len() == 6 {
        let r1 = hex_nibble(bytes[0])?;
        let r0 = hex_nibble(bytes[1])?;
        let g1 = hex_nibble(bytes[2])?;
        let g0 = hex_nibble(bytes[3])?;
        let b1 = hex_nibble(bytes[4])?;
        let b0 = hex_nibble(bytes[5])?;
        return Ok(Color32::from_rgb(
            (r1 << 4) | r0,
            (g1 << 4) | g0,
            (b1 << 4) | b0,
        ));
    }
    if bytes.len() == 8 {
        let r1 = hex_nibble(bytes[0])?;
        let r0 = hex_nibble(bytes[1])?;
        let g1 = hex_nibble(bytes[2])?;
        let g0 = hex_nibble(bytes[3])?;
        let b1 = hex_nibble(bytes[4])?;
        let b0 = hex_nibble(bytes[5])?;
        let a1 = hex_nibble(bytes[6])?;
        let a0 = hex_nibble(bytes[7])?;
        return Ok(Color32::from_rgba_premultiplied(
            (r1 << 4) | r0,
            (g1 << 4) | g0,
            (b1 << 4) | b0,
            (a1 << 4) | a0,
        ));
    }
    Err(Error::Render {
        what: format!("invalid hex color length (expected 3, 6, or 8 characters): '{s}'"),
    })
}

fn format_hex_color(c: Color32) -> String {
    if c.a() == 255 {
        format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", c.r(), c.g(), c.b(), c.a())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Dark,
    Light,
    Nord,
    Corkboard,
    Midnight,
    Dracula,
    SolarizedDark,
    Custom,
}

impl Theme {
    pub const ALL: &[Theme] = &[
        Theme::Dark,
        Theme::Light,
        Theme::Nord,
        Theme::Corkboard,
        Theme::Midnight,
        Theme::Dracula,
        Theme::SolarizedDark,
        Theme::Custom,
    ];

    pub fn label(&self) -> Result<&'static str, Error> {
        Ok(match self {
            Theme::Dark => "Dark",
            Theme::Light => "Light",
            Theme::Nord => "Nord",
            Theme::Corkboard => "Corkboard",
            Theme::Midnight => "Midnight",
            Theme::Dracula => "Dracula",
            Theme::SolarizedDark => "Solarized Dark",
            Theme::Custom => "Custom (theme.ron)",
        })
    }

    pub fn id_str(&self) -> Result<&'static str, Error> {
        Ok(match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
            Theme::Nord => "nord",
            Theme::Corkboard => "corkboard",
            Theme::Midnight => "midnight",
            Theme::Dracula => "dracula",
            Theme::SolarizedDark => "solarized-dark",
            Theme::Custom => "custom",
        })
    }

    pub fn is_dark(&self) -> Result<bool, Error> {
        Ok(match self {
            Theme::Light | Theme::Corkboard => false,
            Theme::Dark
            | Theme::Nord
            | Theme::Midnight
            | Theme::Dracula
            | Theme::SolarizedDark
            | Theme::Custom => true,
        })
    }

    pub fn from_id(id: &str) -> Result<Self, Error> {
        let lower = id.to_lowercase();
        for t in Self::ALL {
            if t.id_str()? == lower {
                return Ok(*t);
            }
        }
        if lower == "solarized_dark" {
            return Ok(Theme::SolarizedDark);
        }
        Err(Error::Render {
            what: format!(
                "bad theme '{id}': choose from dark, light, nord, corkboard, midnight, dracula, solarized-dark, custom"
            ),
        })
    }

    pub fn def(&self) -> Result<ThemeDef, Error> {
        Ok(match self {
            Theme::Dark => dark_theme(),
            Theme::Light => light_theme(),
            Theme::Nord => nord_theme(),
            Theme::Corkboard => corkboard_theme(),
            Theme::Midnight => midnight_theme(),
            Theme::Dracula => dracula_theme(),
            Theme::SolarizedDark => solarized_dark_theme(),
            Theme::Custom => dark_theme(),
        })
    }
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Ok(s) = self.label() else {
            return write!(f, "Theme");
        };
        write!(f, "{s}")
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeDef {
    pub name: String,
    pub dark_mode: bool,
    pub panel_fill: HexColor,
    pub window_fill: HexColor,
    pub faint_bg: HexColor,
    pub extreme_bg: HexColor,
    pub text: HexColor,
    pub weak_text: HexColor,
    pub selection_bg: HexColor,
    pub selection_stroke: HexColor,
    pub yarn_red: HexColor,
    pub unproven_edge: HexColor,
    pub warn: HexColor,
    pub error: HexColor,
}

impl Default for ThemeDef {
    fn default() -> Self {
        dark_theme()
    }
}

impl ThemeDef {
    pub fn to_visuals(&self) -> Result<egui::Visuals, Error> {
        let mut v = if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        v.dark_mode = self.dark_mode;
        v.panel_fill = self.panel_fill.0;
        v.window_fill = self.window_fill.0;
        v.faint_bg_color = self.faint_bg.0;
        v.extreme_bg_color = self.extreme_bg.0;
        v.code_bg_color = self.extreme_bg.0;
        v.hyperlink_color = self.selection_stroke.0;

        v.widgets.noninteractive.bg_fill = self.panel_fill.0;
        v.widgets.noninteractive.weak_bg_fill = self.panel_fill.0;
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, self.text.0);

        v.widgets.inactive.bg_fill = self.faint_bg.0;
        v.widgets.inactive.weak_bg_fill = self.panel_fill.0;
        v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, self.text.0);

        v.widgets.hovered.bg_fill = self.selection_bg.0;
        v.widgets.hovered.weak_bg_fill = self.selection_bg.0;
        v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, self.text.0);

        v.widgets.active.bg_fill = self.selection_bg.0;
        v.widgets.active.weak_bg_fill = self.selection_bg.0;
        v.widgets.active.fg_stroke = Stroke::new(1.0_f32, self.text.0);

        v.widgets.open.bg_fill = self.faint_bg.0;
        v.widgets.open.weak_bg_fill = self.faint_bg.0;
        v.widgets.open.fg_stroke = Stroke::new(1.0_f32, self.text.0);

        v.selection.bg_fill = self.selection_bg.0;
        v.selection.stroke = Stroke::new(1.0_f32, self.selection_stroke.0);

        v.warn_fg_color = self.warn.0;
        v.error_fg_color = self.error.0;

        Ok(v)
    }
}

fn dark_theme() -> ThemeDef {
    ThemeDef {
        name: "Dark".to_owned(),
        dark_mode: true,
        panel_fill: HexColor(Color32::from_rgb(27, 27, 27)),
        window_fill: HexColor(Color32::from_rgb(31, 31, 31)),
        faint_bg: HexColor(Color32::from_rgb(36, 36, 36)),
        extreme_bg: HexColor(Color32::from_rgb(20, 20, 20)),
        text: HexColor(Color32::from_rgb(230, 230, 230)),
        weak_text: HexColor(Color32::from_rgb(150, 150, 150)),
        selection_bg: HexColor(Color32::from_rgb(31, 61, 92)),
        selection_stroke: HexColor(Color32::from_rgb(77, 166, 255)),
        yarn_red: HexColor(Color32::from_rgb(214, 48, 38)),
        unproven_edge: HexColor(Color32::from_rgb(255, 96, 72)),
        warn: HexColor(Color32::from_rgb(235, 168, 62)),
        error: HexColor(Color32::from_rgb(235, 80, 80)),
    }
}

fn light_theme() -> ThemeDef {
    ThemeDef {
        name: "Light".to_owned(),
        dark_mode: false,
        panel_fill: HexColor(Color32::from_rgb(242, 242, 242)),
        window_fill: HexColor(Color32::from_rgb(255, 255, 255)),
        faint_bg: HexColor(Color32::from_rgb(232, 232, 232)),
        extreme_bg: HexColor(Color32::from_rgb(250, 250, 250)),
        text: HexColor(Color32::from_rgb(26, 26, 26)),
        weak_text: HexColor(Color32::from_rgb(110, 110, 110)),
        selection_bg: HexColor(Color32::from_rgb(204, 230, 255)),
        selection_stroke: HexColor(Color32::from_rgb(26, 140, 255)),
        yarn_red: HexColor(Color32::from_rgb(176, 24, 16)),
        unproven_edge: HexColor(Color32::from_rgb(198, 30, 18)),
        warn: HexColor(Color32::from_rgb(200, 122, 20)),
        error: HexColor(Color32::from_rgb(200, 40, 40)),
    }
}

fn nord_theme() -> ThemeDef {
    ThemeDef {
        name: "Nord".to_owned(),
        dark_mode: true,
        panel_fill: HexColor(Color32::from_rgb(46, 52, 64)),
        window_fill: HexColor(Color32::from_rgb(59, 66, 82)),
        faint_bg: HexColor(Color32::from_rgb(67, 76, 94)),
        extreme_bg: HexColor(Color32::from_rgb(36, 41, 51)),
        text: HexColor(Color32::from_rgb(236, 239, 244)),
        weak_text: HexColor(Color32::from_rgb(216, 222, 233)),
        selection_bg: HexColor(Color32::from_rgb(76, 86, 106)),
        selection_stroke: HexColor(Color32::from_rgb(136, 192, 208)),
        yarn_red: HexColor(Color32::from_rgb(191, 97, 106)),
        unproven_edge: HexColor(Color32::from_rgb(208, 135, 112)),
        warn: HexColor(Color32::from_rgb(235, 203, 139)),
        error: HexColor(Color32::from_rgb(191, 97, 106)),
    }
}

fn corkboard_theme() -> ThemeDef {
    ThemeDef {
        name: "Corkboard".to_owned(),
        dark_mode: false,
        panel_fill: HexColor(Color32::from_rgb(216, 202, 168)),
        window_fill: HexColor(Color32::from_rgb(244, 236, 216)),
        faint_bg: HexColor(Color32::from_rgb(235, 220, 185)),
        extreme_bg: HexColor(Color32::from_rgb(255, 253, 245)),
        text: HexColor(Color32::from_rgb(43, 33, 24)),
        weak_text: HexColor(Color32::from_rgb(110, 91, 69)),
        selection_bg: HexColor(Color32::from_rgb(196, 178, 141)),
        selection_stroke: HexColor(Color32::from_rgb(156, 36, 27)),
        yarn_red: HexColor(Color32::from_rgb(179, 26, 16)),
        unproven_edge: HexColor(Color32::from_rgb(191, 74, 36)),
        warn: HexColor(Color32::from_rgb(184, 116, 26)),
        error: HexColor(Color32::from_rgb(168, 32, 26)),
    }
}

fn midnight_theme() -> ThemeDef {
    ThemeDef {
        name: "Midnight".to_owned(),
        dark_mode: true,
        panel_fill: HexColor(Color32::from_rgb(0, 0, 0)),
        window_fill: HexColor(Color32::from_rgb(13, 13, 13)),
        faint_bg: HexColor(Color32::from_rgb(24, 24, 24)),
        extreme_bg: HexColor(Color32::from_rgb(5, 5, 5)),
        text: HexColor(Color32::from_rgb(255, 255, 255)),
        weak_text: HexColor(Color32::from_rgb(166, 166, 166)),
        selection_bg: HexColor(Color32::from_rgb(28, 43, 61)),
        selection_stroke: HexColor(Color32::from_rgb(0, 153, 255)),
        yarn_red: HexColor(Color32::from_rgb(255, 51, 51)),
        unproven_edge: HexColor(Color32::from_rgb(255, 136, 0)),
        warn: HexColor(Color32::from_rgb(255, 204, 0)),
        error: HexColor(Color32::from_rgb(255, 59, 48)),
    }
}

fn dracula_theme() -> ThemeDef {
    ThemeDef {
        name: "Dracula".to_owned(),
        dark_mode: true,
        panel_fill: HexColor(Color32::from_rgb(40, 42, 54)),
        window_fill: HexColor(Color32::from_rgb(33, 34, 44)),
        faint_bg: HexColor(Color32::from_rgb(52, 55, 70)),
        extreme_bg: HexColor(Color32::from_rgb(25, 26, 33)),
        text: HexColor(Color32::from_rgb(248, 248, 242)),
        weak_text: HexColor(Color32::from_rgb(98, 114, 164)),
        selection_bg: HexColor(Color32::from_rgb(68, 71, 90)),
        selection_stroke: HexColor(Color32::from_rgb(189, 147, 249)),
        yarn_red: HexColor(Color32::from_rgb(255, 85, 85)),
        unproven_edge: HexColor(Color32::from_rgb(255, 184, 108)),
        warn: HexColor(Color32::from_rgb(241, 250, 140)),
        error: HexColor(Color32::from_rgb(255, 85, 85)),
    }
}

fn solarized_dark_theme() -> ThemeDef {
    ThemeDef {
        name: "Solarized Dark".to_owned(),
        dark_mode: true,
        panel_fill: HexColor(Color32::from_rgb(0, 43, 54)),
        window_fill: HexColor(Color32::from_rgb(7, 54, 66)),
        faint_bg: HexColor(Color32::from_rgb(10, 65, 79)),
        extreme_bg: HexColor(Color32::from_rgb(0, 33, 43)),
        text: HexColor(Color32::from_rgb(131, 148, 150)),
        weak_text: HexColor(Color32::from_rgb(88, 110, 117)),
        selection_bg: HexColor(Color32::from_rgb(7, 54, 66)),
        selection_stroke: HexColor(Color32::from_rgb(38, 139, 210)),
        yarn_red: HexColor(Color32::from_rgb(220, 50, 47)),
        unproven_edge: HexColor(Color32::from_rgb(203, 75, 22)),
        warn: HexColor(Color32::from_rgb(181, 137, 0)),
        error: HexColor(Color32::from_rgb(220, 50, 47)),
    }
}

pub fn custom_theme_path() -> Result<PathBuf, Error> {
    let base = crate::prefs::config_base()?;
    Ok(base.join("redstring").join("theme.ron"))
}

pub fn load_theme_file(path: &Path) -> Result<ThemeDef, Error> {
    let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    ron::from_str(&text).map_err(|source| Error::RonParse {
        path: path.to_path_buf(),
        source,
    })
}

pub fn save_theme_file(path: &Path, def: &ThemeDef) -> Result<(), Error> {
    let Some(parent) = path.parent() else {
        return Err(Error::NoConfigDir);
    };
    std::fs::create_dir_all(parent).map_err(|source| Error::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    let text = ron::ser::to_string_pretty(def, ron::ser::PrettyConfig::default())
        .map_err(|source| Error::RonSer { source })?;
    std::fs::write(path, text).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

pub fn template_theme_ron() -> Result<&'static str, Error> {
    Ok(r##"(
    name: "Custom Theme",
    dark_mode: true,

    // UI background colors
    panel_fill: "#1e1e2e",
    window_fill: "#181825",
    faint_bg: "#313244",
    extreme_bg: "#11111b",

    // Text ink
    text: "#cdd6f4",
    weak_text: "#a6adc8",

    // Selection & focus
    selection_bg: "#45475a",
    selection_stroke: "#89b4fa",

    // Investigation accents & strings
    yarn_red: "#f38ba8",
    unproven_edge: "#fab387",
    warn: "#f9e2af",
    error: "#f38ba8",
)
"##)
}

pub fn ensure_template_file(path: &Path) -> Result<(), Error> {
    if path.exists() {
        return Ok(());
    }
    write_template_force(path)
}

pub fn write_template_force(path: &Path) -> Result<(), Error> {
    let Some(parent) = path.parent() else {
        return Err(Error::NoConfigDir);
    };
    std::fs::create_dir_all(parent).map_err(|source| Error::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    let content = template_theme_ron()?;
    std::fs::write(path, content).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn file_mtime(path: &Path) -> Result<Option<SystemTime>, Error> {
    if !path.exists() {
        return Ok(None);
    }
    let meta = std::fs::metadata(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let t = meta.modified().map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(Some(t))
}

pub struct ThemeWatcher {
    pub path: PathBuf,
    pub seen_mtime: Option<SystemTime>,
    pub last_poll: Instant,
}

impl ThemeWatcher {
    pub fn new(path: PathBuf) -> Result<Self, Error> {
        let seen = file_mtime(&path)?;
        Ok(Self {
            path,
            seen_mtime: seen,
            last_poll: Instant::now(),
        })
    }

    pub fn poll(&mut self) -> Result<Option<ThemeDef>, Error> {
        if self.last_poll.elapsed().as_millis() < 800 {
            return Ok(None);
        }
        self.last_poll = Instant::now();
        if !self.path.exists() {
            return Ok(None);
        }
        let current_mtime = file_mtime(&self.path)?;
        if current_mtime <= self.seen_mtime {
            return Ok(None);
        }
        self.seen_mtime = current_mtime;
        let loaded = load_theme_file(&self.path)?;
        Ok(Some(loaded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_color_variations() {
        let c = parse_hex_color("#ffffff").unwrap();
        assert_eq!(c, Color32::WHITE);

        let c2 = parse_hex_color("ffffff").unwrap();
        assert_eq!(c2, Color32::WHITE);

        let c3 = parse_hex_color("#fff").unwrap();
        assert_eq!(c3, Color32::WHITE);

        let red = parse_hex_color("#ff0000").unwrap();
        assert_eq!(red, Color32::from_rgb(255, 0, 0));

        let alpha = parse_hex_color("#ff000080").unwrap();
        assert_eq!(alpha, Color32::from_rgba_premultiplied(255, 0, 0, 128));

        assert!(parse_hex_color("invalid").is_err());
        assert!(parse_hex_color("#12").is_err());
        assert!(parse_hex_color("#12345").is_err());
    }

    #[test]
    fn hex_color_round_trip() {
        let hex = HexColor(Color32::from_rgb(46, 52, 64));
        let s = ron::to_string(&hex).unwrap();
        assert_eq!(s, "\"#2e3440\"");

        let back: HexColor = ron::from_str(&s).unwrap();
        assert_eq!(back, hex);

        let tuple_ron = "(46, 52, 64)";
        let from_tuple: HexColor = ron::from_str(tuple_ron).unwrap();
        assert_eq!(from_tuple, hex);
    }

    #[test]
    fn all_predefined_themes_produce_valid_visuals() {
        for t in Theme::ALL {
            let def = t.def().unwrap();
            let v = def.to_visuals().unwrap();
            assert_eq!(v.dark_mode, def.dark_mode);
            assert_eq!(v.panel_fill, def.panel_fill.0);
        }
    }

    #[test]
    fn theme_from_id_lookup() {
        assert_eq!(Theme::from_id("dark").unwrap(), Theme::Dark);
        assert_eq!(Theme::from_id("light").unwrap(), Theme::Light);
        assert_eq!(Theme::from_id("nord").unwrap(), Theme::Nord);
        assert_eq!(Theme::from_id("corkboard").unwrap(), Theme::Corkboard);
        assert_eq!(Theme::from_id("midnight").unwrap(), Theme::Midnight);
        assert_eq!(Theme::from_id("dracula").unwrap(), Theme::Dracula);
        assert_eq!(
            Theme::from_id("solarized-dark").unwrap(),
            Theme::SolarizedDark
        );
        assert_eq!(Theme::from_id("custom").unwrap(), Theme::Custom);
        assert!(Theme::from_id("unknown-theme").is_err());
    }

    #[test]
    fn theme_watcher_detects_file_change() {
        let tmp =
            std::env::temp_dir().join(format!("redstring_test_theme_{}.ron", std::process::id()));
        let _ = std::fs::remove_file(&tmp);

        let mut watcher = ThemeWatcher::new(tmp.clone()).unwrap();
        assert!(watcher.poll().unwrap().is_none());

        ensure_template_file(&tmp).unwrap();
        watcher.seen_mtime = None;
        watcher.last_poll = Instant::now() - std::time::Duration::from_secs(2);

        let loaded = watcher.poll().unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "Custom Theme");

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn save_and_load_theme_file_round_trip() {
        let tmp =
            std::env::temp_dir().join(format!("redstring_save_load_{}.ron", std::process::id()));
        let nord = Theme::Nord.def().unwrap();
        save_theme_file(&tmp, &nord).unwrap();
        let loaded = load_theme_file(&tmp).unwrap();
        assert_eq!(loaded, nord);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn theme_labels_and_ids_are_non_empty() {
        for t in Theme::ALL {
            assert!(!t.label().unwrap().is_empty());
            assert!(!t.id_str().unwrap().is_empty());
            assert_eq!(
                t.is_dark().unwrap(),
                match t {
                    Theme::Light | Theme::Corkboard => false,
                    _ => true,
                }
            );
        }
    }
}
