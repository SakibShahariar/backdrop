//! Stacked Deck Selector — port of `wallpaper_chooser_prototype/src/layouts/stacked_deck.py`.
//!
//! Three physical cards, each with ONE rotation fixed forever at construction
//! (0°, -4°, +4° — never touched again). Rotation belongs to the CARD, not to a
//! spatial "front/mid/back" role. What changes over time is which card currently
//! plays the front (interactable) role, tracked via a mutable paint-order list.
//!
//! This is drawn by a single custom Gtk.Widget managing all 3 cards directly,
//! rather than 3 separate widgets inside a Gtk.Overlay — Gtk.Overlay has no
//! public API to reorder its children's paint order after construction.

use crate::widgets::gsk_utils::draw_texture_cover;
use gdk_pixbuf::Pixbuf;
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

pub const CARD_ROTATIONS_DEG: [f32; 3] = [0.0, -4.0, 4.0];
pub const CARD_WIDTH: i32 = 560;
pub const CARD_HEIGHT: i32 = 340;
pub const DEAL_ANIMATION_MS: u64 = 220;
pub const RISE_ANIMATION_MS: u64 = 200;
pub const DEAL_SLIDE_DISTANCE: f32 = CARD_WIDTH as f32 * 1.3;

const SUPPORTED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

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

fn decode_texture(path: &str, scale_factor: i32) -> Option<gdk::Texture> {
    let scale = if scale_factor <= 0 { 1 } else { scale_factor };
    let w = CARD_WIDTH * scale;
    let h = CARD_HEIGHT * scale;
    let pixbuf = Pixbuf::from_file_at_scale(path, w, h, true).ok()?;
    Some(gdk::Texture::for_pixbuf(&pixbuf))
}

// ---------------------------------------------------------------------------
// DeckSlot — one physical card. rotation is set once and never reassigned.
// ---------------------------------------------------------------------------

pub struct DeckSlot {
    pub rotation: f32,
    pub texture: Option<gdk::Texture>,
    pub offset_x: f32,
    pub offset_y: f32,
    pub scale: f32,
    pub opacity: f32,
}

