//! Shared helper for drawing a texture into a target rectangle using
//! "cover" fit semantics (like CSS background-size: cover) — scale to
//! fill the target completely while preserving aspect ratio, cropping
//! any overflow, centered.
//!
//! Port of the Python prototype's widgets/gsk_utils.py — same reasoning
//! applies here: anywhere a texture is drawn manually via a snapshot
//! rather than through Gtk::Picture (which handles content-fit itself)
//! needs this, or images get stretched to fill the target rect exactly,
//! distorting any image whose aspect ratio doesn't match the widget's.

use gtk::gdk;
use gtk::graphene;
use gtk::prelude::*;

pub fn draw_texture_cover(
    snapshot: &gtk::Snapshot,
    texture: &gdk::Texture,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }

    let tex_w = texture.width() as f32;
    let tex_h = texture.height() as f32;
    if tex_w <= 0.0 || tex_h <= 0.0 {
        return;
    }

    // "Cover" scale: the larger of the two ratios, so the texture
    // overflows the target rect on one axis rather than leaving gaps.
    let scale = (width / tex_w).max(height / tex_h);
    let draw_w = tex_w * scale;
    let draw_h = tex_h * scale;
    let offset_x = x + (width - draw_w) / 2.0;
    let offset_y = y + (height - draw_h) / 2.0;

    let clip_rect = graphene::Rect::new(x, y, width, height);
    snapshot.push_clip(&clip_rect);

    let tex_rect = graphene::Rect::new(offset_x, offset_y, draw_w, draw_h);
    snapshot.append_texture(texture, &tex_rect);

    snapshot.pop();
}
