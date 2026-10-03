use std::time::{Duration, SystemTime};

use egui::{ColorImage, Rect, TextureHandle, TextureOptions, Vec2};

use crate::error::Error;

const MAX_TEX_SIDE: u32 = 512;
const MAX_TEX_SIDE_FULL: u32 = 2048;

const CLASS_TILE: &str = "tile";
const CLASS_FULL: &str = "full";

const RECHECK_SECS: f64 = 1.0;

#[derive(Clone)]
struct Cached {
    tex: Option<TextureHandle>,
    stamp: Option<(SystemTime, u64)>,
    checked: f64,
}

pub fn contain_rect(slot: Rect, tex_size: [usize; 2]) -> Result<Rect, Error> {
    let (w, h) = (tex_size[0].max(1) as f32, tex_size[1].max(1) as f32);
    let scale = (slot.width() / w).min(slot.height() / h);
    Ok(Rect::from_center_size(slot.center(), Vec2::new(w * scale, h * scale)))
}

fn cache_id(class: &str, path: &str) -> egui::Id {
    egui::Id::new(("redstring_texture", class, path))
}

pub fn texture_for(ctx: &egui::Context, path: &str) -> Result<Option<TextureHandle>, Error> {
    Ok(class_texture(ctx, CLASS_TILE, MAX_TEX_SIDE, path))
}

pub fn texture_full_for(ctx: &egui::Context, path: &str) -> Result<Option<TextureHandle>, Error> {
    Ok(class_texture(ctx, CLASS_FULL, MAX_TEX_SIDE_FULL, path))
}

fn class_texture(ctx: &egui::Context, class: &str, cap: u32, path: &str) -> Option<TextureHandle> {
    let id = cache_id(class, path);
    let now = ctx.input(|i| i.time);
    let Some(hit) = ctx.data(|d| d.get_temp::<Cached>(id)) else {
        return load_and_cache(ctx, class, cap, path, id, now);
    };
    if now - hit.checked < RECHECK_SECS {
        if hit.tex.is_none() {
            ctx.request_repaint_after(Duration::from_secs_f64(RECHECK_SECS));
        }
        return hit.tex;
    }
    if file_stamp(path) != hit.stamp {
        return load_and_cache(ctx, class, cap, path, id, now);
    }
    let tex = hit.tex.clone();
    ctx.data_mut(|d| d.insert_temp(id, Cached { checked: now, ..hit }));
    tex
}

fn load_and_cache(
    ctx: &egui::Context,
    class: &str,
    cap: u32,
    path: &str,
    id: egui::Id,
    now: f64,
) -> Option<TextureHandle> {
    let stamp = file_stamp(path);
    let tex = decoded_texture(ctx, class, cap, path);
    if tex.is_none() {
        ctx.request_repaint_after(Duration::from_secs_f64(RECHECK_SECS));
    }
    ctx.data_mut(|d| d.insert_temp(id, Cached { tex: tex.clone(), stamp, checked: now }));
    tex
}

fn decoded_texture(ctx: &egui::Context, class: &str, cap: u32, path: &str) -> Option<TextureHandle> {
    let Ok(ci) = decode(path, cap) else {
        return None;
    };
    Some(ctx.load_texture(format!("{class}:{path}"), ci, TextureOptions::LINEAR))
}

fn file_stamp(path: &str) -> Option<(SystemTime, u64)> {
    let Ok(meta) = std::fs::metadata(path) else {
        return None;
    };
    let Ok(mtime) = meta.modified() else {
        return None;
    };
    Some((mtime, meta.len()))
}

pub fn invalidate_texture(ctx: &egui::Context, path: &str) -> Result<(), Error> {
    ctx.data_mut(|d| {
        d.remove::<Cached>(cache_id(CLASS_TILE, path));
        d.remove::<Cached>(cache_id(CLASS_FULL, path));
    });
    Ok(())
}

