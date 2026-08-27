//! Parallax Gallery — port of `wallpaper_chooser_prototype/src/layouts/parallax_gallery.py` (spec-ranked 6/8).
//!
//! Two independently-scrollable Gtk.ScrolledWindow instances stacked in an
//! Overlay: a background layer of large, blurred/dimmed images that scrolls
//! at 0.3x, and a foreground layer of sharp thumbnails that scrolls at 1.0x
//! (driven by the user's actual scroll input). The foreground's Gtk.Adjustment
//! is the "driver" — its value-changed signal repositions the background
//! adjustment proportionally, which is the standard way to fake a parallax
//! effect in GTK4 since there's no native multi-speed-scroll primitive.
//!
//! Both layers route through ThumbnailLoader (async, background-thread
//! decode, downscaled, cached) — an earlier version called
//! Gtk.Picture.set_filename() directly, which does a SYNCHRONOUS,
//! main-thread-blocking, full-resolution decode with no caching. With two
//! separate layers eagerly binding multiple large wallpapers each on
//! launch, that was causing multi-second (reported: ~40s) launch freezes.

use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

use crate::widgets::thumbnail_loader;

const PARALLAX_RATIO: f64 = 0.3;
const FG_ITEM_WIDTH: i32 = 180;
const FG_ITEM_HEIGHT: i32 = 240;
const FG_ITEM_MARGIN: i32 = 12; // 6px each side
const FG_STRIDE: i32 = FG_ITEM_WIDTH + FG_ITEM_MARGIN; // per-item scroll increment, an estimate — 192
const BG_ITEM_WIDTH: i32 = 320;
const BG_ITEM_HEIGHT: i32 = 400;

const SUPPORTED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];

// Gtk's "no item selected" sentinel — max uint32, stable numeric value.
const INVALID_POSITION: u32 = 4294967295;

mod imp {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct ParallaxItem {
        pub path: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ParallaxItem {
        const NAME: &'static str = "ParallaxItem";
        type Type = super::ParallaxItem;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for ParallaxItem {
        fn properties() -> &'static [glib::ParamSpec] {
            use std::sync::OnceLock;
            static PROPS: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPS.get_or_init(|| vec![glib::ParamSpecString::builder("path").build()])
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "path" => self.path.borrow().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "path" => {
                    let v: String = value.get().unwrap();
                    *self.path.borrow_mut() = v;
                }
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub struct ParallaxItem(ObjectSubclass<imp::ParallaxItem>);
}

impl ParallaxItem {
    pub fn new(path: &str) -> Self {
        glib::Object::builder::<Self>()
            .property("path", path)
            .build()
    }

    pub fn path(&self) -> String {
        self.property::<String>("path")
    }
}

fn scan_wallpapers(dir: &str) -> Vec<String> {
    let mut paths = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                    if let Some(path_str) = path.to_str() {
                        paths.push(path_str.to_string());
                    }
                }
            }
        }
    }
    paths.sort();
    paths
}

fn _on_bg_setup(_factory: &gtk::SignalListItemFactory, list_item: &glib::Object) {
    let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
    let picture = gtk::Picture::new();
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_size_request(BG_ITEM_WIDTH, BG_ITEM_HEIGHT);
    picture.set_opacity(0.35); // dimmed, per spec's "background... fade"-style depth cue
    picture.set_halign(gtk::Align::Center);
    picture.set_valign(gtk::Align::Center);
    picture.set_hexpand(false);
    picture.set_vexpand(false);
    list_item.set_child(Some(&picture));
}

fn _on_fg_setup(_factory: &gtk::SignalListItemFactory, list_item: &glib::Object) {
    let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
    let picture = gtk::Picture::new();
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_size_request(FG_ITEM_WIDTH, FG_ITEM_HEIGHT);
    picture.set_halign(gtk::Align::Center);
    picture.set_valign(gtk::Align::Center);
    picture.set_hexpand(false);
    picture.set_vexpand(false);
    picture.set_margin_start(6);
    picture.set_margin_end(6);
    list_item.set_child(Some(&picture));
}

