use egui::{ColorImage, Pos2, Rect, TextureHandle, TextureOptions};

use crate::error::Error;

const MAX_TEX_SIDE: u32 = 512;

pub fn cover_uv(tex_size: [usize; 2]) -> Result<Rect, Error> {
    let (w, h) = (tex_size[0].max(1) as f32, tex_size[1].max(1) as f32);
    let (u, v) = if w > h { (h / w, 1.0) } else { (1.0, w / h) };
    Ok(Rect::from_min_max(
        Pos2::new((1.0 - u) / 2.0, (1.0 - v) / 2.0),
        Pos2::new((1.0 + u) / 2.0, (1.0 + v) / 2.0),
    ))
}

fn cache_id(path: &str) -> egui::Id {
    egui::Id::new(("redstring_entity_image", path))
}

pub fn texture_for(ctx: &egui::Context, path: &str) -> Result<Option<TextureHandle>, Error> {
    let id = cache_id(path);
    let Some(hit) = ctx.data(|d| d.get_temp::<Option<TextureHandle>>(id)) else {
        return Ok(load_and_cache(ctx, path, id));
    };
    Ok(hit)
}

fn load_and_cache(ctx: &egui::Context, path: &str, id: egui::Id) -> Option<TextureHandle> {
    let Ok(ci) = decode(path) else {
        ctx.data_mut(|d| d.insert_temp::<Option<TextureHandle>>(id, None));
        return None;
    };
    let tex = ctx.load_texture(format!("entity_image:{path}"), ci, TextureOptions::LINEAR);
    let stored = Some(tex.clone());
    ctx.data_mut(|d| d.insert_temp(id, stored));
    Some(tex)
}

pub fn invalidate_texture(ctx: &egui::Context, path: &str) -> Result<(), Error> {
    ctx.data_mut(|d| d.remove::<Option<TextureHandle>>(cache_id(path)));
    Ok(())
}

fn decode(path: &str) -> Result<ColorImage, Error> {
    let bytes = std::fs::read(path).map_err(|source| Error::Io { path: path.into(), source })?;
    let img =
        image::load_from_memory(&bytes).map_err(|source| Error::Image { path: path.into(), source })?;
    let img = if img.width() > MAX_TEX_SIDE || img.height() > MAX_TEX_SIDE {
        img.thumbnail(MAX_TEX_SIDE, MAX_TEX_SIDE)
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
        let ci = decode(ok.to_str().unwrap()).expect("png must decode");
        assert_eq!((ci.width(), ci.height()), (3, 2));
        std::fs::remove_file(&ok).ok();

        let big = dir.join("redstring_node_shape_test_big.png");
        image::RgbaImage::from_pixel(MAX_TEX_SIDE + 88, 100, image::Rgba([0, 255, 0, 255]))
            .save(&big)
            .unwrap();
        let ci = decode(big.to_str().unwrap()).expect("png must decode");
        assert!(ci.width() <= MAX_TEX_SIDE as usize);
        std::fs::remove_file(&big).ok();

        assert!(decode("/nonexistent/redstring.png").is_err());

        let bad = dir.join("redstring_node_shape_test.garbage");
        std::fs::write(&bad, b"not an image").unwrap();
        assert!(decode(bad.to_str().unwrap()).is_err());
        std::fs::remove_file(&bad).ok();
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
