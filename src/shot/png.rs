use crate::error::Error;
use crate::model::Camera;
use crate::ui::RsGraph;

use super::engine::{Opts, RenderedFrame, run_frames};

const BOARD_CLEAR: wgpu::Color = wgpu::Color {
    r: 0.043,
    g: 0.043,
    b: 0.051,
    a: 1.0,
};

pub(super) fn render_png(g: &mut RsGraph, cam: Camera, opts: &Opts) -> Result<String, Error> {
    let frame: RenderedFrame = run_frames(opts.width, opts.height, cam, g)?;
    let ppp = frame.pixels_per_point;
    let primitives = frame.ctx.tessellate(frame.shapes, ppp);

    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .map_err(|e| Error::Render {
                what: format!("no usable wgpu adapter: {e}"),
            })?;
    let info = adapter.get_info();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(
            |e| Error::Render {
                what: format!("request_device: {e}"),
            },
        )?;

    let format = wgpu::TextureFormat::Rgba8Unorm;
    let extent = wgpu::Extent3d {
        width: opts.width,
        height: opts.height,
        depth_or_array_layers: 1,
    };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("redstring_shot_target"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let mut renderer =
        egui_wgpu::Renderer::new(&device, format, egui_wgpu::RendererOptions::default());
    for (id, delta) in &frame.tex_set {
        renderer.update_texture(&device, &queue, *id, delta);
    }
    let screen_desc = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [opts.width, opts.height],
        pixels_per_point: ppp,
    };
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("redstring_shot_encoder"),
    });
    let mut command_buffers =
        renderer.update_buffers(&device, &queue, &mut encoder, &primitives, &screen_desc);
    {
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("redstring_shot_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(BOARD_CLEAR),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        let mut pass = pass.forget_lifetime();
        renderer.render(&mut pass, &primitives, &screen_desc);
    }
    for id in &frame.tex_free {
        renderer.free_texture(id);
    }

    let bytes_per_pixel = 4u32;
    let unpadded = opts.width * bytes_per_pixel;
    let padded = wgpu::util::align_to(unpadded, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("redstring_shot_readback"),
        size: (padded as u64) * (opts.height as u64),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: None,
            },
        },
        extent,
    );
    command_buffers.push(encoder.finish());
    queue.submit(command_buffers);

    let (tx, rx) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |res| {
        drop(tx.send(res));
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| Error::Render {
            what: format!("device poll: {e}"),
        })?;
    let received = rx.recv().map_err(|e| Error::Render {
        what: format!("readback channel closed: {e}"),
    })?;
    received.map_err(|e| Error::Render {
        what: format!("buffer map failed: {e}"),
    })?;

    let mut rgba: Vec<u8> = Vec::with_capacity((opts.width * opts.height * 4) as usize);
    {
        let mapped = readback.slice(..).get_mapped_range();
        for row in mapped.chunks(padded as usize) {
            rgba.extend_from_slice(&row[..unpadded as usize]);
        }
    }
    readback.unmap();

    let Some(img) = image::RgbaImage::from_raw(opts.width, opts.height, rgba) else {
        return Err(Error::Render {
            what: "readback did not fill the target image".to_owned(),
        });
    };
    img.save(&opts.out).map_err(|source| Error::Image {
        path: opts.out.clone(),
        source,
    })?;

    Ok(format!(
        "wgpu {:?} · {} · {:?}",
        info.backend, info.name, info.device_type
    ))
}