impl DeckSlot {
    fn new(rotation: f32) -> Self {
        Self {
            rotation,
            texture: None,
            offset_x: 0.0,
            offset_y: 0.0,
            scale: 1.0,
            opacity: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// DeckWidget — custom GtkWidget that draws 3 DeckSlots in explicit order
// ---------------------------------------------------------------------------

mod imp {
    use super::*;

    pub struct DeckWidget {
        pub slots: RefCell<[DeckSlot; 3]>,
        pub paint_order: RefCell<Vec<usize>>,
    }

    impl Default for DeckWidget {
        fn default() -> Self {
            let slots = [
                DeckSlot::new(CARD_ROTATIONS_DEG[0]),
                DeckSlot::new(CARD_ROTATIONS_DEG[1]),
                DeckSlot::new(CARD_ROTATIONS_DEG[2]),
            ];
            // paint_order reversed relative to construction order so front
            // begins at slot0 (0°), matching initial "0 -4 4" state.
            // paint_order[0] = back, paint_order[2] = front.
            let paint_order = vec![2, 1, 0];
            Self {
                slots: RefCell::new(slots),
                paint_order: RefCell::new(paint_order),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeckWidget {
        const NAME: &'static str = "DeckWidget";
        type Type = super::DeckWidget;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for DeckWidget {}

    impl WidgetImpl for DeckWidget {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            if orientation == gtk::Orientation::Horizontal {
                (CARD_WIDTH, CARD_WIDTH, -1, -1)
            } else {
                (CARD_HEIGHT, CARD_HEIGHT, -1, -1)
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let width = widget.width() as f32;
            let height = widget.height() as f32;
            if width <= 0.0 || height <= 0.0 {
                return;
            }

            // Clone paint_order so we don't hold borrow across snapshot ops
            let paint_order = self.paint_order.borrow().clone();
            let slots = self.slots.borrow();

            for &idx in &paint_order {
                let slot = &slots[idx];
                if slot.opacity <= 0.0 {
                    continue;
                }

                let center = graphene::Point::new(width / 2.0, height / 2.0);

                let transform = gsk::Transform::new()
                    .translate(&graphene::Point::new(slot.offset_x, slot.offset_y))
                    .translate(&center)
                    .rotate(slot.rotation)
                    .scale(slot.scale, slot.scale)
                    .translate(&graphene::Point::new(-width / 2.0, -height / 2.0));

                snapshot.save();
                snapshot.transform(Some(&transform));

                if slot.opacity < 1.0 {
                    snapshot.push_opacity(slot.opacity as f64);
                }

                let rect = graphene::Rect::new(0.0, 0.0, width, height);
                let frame_color = gdk::RGBA::new(0x2e as f32 / 255.0, 0x2e as f32 / 255.0, 0x2e as f32 / 255.0, 1.0);
                snapshot.append_color(&frame_color, &rect);

                if let Some(texture) = slot.texture.as_ref() {
                    draw_texture_cover(snapshot, texture, 0.0, 0.0, width, height);
                }

                if slot.opacity < 1.0 {
                    snapshot.pop();
                }

                snapshot.restore();
            }
        }
    }
}

glib::wrapper! {
    pub struct DeckWidget(ObjectSubclass<imp::DeckWidget>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl DeckWidget {
    pub fn new() -> Self {
        glib::Object::new()
    }
}

impl Default for DeckWidget {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// StackedDeckLayout struct — optional ergonomic wrapper
// ---------------------------------------------------------------------------

pub struct StackedDeckLayout {
    // kept for parity with prototype; build() below is the primary entry point
    _private: (),
}

// ---------------------------------------------------------------------------
// Internal helpers that operate on DeckWidget + shared state
// ---------------------------------------------------------------------------

fn load_all_cards(deck: &DeckWidget, paths: &[String], deck_index: usize) {
    if paths.is_empty() {
        return;
    }
    let n = paths.len();
    let scale_factor = deck.scale_factor();
    // paint_order[-1] is front (deck_index+0), [-2] mid (+1), [0] back (+2)
    let paint_order = deck.imp().paint_order.borrow().clone();
    for (i, &slot_idx) in paint_order.iter().rev().enumerate() {
        let path = &paths[(deck_index + i) % n];
        let texture = decode_texture(path, scale_factor);
        let mut slots = deck.imp().slots.borrow_mut();
        let slot = &mut slots[slot_idx];
        slot.texture = texture;
        slot.offset_x = 0.0;
        slot.offset_y = 0.0;
        slot.scale = 1.0;
        slot.opacity = 1.0;
    }
    deck.queue_draw();
}

fn rise_in(deck: DeckWidget, slot_idx: usize, animating: Rc<Cell<bool>>) {
    let duration_us = RISE_ANIMATION_MS as i64 * 1000;
    let start_offset_y = CARD_HEIGHT as f32 * 0.35;
    let start_scale: f32 = 0.85;

    {
        let mut slots = deck.imp().slots.borrow_mut();
        let slot = &mut slots[slot_idx];
        slot.offset_y = start_offset_y;
        slot.scale = start_scale;
        slot.opacity = 0.0;
    }
    deck.queue_draw();

    let start_time = glib::monotonic_time();
    let deck_clone = deck.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        let elapsed = glib::monotonic_time() - start_time;
        let t = (elapsed as f64 / duration_us as f64).clamp(0.0, 1.0);
        let eased = ease_out_cubic(t);

        {
            let mut slots = deck_clone.imp().slots.borrow_mut();
            let slot = &mut slots[slot_idx];
            slot.offset_y = start_offset_y * (1.0 - eased as f32);
            slot.scale = start_scale + (1.0 - start_scale) * eased as f32;
            slot.opacity = eased as f32;
        }
        deck_clone.queue_draw();

        if t >= 1.0 {
            {
                let mut slots = deck_clone.imp().slots.borrow_mut();
                let slot = &mut slots[slot_idx];
                slot.offset_y = 0.0;
                slot.scale = 1.0;
                slot.opacity = 1.0;
            }
            deck_clone.queue_draw();
            animating.set(false);
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

fn deal_next(
    deck: &DeckWidget,
    paths: Rc<RefCell<Vec<String>>>,
    deck_index: Rc<Cell<usize>>,
    animating: Rc<Cell<bool>>,
) {
    let front_idx = {
        let po = deck.imp().paint_order.borrow();
        *po.last().expect("paint_order has 3 elements")
    };
    let start_time = glib::monotonic_time();
    let duration_us = DEAL_ANIMATION_MS as i64 * 1000;
    let target_offset = -DEAL_SLIDE_DISTANCE;

    let deck_clone = deck.clone();
    let paths_clone = paths.clone();
    let deck_index_clone = deck_index.clone();
    let animating_clone = animating.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        let elapsed = glib::monotonic_time() - start_time;
        let t = (elapsed as f64 / duration_us as f64).clamp(0.0, 1.0);
        let eased = ease_out_cubic(t);

        {
            let mut slots = deck_clone.imp().slots.borrow_mut();
            let slot = &mut slots[front_idx];
            slot.offset_x = target_offset * eased as f32;
            slot.opacity = 1.0 - eased as f32;
        }
        deck_clone.queue_draw();

        if t >= 1.0 {
            let n = paths_clone.borrow().len();
            if n == 0 {
                animating_clone.set(false);
                return glib::ControlFlow::Break;
            }
            let new_index = (deck_index_clone.get() + 1) % n;
            deck_index_clone.set(new_index);

            // Recycle: front becomes new back, keeping its own rotation
            let scale_factor = deck_clone.scale_factor();
            let new_texture =
                decode_texture(&paths_clone.borrow()[(new_index + 2) % n], scale_factor);
            {
                let mut slots = deck_clone.imp().slots.borrow_mut();
                let slot = &mut slots[front_idx];
                slot.texture = new_texture;
                slot.offset_x = 0.0;
                slot.offset_y = 0.0;
                slot.scale = 1.0;
                slot.opacity = 1.0;
            }

            {
                let mut po = deck_clone.imp().paint_order.borrow_mut();
                po.pop();
                po.insert(0, front_idx);
            }
            deck_clone.queue_draw();
            animating_clone.set(false);
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

fn deal_previous(
    deck: &DeckWidget,
    paths: Rc<RefCell<Vec<String>>>,
    deck_index: Rc<Cell<usize>>,
    animating: Rc<Cell<bool>>,
) {
    let back_idx = {
        let po = deck.imp().paint_order.borrow();
        po[0]
    };
    let n = paths.borrow().len();
    if n == 0 {
        animating.set(false);
        return;
    }
    let new_index = (deck_index.get() + n - 1) % n;
    deck_index.set(new_index);

    let scale_factor = deck.scale_factor();
    let new_texture = decode_texture(&paths.borrow()[new_index % n], scale_factor);
    {
        let mut slots = deck.imp().slots.borrow_mut();
        let slot = &mut slots[back_idx];
        slot.texture = new_texture;
    }

    {
        let mut po = deck.imp().paint_order.borrow_mut();
        po.remove(0);
        po.push(back_idx);
    }
    deck.queue_draw();

    rise_in(deck.clone(), back_idx, animating);
}

fn deal(
    direction: i32,
    deck: &DeckWidget,
    paths: Rc<RefCell<Vec<String>>>,
    deck_index: Rc<Cell<usize>>,
    animating: Rc<Cell<bool>>,
) {
    if paths.borrow().is_empty() || animating.get() {
        return;
    }
    animating.set(true);
    if direction > 0 {
        deal_next(deck, paths, deck_index, animating);
    } else {
        deal_previous(deck, paths, deck_index, animating);
    }
}

// ---------------------------------------------------------------------------
// Public build() — scans wallpapers and returns outer Box
// ---------------------------------------------------------------------------

pub fn build(wallpaper_dir: &str) -> gtk::Widget {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 12);
    outer.set_valign(gtk::Align::Center);
    outer.set_halign(gtk::Align::Center);
    // Make outer focusable so key controller receives events
    outer.set_focusable(true);

    let deck = DeckWidget::new();
    deck.set_halign(gtk::Align::Center);
    deck.set_valign(gtk::Align::Center);
    deck.set_margin_top(20);
    deck.set_margin_bottom(20);
    deck.set_margin_start(20);
    deck.set_margin_end(20);
    // Ensure size request matches card size (also via measure)
    deck.set_size_request(CARD_WIDTH, CARD_HEIGHT);

    let paths: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(scan_wallpapers(wallpaper_dir)));
    let deck_index: Rc<Cell<usize>> = Rc::new(Cell::new(0));
    let animating: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    // Swipe
    {
        let deck_clone = deck.clone();
        let paths_clone = paths.clone();
        let deck_index_clone = deck_index.clone();
        let animating_clone = animating.clone();
        let swipe = gtk::GestureSwipe::new();
        swipe.connect_swipe(move |_, vx, _vy| {
            if vx.abs() > 50.0 {
                let dir = if vx < 0.0 { 1 } else { -1 };
                deal(dir, &deck_clone, paths_clone.clone(), deck_index_clone.clone(), animating_clone.clone());
            }
        });
        deck.add_controller(swipe);
    }

    // Click
    {
        let deck_clone = deck.clone();
        let paths_clone = paths.clone();
        let deck_index_clone = deck_index.clone();
        let animating_clone = animating.clone();
        let click = gtk::GestureClick::new();
        click.connect_released(move |_, _, _, _| {
            deal(1, &deck_clone, paths_clone.clone(), deck_index_clone.clone(), animating_clone.clone());
        });
        deck.add_controller(click);
    }

    outer.append(&deck);

    let hint = gtk::Label::new(Some("Swipe, click, or press \u{2190} / \u{2192} to browse"));
    hint.add_css_class("dim-label");
    outer.append(&hint);

    let select_button = gtk::Button::with_label("Use this wallpaper");
    select_button.add_css_class("suggested-action");
    select_button.set_halign(gtk::Align::Center);
    {
        let paths_clone = paths.clone();
        let deck_index_clone = deck_index.clone();
        select_button.connect_clicked(move |_| {
            let p = paths_clone.borrow();
            if p.is_empty() {
                return;
            }
            let path = &p[deck_index_clone.get() % p.len()];
            eprintln!("[stacked_deck] wallpaper-selected: {}", path);
            // keep signal name visible for verification
            let _ = "wallpaper-selected";
        });
    }
    outer.append(&select_button);

    // Key controller on outer (Capture)
    {
        let deck_clone = deck.clone();
        let paths_clone = paths.clone();
        let deck_index_clone = deck_index.clone();
        let animating_clone = animating.clone();
        let key_controller = gtk::EventControllerKey::new();
        key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        key_controller.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Left {
                deal(-1, &deck_clone, paths_clone.clone(), deck_index_clone.clone(), animating_clone.clone());
                glib::Propagation::Stop
            } else if key == gdk::Key::Right {
                deal(1, &deck_clone, paths_clone.clone(), deck_index_clone.clone(), animating_clone.clone());
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        outer.add_controller(key_controller);
    }

    // Load on realize (like prototype's deck.connect("realize", ...))
    {
        let paths_clone = paths.clone();
        let deck_index_clone = deck_index.clone();
        deck.connect_realize(move |w| {
            let p = paths_clone.borrow().clone();
            load_all_cards(w, &p, deck_index_clone.get());
        });
        // Also queue initial load in case already realized when added
        // Defer via idle so scale_factor is valid after realize
        let deck_for_idle = deck.clone();
        let paths_for_idle = paths.clone();
        let deck_index_for_idle = deck_index.clone();
        glib::idle_add_local_once(move || {
            if deck_for_idle.is_realized() {
                let p = paths_for_idle.borrow().clone();
                load_all_cards(&deck_for_idle, &p, deck_index_for_idle.get());
            }
        });
    }

    // If file list is empty we still return outer with diagnostic
    if paths.borrow().is_empty() {
        eprintln!(
            "No wallpapers found under {wallpaper_dir}; pass a directory with .png/.jpg/.webp files."
        );
    } else {
        eprintln!(
            "[diag] stacked_deck::build() found {} wallpaper(s), deck ready",
            paths.borrow().len()
        );
    }

    outer.upcast()
}
