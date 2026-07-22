use egui::{ColorImage, Pos2, Rect, TextureHandle, TextureOptions};

use crate::error::Error;

const MAX_TEX_SIDE: u32 = 512;
const MAX_TEX_SIDE_FULL: u32 = 2048;

const CLASS_TILE: &str = "tile";
const CLASS_FULL: &str = "full";

pub fn cover_uv(tex_size: [usize; 2]) -> Result<Rect, Error> {
    let (w, h) = (tex_size[0].max(1) as f32, tex_size[1].max(1) as f32);
    let (u, v) = if w > h { (h / w, 1.0) } else { (1.0, w / h) };
    Ok(Rect::from_min_max(
        Pos2::new((1.0 - u) / 2.0, (1.0 - v) / 2.0),
        Pos2::new((1.0 + u) / 2.0, (1.0 + v) / 2.0),
    ))
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
    let Some(hit) = ctx.data(|d| d.get_temp::<Option<TextureHandle>>(id)) else {
        return load_and_cache(ctx, class, cap, path, id);
    };
    hit
}

fn load_and_cache(
    ctx: &egui::Context,
    class: &str,
    cap: u32,
    path: &str,
    id: egui::Id,
) -> Option<TextureHandle> {
    let Ok(ci) = decode(path, cap) else {
        ctx.data_mut(|d| d.insert_temp::<Option<TextureHandle>>(id, None));
        return None;
    };
    let tex = ctx.load_texture(format!("{class}:{path}"), ci, TextureOptions::LINEAR);
    let stored = Some(tex.clone());
    ctx.data_mut(|d| d.insert_temp(id, stored));
    Some(tex)
}

pub fn invalidate_texture(ctx: &egui::Context, path: &str) -> Result<(), Error> {
    ctx.data_mut(|d| {
        d.remove::<Option<TextureHandle>>(cache_id(CLASS_TILE, path));
        d.remove::<Option<TextureHandle>>(cache_id(CLASS_FULL, path));
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
    fn cover_uv_crops_long_axis() {
        let uv = cover_uv([200, 100]).unwrap();
        assert!((uv.min.x - 0.25).abs() < 1e-4 && (uv.max.x - 0.75).abs() < 1e-4);
        assert!((uv.min.y - 0.0).abs() < 1e-4 && (uv.max.y - 1.0).abs() < 1e-4);
        let uv = cover_uv([64, 64]).unwrap();
        assert_eq!(uv, Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)));
    }
}
