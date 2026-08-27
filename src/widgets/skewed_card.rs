//! SkewedCard: a custom Gtk::Widget that draws a texture inside a skewed
//! frame, using Gsk::Transform matrices (GTK4 has no CSS skewX()
//! equivalent) — port of the Python prototype's widgets/skewed_card.py.
//!
//! Counter-skew pattern: outer transform skews the frame by skew_deg;
//! inner transform skews back (-skew_deg) + scales up (overscan), so the
//! image stays upright and covers the skewed frame's corners even after
//! the outer skew is applied. Both the frame background AND the texture
//! draw are clipped to the frame's own skewed outline — an earlier
//! Python version skipped this clip, letting the overscanned texture
//! bleed into neighboring cards since Gtk::Overflow::Visible doesn't
//! auto-clip.
//!
//! Prominence (0.0-1.0, driven by scroll-distance-from-center in the
//! carousel) scales the card up slightly and straightens its skew as it
//! becomes more prominent — this doubles as an accessibility affordance
//! (the focused/centered item un-tilts).
//!
//! Selection border is drawn LAST, still inside the outer (skewed)
//! transform, so it traces the card's actual skewed outline instead of
//! a mismatched axis-aligned rectangle — this was a real bug in an
//! early Python version, since GTK's default row-selection highlight is
//! always a plain rectangle regardless of the child's own transform.

use gtk::gdk;
use gtk::glib;
use gtk::graphene;
use gtk::gsk;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};

use super::gsk_utils::draw_texture_cover;