fn decode(path: &str, cap: u32) -> Result<ColorImage, Error> {
    let bytes = std::fs::read(path).map_err(|source| Error::Io { path: path.into(), source })?;
    let img =
        image::load_from_memory(&bytes).map_err(|source| Error::Image { path: path.into(), source })?;
    let img = if img.width() > cap || img.height() > cap {
        img.thumbnail(cap, cap)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_or_named_error() {
        let dir = std::env::temp_dir();
        let ok = dir.join("redstring_node_shape_test.png");
        image::RgbaImage::from_pixel(3, 2, image::Rgba([255, 0, 0, 255]))
            .save(&ok)
            .unwrap();
        let ci = decode(ok.to_str().unwrap(), MAX_TEX_SIDE).expect("png must decode");
        assert_eq!((ci.width(), ci.height()), (3, 2));
        std::fs::remove_file(&ok).ok();

        let big = dir.join("redstring_node_shape_test_big.png");
        image::RgbaImage::from_pixel(MAX_TEX_SIDE + 88, 100, image::Rgba([0, 255, 0, 255]))
            .save(&big)
            .unwrap();
        let ci = decode(big.to_str().unwrap(), MAX_TEX_SIDE).expect("png must decode");
        assert!(ci.width() <= MAX_TEX_SIDE as usize);
        std::fs::remove_file(&big).ok();

        assert!(decode("/nonexistent/redstring.png", MAX_TEX_SIDE).is_err());

        let bad = dir.join("redstring_node_shape_test.garbage");
        std::fs::write(&bad, b"not an image").unwrap();
        assert!(decode(bad.to_str().unwrap(), MAX_TEX_SIDE).is_err());
        std::fs::remove_file(&bad).ok();
    }

    #[test]
    fn cache_keys_class_the_same_path_apart() {
        assert_ne!(
            cache_id(CLASS_TILE, "/w.png"),
            cache_id(CLASS_FULL, "/w.png"),
            "the same file must cache separately per size class so a wallpaper never reuses the 512px tile"
        );
        assert_eq!(cache_id(CLASS_TILE, "/w.png"), cache_id(CLASS_TILE, "/w.png"));
        assert_ne!(cache_id(CLASS_TILE, "/w.png"), cache_id(CLASS_TILE, "/x.png"));
    }

    #[test]
    fn full_class_keeps_wallpaper_resolution_tile_thumbnails() {
        let dir = std::env::temp_dir();
        let p = dir.join("redstring_texture_class_test.png");
        let w = MAX_TEX_SIDE + 200;
        image::RgbaImage::from_pixel(w, 300, image::Rgba([0, 0, 255, 255]))
            .save(&p)
            .unwrap();
        let full = decode(p.to_str().unwrap(), MAX_TEX_SIDE_FULL).expect("full-class decode");
        assert_eq!(
            full.width(),
            w as usize,
            "a wallpaper under the full cap must keep its native resolution"
        );
        let tile = decode(p.to_str().unwrap(), MAX_TEX_SIDE).expect("tile-class decode");
        assert!(tile.width() <= MAX_TEX_SIDE as usize, "the tile class must keep the 512 cap");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn contain_rect_keeps_the_whole_image_and_its_aspect() {
        let slot = Rect::from_min_max(egui::Pos2::new(0.0, 0.0), egui::Pos2::new(100.0, 100.0));
        let wide = contain_rect(slot, [200, 100]).unwrap();
        assert_eq!((wide.width(), wide.height()), (100.0, 50.0), "a wide image is letterboxed, never cropped");
        assert_eq!(wide.center(), slot.center());
        assert_eq!(contain_rect(slot, [512, 512]).unwrap(), slot, "a square image fills the tile");
    }

    fn texture_at(ctx: &egui::Context, path: &str, t: f64) -> Option<[usize; 2]> {
        let input = egui::RawInput { time: Some(t), ..Default::default() };
        let mut size = None;
        let _full = ctx.run_ui(input, |ui| {
            size = texture_for(ui.ctx(), path).unwrap().map(|tex| tex.size());
        });
        size
    }

    #[test]
    fn a_missing_image_loads_once_the_file_appears() {
        let p = std::env::temp_dir().join(format!("redstring_late_{}.png", std::process::id()));
        let path = p.to_str().unwrap().to_owned();
        std::fs::remove_file(&p).ok();
        let ctx = egui::Context::default();
        assert_eq!(texture_at(&ctx, &path, 0.0), None, "nothing on disk yet");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 0])).save(&p).unwrap();
        assert_eq!(texture_at(&ctx, &path, 0.4), None, "inside the recheck window the miss is cached");
        assert_eq!(
            texture_at(&ctx, &path, 1.5),
            Some([4, 4]),
            "an image written after its entity must load without restart or rename"
        );
        image::RgbaImage::from_pixel(9, 5, image::Rgba([255, 0, 0, 255])).save(&p).unwrap();
        assert_eq!(texture_at(&ctx, &path, 3.0), Some([9, 5]), "a replaced image reloads");
        std::fs::remove_file(&p).ok();
    }
}
