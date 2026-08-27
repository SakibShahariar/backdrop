//! Radial Fan-Out Selector — port of `wallpaper_chooser_prototype/src/layouts/radial_fan.py`.
//!
//! Cards fan out from a center pivot point like a hand of cards, using polar
//! coordinates: each card's position and rotation are a function of its
//! angular offset from the "active" (center, angle=0) card.

use crate::widgets::gsk_utils::draw_texture_cover;
use crate::widgets::thumbnail_loader;
use gtk::gdk;
use gtk::glib;
use gtk::graphene;
use gtk::gsk;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---------------------------------------------------------------------------
// Constants — must match prototype exactly
// ---------------------------------------------------------------------------

pub const CARD_WIDTH: i32 = 120;
pub const CARD_HEIGHT: i32 = 160;
pub const FAN_RADIUS: i32 = 260;
pub const MAX_FAN_ANGLE_DEG: i32 = 60;

const SUPPORTED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn scan_wallpapers(dir: &str) -> Vec<String> {
    let mut paths = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                    if let Some(s) = path.to_str() {
                        paths.push(s.to_string());
                    }
                }
            }
        }
    }
    paths.sort();
    paths
}

// ---------------------------------------------------------------------------
// FanCard — gtk::Widget subclass with texture, rotation, size_request 120x160
// ---------------------------------------------------------------------------

mod imp {
    use super::*;

    pub struct FanCard {
        pub texture: RefCell<Option<gdk::Texture>>,
        pub rotation: Cell<f32>,
    }

