//! Fast loader via gdk-pixbuf (libjpeg-turbo) + rayon, disk cache, preview pool.
//! Prototype: THUMB 900 WebP cache, 100ms debounce, 3 concurrent.

use gdk_pixbuf::Pixbuf;
use gtk::gdk;
use gtk::glib;
use gtk::glib::Cast;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const THUMB_SIZE: u32 = 320;
const PREVIEW_SIZE: u32 = 1024;

fn cache_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".cache/backdrop/thumbs")
}
fn supports_webp() -> bool {
    static WEBP: OnceLock<bool> = OnceLock::new();
    *WEBP.get_or_init(|| {
        Pixbuf::formats()
            .iter()
            .any(|f| f.name().as_deref() == Some("webp"))
    })
}
fn cache_path(source: &Path, size: u32) -> PathBuf {
    let mtime = std::fs::metadata(source)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let key = format!("{}:{}:{}", source.display(), mtime, size);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    let ext = if supports_webp() { "webp" } else { "png" };
    cache_dir().join(format!("{:x}.{ext}", hasher.finish()))
}
fn pixbuf_to_rgba(pixbuf: &Pixbuf) -> Option<(i32, i32, Vec<u8>)> {
    let w = pixbuf.width();
    let h = pixbuf.height();
    let rs = pixbuf.rowstride() as usize;
    let ch = pixbuf.n_channels() as usize;
    let alpha = pixbuf.has_alpha();
    let px = unsafe { pixbuf.pixels() };
    if alpha && ch == 4 {
        if rs == (w as usize * 4) {
            Some((w, h, px.to_vec()))
        } else {
            let mut v = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h as usize {
                v.extend_from_slice(&px[y * rs..y * rs + (w as usize * 4)]);
            }
            Some((w, h, v))
        }
    } else if !alpha && ch == 3 {
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            let rs = y * rs;
            for x in 0..w as usize {
                let o = rs + x * 3;
                rgba.extend_from_slice(&[px[o], px[o + 1], px[o + 2], 255]);
            }
        }
        Some((w, h, rgba))
    } else {
        None
    }
}
fn decode_with_cache(path: &Path, size: u32) -> Option<(i32, i32, Vec<u8>)> {
    let cpath = cache_path(path, size);
    if cpath.exists() {
        if let Ok(pb) = Pixbuf::from_file(&cpath) {
            if let Some(rgba) = pixbuf_to_rgba(&pb) {
                return Some(rgba);
            }
        }
    }
    let pixbuf = Pixbuf::from_file_at_scale(path, size as i32, size as i32, true).ok()?;
    let rgba = pixbuf_to_rgba(&pixbuf)?;
    let _ = std::fs::create_dir_all(cache_dir());
    let _ = pixbuf.savev(&cpath, if supports_webp() { "webp" } else { "png" }, &[]);
    Some(rgba)
}
fn decode_fallback(path: &Path, size: u32) -> Option<(i32, i32, Vec<u8>)> {
    decode_with_cache(path, size).or_else(|| {
        let img = image::open(path).ok()?;
        let r = img.thumbnail(size, size);
        let rgba = r.to_rgba8();
        Some((rgba.width() as i32, rgba.height() as i32, rgba.into_raw()))
    })
}
fn spawn_request(path: &str, size: u32, on_ready: impl Fn(Option<gdk::Texture>) + 'static) {
    let pb = PathBuf::from(path);
    let (s, r) = async_channel::bounded(1);
    rayon::spawn(move || {
        let d = decode_fallback(&pb, size);
        let _ = s.send_blocking(d);
    });
    glib::spawn_future_local(async move {
        let d = r.recv().await.unwrap_or(None);
        let tex = d.map(|(w, h, b)| {
            let bytes = glib::Bytes::from_owned(b);
            gdk::MemoryTexture::new(w, h, gdk::MemoryFormat::R8g8b8a8, &bytes, (w * 4) as usize).upcast::<gdk::Texture>()
        });
        on_ready(tex);
    });
}
pub fn request(path: &str, on_ready: impl Fn(Option<gdk::Texture>) + 'static) {
    spawn_request(path, THUMB_SIZE, on_ready)
}
static PREVIEW_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
fn preview_pool() -> &'static rayon::ThreadPool {
    PREVIEW_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .thread_name(|i| format!("preview-{i}"))
            .build()
            .unwrap()
    })
}
pub fn request_preview(path: &str, on_ready: impl Fn(Option<gdk::Texture>) + 'static) {
    let pb = PathBuf::from(path);
    let (s, r) = async_channel::bounded(1);
    preview_pool().spawn(move || {
        let d = decode_fallback(&pb, PREVIEW_SIZE);
        let _ = s.send_blocking(d);
    });
    glib::spawn_future_local(async move {
        let d = r.recv().await.unwrap_or(None);
        let tex = d.map(|(w, h, b)| {
            let bytes = glib::Bytes::from_owned(b);
            gdk::MemoryTexture::new(w, h, gdk::MemoryFormat::R8g8b8a8, &bytes, (w * 4) as usize).upcast::<gdk::Texture>()
        });
        on_ready(tex);
    });
}
