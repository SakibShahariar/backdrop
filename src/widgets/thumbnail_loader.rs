//! Async thumbnail loader — fast decode via gdk-pixbuf (libjpeg-turbo) +
//! rayon pool, fallback to image crate. Two sizes: THUMB 320 for cards,
//! PREVIEW 1920 for preview.

use gdk_pixbuf::Pixbuf;
use gtk::gdk;
use gtk::glib;
use gtk::glib::Cast;
use std::path::PathBuf;

const THUMB_SIZE: u32 = 320;
const PREVIEW_SIZE: u32 = 1920;

fn pixbuf_to_rgba(pixbuf: &Pixbuf) -> Option<(i32, i32, Vec<u8>)> {
    let width = pixbuf.width();
    let height = pixbuf.height();
    let rowstride = pixbuf.rowstride() as usize;
    let n_channels = pixbuf.n_channels() as usize;
    let has_alpha = pixbuf.has_alpha();
    let pixels = unsafe { pixbuf.pixels() };

    if has_alpha && n_channels == 4 {
        // Already RGBA, but rowstride may be padded; repack tightly if needed
        if rowstride == (width as usize * 4) {
            Some((width, height, pixels.to_vec()))
        } else {
            let mut tight = Vec::with_capacity((width * height * 4) as usize);
            for y in 0..height as usize {
                let start = y * rowstride;
                let end = start + (width as usize * 4);
                tight.extend_from_slice(&pixels[start..end]);
            }
            Some((width, height, tight))
        }
    } else if !has_alpha && n_channels == 3 {
        // RGB -> RGBA (add opaque alpha)
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height as usize {
            let row_start = y * rowstride;
            for x in 0..width as usize {
                let off = row_start + x * 3;
                rgba.push(pixels[off]);
                rgba.push(pixels[off + 1]);
                rgba.push(pixels[off + 2]);
                rgba.push(255);
            }
        }
        Some((width, height, rgba))
    } else {
        None
    }
}

fn decode_with_pixbuf(path: &PathBuf, size: u32) -> Option<(i32, i32, Vec<u8>)> {
    // from_file_at_scale with preserve_aspect_ratio=true, scale to fit size×size
    let pixbuf = Pixbuf::from_file_at_scale(path, size as i32, size as i32, true).ok()?;
    pixbuf_to_rgba(&pixbuf)
}

fn decode_with_image(path: &PathBuf, size: u32) -> Option<(i32, i32, Vec<u8>)> {
    let img = image::open(path).ok()?;
    let resized = img.thumbnail(size, size);
    let rgba = resized.to_rgba8();
    Some((rgba.width() as i32, rgba.height() as i32, rgba.into_raw()))
}

fn decode_and_send(path: PathBuf, size: u32, sender: async_channel::Sender<Option<(i32, i32, Vec<u8>)>>) {
    // Try pixbuf first (fast, libjpeg-turbo), fallback to image crate
    let decoded = decode_with_pixbuf(&path, size).or_else(|| decode_with_image(&path, size));
    let _ = sender.send_blocking(decoded);
}

fn spawn_request(path: &str, size: u32, on_ready: impl Fn(Option<gdk::Texture>) + 'static) {
    let path_buf = PathBuf::from(path);
    let (sender, receiver) = async_channel::bounded(1);
    rayon::spawn(move || decode_and_send(path_buf, size, sender));
    glib::spawn_future_local(async move {
        let decoded_opt = receiver.recv().await.unwrap_or(None);
        let texture = decoded_opt.map(|(width, height, rgba_bytes)| {
            let bytes = glib::Bytes::from_owned(rgba_bytes);
            let stride = width * 4;
            gdk::MemoryTexture::new(
                width,
                height,
                gdk::MemoryFormat::R8g8b8a8,
                &bytes,
                stride as usize,
            )
            .upcast::<gdk::Texture>()
        });
        on_ready(texture);
    });
}

/// Thumbnail for cards (320px, fast)
pub fn request(path: &str, on_ready: impl Fn(Option<gdk::Texture>) + 'static) {
    spawn_request(path, THUMB_SIZE, on_ready)
}

/// High-res for preview (1920px)
pub fn request_preview(path: &str, on_ready: impl Fn(Option<gdk::Texture>) + 'static) {
    spawn_request(path, PREVIEW_SIZE, on_ready)
}