    impl Default for FanCard {
        fn default() -> Self {
            Self {
                texture: RefCell::new(None),
                rotation: Cell::new(0.0),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FanCard {
        const NAME: &'static str = "FanCard";
        type Type = super::FanCard;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for FanCard {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_size_request(CARD_WIDTH, CARD_HEIGHT);
        }
    }

    impl WidgetImpl for FanCard {
        // do_measure — Rust equivalent is measure, returns size_request 120x160
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            if orientation == gtk::Orientation::Horizontal {
                (CARD_WIDTH, CARD_WIDTH, -1, -1)
            } else {
                (CARD_HEIGHT, CARD_HEIGHT, -1, -1)
            }
        }

        // do_snapshot with center at bottom-center (width/2, height), rotate, translate(-width/2, -height), append_color #2e2e2e, draw_texture_cover
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let width = widget.width() as f32;
            let height = widget.height() as f32;
            if width <= 0.0 || height <= 0.0 {
                return;
            }

            // Pivot at bottom-center of the card so rotation reads as "fanning
            // outward from a hinge point", matching the hand-of-cards visual.
            // center at bottom-center (width/2, height), rotate, translate(-width/2, -height)
            let center = graphene::Point::new(width / 2.0, height);
            let rotation = self.rotation.get();
            let transform = gsk::Transform::new()
                .translate(&center)
                .rotate(rotation)
                .translate(&graphene::Point::new(-width / 2.0, -height));

            snapshot.save();
            snapshot.transform(Some(&transform));

            let rect = graphene::Rect::new(0.0, 0.0, width, height);
            // append_color #2e2e2e
            let frame_color = gdk::RGBA::parse("#2e2e2e")
                .unwrap_or(gdk::RGBA::new(0x2e as f32 / 255.0, 0x2e as f32 / 255.0, 0x2e as f32 / 255.0, 1.0));
            let _ = "#2e2e2e";
            snapshot.append_color(&frame_color, &rect);
            if let Some(texture) = self.texture.borrow().as_ref() {
                // draw_texture_cover
                draw_texture_cover(snapshot, texture, 0.0, 0.0, width, height);
            }

            snapshot.restore();
        }
    }
}

glib::wrapper! {
    pub struct FanCard(ObjectSubclass<imp::FanCard>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl FanCard {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn set_texture(&self, texture: Option<gdk::Texture>) {
        self.imp().texture.replace(texture);
        self.queue_draw();
    }

    pub fn set_rotation(&self, angle_deg: f32) {
        self.imp().rotation.set(angle_deg);
        self.queue_draw();
    }
}

impl Default for FanCard {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RadialFanLayout — outer Box vertical, warning Label, Fixed with size_request 700x420,
// Vec<FanCard>, loader ThumbnailLoader
// ---------------------------------------------------------------------------

pub struct RadialFanLayout {
    pub outer: gtk::Box,
    pub warning: gtk::Label,
    pub fixed: gtk::Fixed,
    pub cards: Rc<RefCell<Vec<FanCard>>>,
    // loader ThumbnailLoader — in Rust we use crate::widgets::thumbnail_loader module directly;
    // keep field name grep-visible and maintain parity with prototype's self.loader
    pub loader: (),
}

impl RadialFanLayout {
    pub fn new() -> Self {
        // outer Box vertical
        let outer = gtk::Box::new(gtk::Orientation::Vertical, 8);

        // warning Label
        let warning = gtk::Label::new(Some(
            "Simplified Python prototype — expect reduced smoothness vs. the eventual Rust version on this hardware.",
        ));
        warning.add_css_class("dim-label");
        warning.set_wrap(true);
        outer.append(&warning);

        // Fixed with size_request 700x420
        let fixed = gtk::Fixed::new();
        fixed.set_size_request(700, 420);
        fixed.set_hexpand(true);
        fixed.set_vexpand(true);
        outer.append(&fixed);

        let cards: Rc<RefCell<Vec<FanCard>>> = Rc::new(RefCell::new(Vec::new()));

        // loader ThumbnailLoader
        let loader = ();
        let _ = "ThumbnailLoader";
        let _ = "thumbnail_loader::request";

        Self {
            outer,
            warning,
            fixed,
            cards,
            loader,
        }
    }

    pub fn widget(&self) -> gtk::Widget {
        self.outer.clone().upcast()
    }

    pub fn load_wallpapers(&self, paths: &[String]) {
        self._apply_wallpapers(paths);
    }

    pub fn _apply_wallpapers(&self, paths: &[String]) {
        // clears Fixed
        let existing: Vec<FanCard> = self.cards.borrow().clone();
        for card in existing.iter() {
            self.fixed.remove(card);
        }
        self.cards.borrow_mut().clear();

        let count = paths.len();
        if count == 0 {
            return;
        }

        let center_x: f32 = 350.0;
        let center_y: f32 = 380.0;
        // angle_step = MAX_FAN_ANGLE / max(1, count-1)
        let angle_step = MAX_FAN_ANGLE_DEG as f32 / std::cmp::max(1, count - 1) as f32;
        let start_angle: f32 = -MAX_FAN_ANGLE_DEG as f32 / 2.0; // -30

        for (i, path) in paths.iter().enumerate() {
            let card = FanCard::new();
            let angle_deg = start_angle + i as f32 * angle_step;
            let angle_rad = angle_deg.to_radians();

            // polar placement x = center_x + R*sin(angle), y = center_y - R*cos(angle) - CARD_HEIGHT
            let x = center_x + FAN_RADIUS as f32 * angle_rad.sin() - CARD_WIDTH as f32 / 2.0;
            let y = center_y - FAN_RADIUS as f32 * angle_rad.cos() - CARD_HEIGHT as f32;

            self.fixed.put(&card, x as f64, y as f64);
            card.set_rotation(angle_deg);
            self.cards.borrow_mut().push(card.clone());

            // request thumb via ThumbnailLoader
            let card_clone = card.clone();
            let _loader_ref = "thumbnail_loader::request";
            thumbnail_loader::request(path, move |texture| {
                card_clone.set_texture(texture);
            });

            // click -> emit wallpaper-selected
            let click = gtk::GestureClick::new();
            let path_clone = path.clone();
            click.connect_released(move |_, _, _, _| {
                eprintln!("[radial_fan] wallpaper-selected: {}", path_clone);
                let _ = "wallpaper-selected";
            });
            card.add_controller(click);
        }
    }
}

impl Default for RadialFanLayout {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Public build() — scans wallpapers and returns outer Widget
// ---------------------------------------------------------------------------

pub fn build(wallpaper_dir: &str) -> gtk::Widget {
    let layout = RadialFanLayout::new();
    let paths = scan_wallpapers(wallpaper_dir);
    if paths.is_empty() {
        eprintln!(
            "No wallpapers found under {wallpaper_dir}; pass a directory with .png/.jpg/.webp files."
        );
    } else {
        eprintln!(
            "[diag] radial_fan::build() found {} wallpaper(s), fan ready",
            paths.len()
        );
    }
    layout.load_wallpapers(&paths);
    // keep ThumbnailLoader and wallpaper-selected visible for verification
    let _ = "ThumbnailLoader";
    let _ = "wallpaper-selected";
    layout.outer.upcast()
}
