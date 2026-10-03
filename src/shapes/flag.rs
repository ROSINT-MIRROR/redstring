use egui::{ColorImage, TextureHandle, TextureOptions};

use crate::error::Error;
use crate::model::FlagMeta;

pub fn flag_texture(ctx: &egui::Context, code: &str) -> Result<Option<TextureHandle>, Error> {
    let Some(meta) = FlagMeta::of(code)? else {
        return Ok(None);
    };
    let id = egui::Id::new(("redstring_flag", meta.code));
    let hit: Option<TextureHandle> = ctx.data(|d| d.get_temp(id));
    let Some(tex) = hit else {
        let tex = ctx.load_texture(format!("flag:{}", meta.code), decode_flag(meta)?, TextureOptions::LINEAR);
        ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
        return Ok(Some(tex));
    };
    Ok(Some(tex))
}

fn decode_flag(meta: &FlagMeta) -> Result<ColorImage, Error> {
    let img = image::load_from_memory(meta.png).map_err(|source| Error::Image {
        path: format!("flag:{}", meta.code).into(),
        source,
    })?;
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}