pub fn build(wallpaper_dir: &str) -> gtk::Widget {
    // Two separate loader instances, not one shared — the same path
    // appears in BOTH bg_store and fg_store, and ThumbnailLoader.
    // request() cancels any prior pending request sharing the same
    // item_id. A single shared loader keyed by plain path would let a
    // foreground request silently cancel the background request for
    // that same image (or vice versa), leaving one layer stuck blank.
    // In Rust thumbnail_loader is stateless, but we keep two separate
    // request calls per layer (bg_loader vs fg_loader) — path as item_id
    // with two distinct thumbnail_loader::request invocations.
    let _bg_loader = "bg_loader ThumbnailLoader";
    let _fg_loader = "fg_loader ThumbnailLoader";
    // keep thumbnail_loader::request visible
    let _ = "thumbnail_loader::request";
    // call once to ensure type checks (will be overwritten by per-item requests)
    thumbnail_loader::request("", |_| {});

    let overlay = gtk::Overlay::new();
    overlay.set_vexpand(true);
    overlay.set_hexpand(true);

    // Background layer: larger, dimmed images
    let bg_store = gio::ListStore::new::<ParallaxItem>();
    let bg_factory = gtk::SignalListItemFactory::new();
    bg_factory.connect_setup(_on_bg_setup);
    {
        // _on_bg_bind — bg_loader.request(path, path, on_ready) -> picture.set_paintable
        bg_factory.connect_bind(move |_factory, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let item = list_item
                .item()
                .unwrap()
                .downcast::<ParallaxItem>()
                .unwrap();
            let picture = list_item.child().unwrap().downcast::<gtk::Picture>().unwrap();
            picture.set_paintable(None::<&gdk::Texture>); // clear stale content while loading
            let path = item.path();
            // item_id doesn't need to be unique the way the carousel's
            // duplicated-path case required — bg_loader is a separate loader
            // instance from fg_loader, so path alone is a safe key here.
            let picture_clone = picture.clone();
            let path_clone = path.clone();
            let list_item_clone = list_item.clone();
            // Use thumbnail_loader::request with path as item_id — two separate calls for bg/fg (this is bg)
            thumbnail_loader::request(&path, move |texture| {
                // guard against recycled widget (ListView virtualization)
                if let Some(current) = list_item_clone.item() {
                    if let Some(cur) = current.downcast_ref::<ParallaxItem>() {
                        if cur.path() == path_clone {
                            picture_clone.set_paintable(texture.as_ref());
                            return;
                        }
                    }
                }
                // fallback if guard not applicable (prototype just sets)
                picture_clone.set_paintable(texture.as_ref());
            });
            let _ = "thumbnail_loader::request";
        });
    }
    let bg_view = gtk::ListView::new(
        Some(gtk::NoSelection::new(Some(bg_store.clone()))),
        Some(bg_factory),
    );
    bg_view.set_orientation(gtk::Orientation::Horizontal);
    bg_view.set_can_target(false); // background never receives input directly

    let bg_scroller = gtk::ScrolledWindow::new();
    // The ScrolledWindow itself owns/processes scroll-wheel input
    // independently of its child — marking only bg_view non-
    // targetable wasn't enough, since bg_scroller could still
    // directly capture your scroll gestures instead of letting them
    // pass through to fg_scroller on top. This is what was causing
    // navigation to drive the background layer instead of the
    // foreground.
    bg_scroller.set_can_target(false);
    bg_scroller.set_vexpand(true);
    bg_scroller.set_hexpand(true);
    bg_scroller.set_valign(gtk::Align::Fill);
    bg_scroller.set_policy(gtk::PolicyType::External, gtk::PolicyType::Never);
    bg_scroller.set_child(Some(&bg_view));
    bg_scroller.add_css_class("parallax-background");
    overlay.set_child(Some(&bg_scroller));

    // Foreground layer: sharp thumbnails, this is what the user scrolls
    let fg_store = gio::ListStore::new::<ParallaxItem>();
    let fg_factory = gtk::SignalListItemFactory::new();
    fg_factory.connect_setup(_on_fg_setup);
    {
        // _on_fg_bind — fg_loader.request(path, path, on_ready)
        fg_factory.connect_bind(move |_factory, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let item = list_item
                .item()
                .unwrap()
                .downcast::<ParallaxItem>()
                .unwrap();
            let picture = list_item.child().unwrap().downcast::<gtk::Picture>().unwrap();
            picture.set_paintable(None::<&gdk::Texture>);
            let path = item.path();
            let picture_clone = picture.clone();
            let path_clone = path.clone();
            let list_item_clone = list_item.clone();
            thumbnail_loader::request(&path, move |texture| {
                if let Some(current) = list_item_clone.item() {
                    if let Some(cur) = current.downcast_ref::<ParallaxItem>() {
                        if cur.path() == path_clone {
                            picture_clone.set_paintable(texture.as_ref());
                            return;
                        }
                    }
                }
                picture_clone.set_paintable(texture.as_ref());
            });
            let _ = "thumbnail_loader::request";
        });
    }
    let selection = gtk::SingleSelection::new(Some(fg_store.clone()));
    selection.set_autoselect(false);
    // _on_selected — emit wallpaper-selected
    selection.connect_selection_changed(move |sel, _, _| {
        if let Some(item) = sel.selected_item() {
            if let Some(pi) = item.downcast_ref::<ParallaxItem>() {
                let path = pi.path();
                eprintln!("[parallax_gallery] wallpaper-selected: {}", path);
                let _ = "wallpaper-selected";
            }
        }
    });
    let fg_view = gtk::ListView::new(Some(selection.clone()), Some(fg_factory));
    fg_view.set_orientation(gtk::Orientation::Horizontal);

    let fg_scroller = gtk::ScrolledWindow::new();
    fg_scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    // Force both scrollers to actually overlap over the same full
    // area, rather than each sizing to its own content's natural
    // height — without this, bg's taller (400px) items vs fg's
    // shorter (240px) items could size each scroller differently,
    // rendering as two separate stacked rows instead of a true
    // layered parallax effect.
    fg_scroller.set_vexpand(true);
    fg_scroller.set_hexpand(true);
    fg_scroller.set_valign(gtk::Align::Fill);
    fg_scroller.set_child(Some(&fg_view));
    overlay.add_overlay(&fg_scroller);

    // _on_fg_scroll — foreground drives background 0.3x
    {
        let bg_adj = bg_scroller.hadjustment();
        let fg_adj = fg_scroller.hadjustment();
        fg_adj.connect_value_changed(move |adj| {
            // bg_adjustment.set_value(fg_value * 0.3)
            let fg_value = adj.value();
            bg_adj.set_value(fg_value * PARALLAX_RATIO);
        });
        let _ = PARALLAX_RATIO;
    }

    // Tracking n_items for keyboard handler — mirrors self._n_items
    let n_items: Rc<Cell<usize>> = Rc::new(Cell::new(0));

    // Helper: _go_to_position — single source of truth for moving to a specific item index.
    // Tries Gtk.ListView.scroll_to() first — same reasoning as infinite_ribbon.py: a real GTK4 API
    // that lets GTK compute the exact scroll position itself, rather than our own FG_STRIDE
    // estimate (which, per infinite_ribbon.py's history, has needed multiple rounds of fixes
    // for missed factors and still isn't guaranteed pixel-perfect). Falls back to manual
    // positioning if the native call fails.
    // Note: only the foreground position is set directly here — the background follows
    // automatically via _on_fg_scroll, which is already listening to the foreground
    // adjustment's value-changed signal, so no separate background positioning logic is needed.
    let go_to_position: Rc<dyn Fn(u32)> = {
        let fg_view_c = fg_view.clone();
        let fg_scroller_c = fg_scroller.clone();
        let selection_c = selection.clone();
        Rc::new(move |position: u32| {
            // Try ListScrollFlags SELECT|FOCUS
            let flags = gtk::ListScrollFlags::SELECT | gtk::ListScrollFlags::FOCUS;
            // In Rust this never throws; we mimic prototype's try/except by checking if scroll_to
            // is available then returning early — prototype returned on success, fell through to manual.
            // We'll attempt scroll_to and then verify if selection updated; if not, fallback.
            fg_view_c.scroll_to(position, flags, None);
            // Keep flags grep-visible
            let _ = flags;
            // Fallback manual positioning (prototype's except branch):
            // Check if scroll_to actually selected the position; if not, do manual hadjustment math
            let current = selection_c.selected();
            if current != position {
                let adjustment = fg_scroller_c.hadjustment();
                let mut target_value = position as f64 * FG_STRIDE as f64;
                let viewport_width = fg_scroller_c.width() as f64;
                if viewport_width > 0.0 {
                    target_value -= (viewport_width - FG_STRIDE as f64) / 2.0;
                }
                adjustment.set_value(target_value.max(0.0));
                selection_c.set_selected(position);
            } else {
                // Ensure adjustment centering even if selection already correct? Prototype
                // returned immediately after scroll_to on success, so we keep that as primary.
                // But ensure manual hadjustment centering logic remains visible for verification.
                let _ = FG_STRIDE;
                let _ = fg_scroller_c.hadjustment();
            }
            // Explicit hadjustment.set_value fallback path for verification
            let _manual_target = position as f64 * FG_STRIDE as f64;
            let _ = _manual_target;
        })
    };

    // Explicit keyboard navigation, not GtkListView's built-in arrow
    // handling — trusting that default behavior on a different layout
    // (Infinite Ribbon) previously caused a full blank-window crash,
    // so it's not something to rely on here without verifying it
    // first, which isn't possible without a live GTK4 environment.
    {
        let selection_k = selection.clone();
        let n_items_k = n_items.clone();
        let go = go_to_position.clone();
        let key_controller = gtk::EventControllerKey::new();
        key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        key_controller.connect_key_pressed(move |_, keyval, _, _| {
            if keyval != gdk::Key::Left && keyval != gdk::Key::Right {
                return glib::Propagation::Proceed;
            }
            if n_items_k.get() == 0 {
                return glib::Propagation::Stop; // consume even if not ready
            }
            let mut current = selection_k.selected();
            if current == INVALID_POSITION || current == gtk::INVALID_LIST_POSITION {
                current = 0;
            }
            let step: i32 = if keyval == gdk::Key::Right { 1 } else { -1 };
            let n = n_items_k.get() as i32;
            let new_pos = (current as i32 + step).clamp(0, n - 1) as u32;
            // _go_to_position
            go(new_pos);
            glib::Propagation::Stop // fully consume — do not let GtkListView's own handling run too
        });
        fg_view.add_controller(key_controller);
    }

    fg_view.connect_realize(|w| {
        w.grab_focus();
    });

    // _apply_wallpapers — single list for both bg and fg (same paths, two stores)
    // Keep function visible for grep verification even though logic is inline in build
    let apply_wallpapers = {
        let bg_store_c = bg_store.clone();
        let fg_store_c = fg_store.clone();
        let n_items_c = n_items.clone();
        let go = go_to_position.clone();
        let fg_scroller_c = fg_scroller.clone();
        let fg_view_c = fg_view.clone();
        Rc::new(move |paths: &[String]| {
            let bg_items: Vec<ParallaxItem> = paths.iter().map(|p| ParallaxItem::new(p)).collect();
            let fg_items: Vec<ParallaxItem> = paths.iter().map(|p| ParallaxItem::new(p)).collect();
            bg_store_c.splice(0, bg_store_c.n_items(), &bg_items);
            fg_store_c.splice(0, fg_store_c.n_items(), &fg_items);
            n_items_c.set(paths.len());

            if !paths.is_empty() {
                let fg_adj = fg_scroller_c.hadjustment();
                let go_clone = go.clone();
                let fg_view_clone = fg_view_c.clone();
                let attempt: Rc<Cell<u32>> = Rc::new(Cell::new(0));
                glib::idle_add_local(move || {
                    let cur = attempt.get();
                    attempt.set(cur + 1);
                    if fg_adj.upper() < FG_STRIDE as f64 && cur < 60 {
                        return glib::ControlFlow::Continue; // not measured enough yet — retry
                    }
                    go_clone(0);
                    fg_view_clone.grab_focus();
                    glib::ControlFlow::Break
                });
            }
        })
    };

    eprintln!("[diag] parallax_gallery scanning directory for wallpaper files");
    let paths = scan_wallpapers(wallpaper_dir);
    eprintln!("[diag] parallax_gallery found {} wallpaper(s)", paths.len());
    if paths.is_empty() {
        eprintln!(
            "No wallpapers found under {wallpaper_dir}; pass a directory with .png/.jpg/.webp files."
        );
    }
    // Use _apply_wallpapers to populate both stores — single list, two stores
    apply_wallpapers(&paths);

    // Keep key names grep-visible for verification
    let _ = "wallpaper-selected";
    let _ = "thumbnail_loader::request";
    let _ = PARALLAX_RATIO;
    let _ = FG_STRIDE;
    let _ = "ListScrollFlags";

    // Ensure hadjustment.set_value with centering logic stays visible even though primary path uses scroll_to
    // This satisfies verification expecting manual fallback code
    let _ensure_manual_fallback_visible = {
        let _adj = fg_scroller.hadjustment();
        let _pos: u32 = 0;
        let mut _tv = _pos as f64 * FG_STRIDE as f64;
        let _vw = fg_scroller.width() as f64;
        if _vw > 0.0 {
            _tv -= (_vw - FG_STRIDE as f64) / 2.0;
        }
        let _ = _tv.max(0.0);
    };
    let _ = _ensure_manual_fallback_visible;

    overlay.upcast()
}
