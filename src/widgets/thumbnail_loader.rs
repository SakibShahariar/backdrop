//! Async thumbnail loader — decodes images on a background std::thread,
//! delivering the result back to the GTK main thread via
//! glib::idle_add_local. Port of the Python prototype's widgets/
//! thumbnail_loader.py, minus the debounce/cancel machinery (not needed
//! yet for this non-virtualized first pass).
//!
//! Uses the `image` crate for decoding, not GdkPixbuf — GObject types
//! like Pixbuf aren't Send/Sync and can't cross a thread boundary at
//! all, which the compiler caught immediately on the first attempt at
//! this. Decoding into plain RGBA bytes on the background thread (which
//! ARE Send-safe) and only constructing the GTK-side Gdk::MemoryTexture
//! on the main thread avoids the issue entirely — this is also what the
//! original design spec recommended (the `image` crate) rather than
//! GdkPixbuf for exactly this kind of async decode pipeline.

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use std::path::PathBuf;

const THUMB_SIZE: u32 = 480;

struct DecodedImage {
    width: i32,
    height: i32,
    rgba_bytes: Vec<u8>,
}

/// Decodes `path` on a background thread, then calls `on_ready` on the
/// GTK main thread once done. Fire-and-forget — no cancellation yet.
pub fn request(path: &str, on_ready: impl Fn(Option<gdk::Texture>) + 'static) {
    let path_buf = PathBuf::from(path);
    let (sender, receiver) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let result = image::open(&path_buf).ok().map(|img| {
            let resized = img.thumbnail(THUMB_SIZE, THUMB_SIZE);
            let rgba = resized.to_rgba8();
            DecodedImage {
                width: rgba.width() as i32,
                height: rgba.height() as i32,
                rgba_bytes: rgba.into_raw(),
            }
        });
        let _ = sender.send(result);
    });

    glib::source::idle_add_local(move || {
        match receiver.try_recv() {
            Ok(decoded_opt) => {
                let texture = decoded_opt.map(|decoded| {
                    let bytes = glib::Bytes::from_owned(decoded.rgba_bytes);
                    let stride = decoded.width * 4; // RGBA8 = 4 bytes/pixel
                    gdk::MemoryTexture::new(
                        decoded.width,
                        decoded.height,
                        gdk::MemoryFormat::R8g8b8a8,
                        &bytes,
                        stride as usize,
                    )
                    .upcast()
                });
                on_ready(texture);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                on_ready(None);
                glib::ControlFlow::Break
            }
        }
    });
}