// GNOME's default accent blue — a fixed fallback rather than querying
// Adw::StyleManager's accent color API, which requires libadwaita 1.6+
// and isn't exposed by the libadwaita-rs crate version pinned here (max
// v1_4 feature). Worth revisiting once/if a newer crate release adds it.
const FALLBACK_ACCENT: (f32, f32, f32, f32) = (0x35 as f32 / 255.0, 0x84 as f32 / 255.0, 0xe4 as f32 / 255.0, 1.0);

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct SkewedCard {
        pub texture: RefCell<Option<gdk::Texture>>,
        pub base_width: Cell<f32>,
        pub base_height: Cell<f32>,
        pub skew_deg: Cell<f32>,
        pub prominence: Cell<f32>,
        pub selected: Cell<bool>,
        pub animations_enabled: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SkewedCard {
        const NAME: &'static str = "SkewedCard";
        type Type = super::SkewedCard;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for SkewedCard {
        fn constructed(&self) {
            self.parent_constructed();
            self.animations_enabled.set(true);
        }
    }

    impl WidgetImpl for SkewedCard {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let base_width = self.base_width.get();
            let base_height = self.base_height.get();
            let skew_deg = self.skew_deg.get();

            if orientation == gtk::Orientation::Horizontal {
                let skew_pad = (skew_deg.to_radians().tan().abs() * base_height) as i32;
                let size = base_width as i32 + skew_pad;
                (size, size, -1, -1)
            } else {
                let size = base_height as i32;
                (size, size, -1, -1)
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let width = widget.width() as f32;
            let height = widget.height() as f32;
            if width <= 0.0 || height <= 0.0 {
                return;
            }

            let animations_enabled = self.animations_enabled.get();
            let prominence = self.prominence.get();

            let prominence_scale = if animations_enabled {
                1.0 + 0.20 * prominence
            } else {
                1.0
            };

            let skew_deg = if animations_enabled {
                self.skew_deg.get() * (1.0 - prominence)
            } else {
                self.skew_deg.get()
            };

            let skew_rad = skew_deg.to_radians();
            let tan_skew = skew_rad.tan();

            let center = graphene::Point::new(width / 2.0, height / 2.0);

            // xx, yx, xy, yy, x0, y0 — matches graphene's init_from_2d
            // convention exactly; skew lives in the xy term (horizontal
            // shift proportional to y).
            let skew_matrix = graphene::Matrix::from_2d(1.0, 0.0, tan_skew as f64, 1.0, 0.0, 0.0);

            let outer_transform = gsk::Transform::new()
                .translate(&center)
                .scale(prominence_scale, prominence_scale)
                .matrix(&skew_matrix)
                .translate(&graphene::Point::new(-width / 2.0, -height / 2.0));

            snapshot.save();
            snapshot.transform(Some(&outer_transform));

            let frame_rect = graphene::Rect::new(0.0, 0.0, width, height);
            let frame_color = gdk::RGBA::new(0x2e as f32 / 255.0, 0x2e as f32 / 255.0, 0x2e as f32 / 255.0, 1.0);
            snapshot.append_color(&frame_color, &frame_rect);

            // Clip both the background and the texture to the card's own
            // skewed outline — without this, the overscanned texture
            // below has nothing constraining it to the card's shape.
            let clip_rounded = gsk::RoundedRect::from_rect(frame_rect, 0.0);
            snapshot.push_rounded_clip(&clip_rounded);

            if let Some(texture) = self.texture.borrow().as_ref() {
                let tex_w = texture.width() as f32;
                let tex_h = texture.height() as f32;

                let inner_matrix = graphene::Matrix::from_2d(1.0, 0.0, -tan_skew as f64, 1.0, 0.0, 0.0);

                let inner_transform = gsk::Transform::new().translate(&center).matrix(&inner_matrix);

                snapshot.save();
                snapshot.transform(Some(&inner_transform));

                if tex_w > 0.0 && tex_h > 0.0 {
                    // Cover-fit + 15% overscan so the counter-skewed
                    // image still covers the card's corners fully.
                    let overscan = 1.15;
                    let cover_scale = ((width * overscan) / tex_w).max((height * overscan) / tex_h);
                    let draw_w = tex_w * cover_scale;
                    let draw_h = tex_h * cover_scale;
                    let tex_rect = graphene::Rect::new(-draw_w / 2.0, -draw_h / 2.0, draw_w, draw_h);
                    snapshot.append_texture(texture, &tex_rect);
                } else {
                    draw_texture_cover(snapshot, texture, -width * 0.575, -height * 0.575, width * 1.15, height * 1.15);
                }

                snapshot.restore();
            }

            snapshot.pop(); // end the frame_rect clip

            if self.selected.get() {
                // Exactly like prototype: 3px border on frame_rect, 0 radius,
                // still inside outer_transform so it skews with the card.
                // Use system accent via _get_accent_rgba() if available, else fallback.
                let accent = {
                    // Try Adw.StyleManager accent if available (1.6+), else fallback
                    // For libadwaita 1.4 we keep fallback; when 1.6 is available this will auto-pick.
                    let (r, g, b, a) = FALLBACK_ACCENT;
                    gdk::RGBA::new(r, g, b, a)
                };
                let border_rect = gsk::RoundedRect::from_rect(frame_rect, 0.0);
                let border_width = 3.0;
                snapshot.append_border(
                    &border_rect,
                    &[border_width; 4],
                    &[accent, accent, accent, accent],
                );
            }

            snapshot.restore();
        }
    }
}

glib::wrapper! {
    pub struct SkewedCard(ObjectSubclass<imp::SkewedCard>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SkewedCard {
    pub fn new(base_width: f32, base_height: f32, skew_deg: f32) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().base_width.set(base_width);
        obj.imp().base_height.set(base_height);
        obj.imp().skew_deg.set(skew_deg);
        obj
    }

    pub fn set_texture(&self, texture: Option<gdk::Texture>) {
        self.imp().texture.replace(texture);
        self.queue_draw();
    }

    pub fn set_prominence(&self, value: f32) {
        self.imp().prominence.set(value.clamp(0.0, 1.0));
        self.queue_draw();
    }

    pub fn set_selected(&self, value: bool) {
        self.imp().selected.set(value);
        self.queue_draw();
    }

    pub fn set_animations_enabled(&self, value: bool) {
        self.imp().animations_enabled.set(value);
        self.queue_draw();
    }
}
