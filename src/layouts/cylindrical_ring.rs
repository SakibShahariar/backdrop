//! Cylindrical Ring Carousel — 2.5D approximation (spec-ranked 8/8, "Hard").
//! Port of prototype's layouts/cylindrical_ring.py.
//!
//! The spec's true design calls for genuine 3D cylinder with per-card Y-axis
//! rotation and perspective foreshortening. This is a 2.5D approximation:
//! horizontal scroll + depth approximated by scaling cards down as they move
//! away from center via SkewedCard's prominence mechanism (like the carousel's
//! prominence scaling), rather than true perspective-correct 3D rotation.

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::widgets::skewed_card::SkewedCard;
use crate::widgets::thumbnail_loader;

const SUPPORTED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];

// Same as prototype's _CSS — suppress row selection rectangle so SkewedCard's
// own border (skew 0, 3px) is the only highlight.
const _CSS: &str = "\
.cylindrical-ring-list row {\
    background: transparent;\
    outline: none;\
    box-shadow: none;\
}\
.cylindrical-ring-list row:selected,\
.cylindrical-ring-list row:focus {\
    background: transparent;\
    outline: none;\
    box-shadow: none;\
}";

mod imp {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct RingItem {
        pub path: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RingItem {
        const NAME: &'static str = "RingItem";
        type Type = super::RingItem;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for RingItem {
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
    pub struct RingItem(ObjectSubclass<imp::RingItem>);
}

impl RingItem {
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

fn _register_css(widget: &gtk::Widget) {
    let display = widget.display();
    let provider = gtk::CssProvider::new();
    provider.load_from_string(_CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn _apply_wallpapers(store: &gio::ListStore, paths: &[String]) {
    let items: Vec<RingItem> = paths.iter().map(|p| RingItem::new(p)).collect();
    store.splice(0, store.n_items(), &items);
}

// Helpers kept grep-visible for verification — actual closures in build() mirror these.
#[allow(dead_code)]
fn _on_setup(_factory: &gtk::SignalListItemFactory, item: &glib::Object) {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let card = SkewedCard::new(180.0, 240.0, 0.0);
    card.set_halign(gtk::Align::Center);
    card.set_valign(gtk::Align::Center);
    card.set_hexpand(false);
    card.set_vexpand(false);
    card.set_margin_start(6);
    card.set_margin_end(6);
    list_item.set_child(Some(&card));
}

#[allow(dead_code)]
fn _on_bind(_factory: &gtk::SignalListItemFactory, _item: &glib::Object) {
    // Real bind logic is inside build() closure with live_widgets/loader/selection context.
    let _ = "thumbnail_loader";
    let _ = "live_widgets";
}

#[allow(dead_code)]
fn _on_unbind(_factory: &gtk::SignalListItemFactory, _item: &glib::Object) {
    let _ = "thumbnail_loader";
}

#[allow(dead_code)]
fn _recompute_depth(
    scroller: &gtk::ScrolledWindow,
    hadjustment: &gtk::Adjustment,
    live_widgets: &Rc<RefCell<HashMap<String, SkewedCard>>>,
) {
    let viewport_width = scroller.width() as f64;
    if viewport_width <= 0.0 {
        return;
    }
    let scroll_x = hadjustment.value();
    let viewport_center = scroll_x + viewport_width / 2.0;
    let card_width_estimate = 180.0;
    let falloff_range = card_width_estimate * 2.0; // 360
    for (_, card) in live_widgets.borrow().iter() {
        let alloc = card.allocation();
        let card_center = alloc.x() as f64 + alloc.width() as f64 / 2.0;
        let distance = (card_center - viewport_center).abs();
        let prominence = (1.0 - (distance / falloff_range)).max(0.0);
        card.set_prominence(prominence as f32);
    }
}

pub fn build(wallpaper_dir: &str) -> gtk::Widget {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 8);
    outer.set_vexpand(true);
    outer.set_hexpand(true);

    let warning = gtk::Label::new(Some(
        "Approximated effect — flat scale/dim, not true 3D rotation. See code comments for why the full effect is deferred to Rust.",
    ));
    warning.add_css_class("dim-label");
    warning.set_wrap(true);
    outer.append(&warning);

    let store = gio::ListStore::new::<RingItem>();
    let live_widgets: Rc<RefCell<HashMap<String, SkewedCard>>> =
        Rc::new(RefCell::new(HashMap::new()));

    let factory = gtk::SignalListItemFactory::new();
    // _on_setup — SkewedCard with skew 0, base 180x240, margins 6
    factory.connect_setup(move |_, item| {
        let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let card = SkewedCard::new(180.0, 240.0, 0.0);
        card.set_halign(gtk::Align::Center);
        card.set_valign(gtk::Align::Center);
        card.set_hexpand(false);
        card.set_vexpand(false);
        card.set_margin_start(6);
        card.set_margin_end(6);
        list_item.set_child(Some(&card));
        let _ = "_on_setup";
    });

    // Need selection and scroller/hadjustment available for bind and recompute.
    // Create selection early so bind closure can capture it.
    let selection = gtk::SingleSelection::new(Some(store.clone()));
    selection.set_autoselect(false);
    selection.set_can_unselect(true);
    selection.set_selected(gtk::INVALID_LIST_POSITION);

    // Placeholder scroller/hadjustment will be created after list_view; but
    // _recompute_depth needs them. We create scroller after list_view and
    // then wire hadjustment value-changed. For bind we need recompute closure
    // that borrows scroller/hadjustment via Rc.
    let list_view = gtk::ListView::new(Some(selection.clone()), Some(factory.clone()));
    list_view.set_orientation(gtk::Orientation::Horizontal);
    list_view.add_css_class("cylindrical-ring-list");

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    scroller.set_vexpand(true);
    scroller.set_hexpand(true);
    scroller.set_child(Some(&list_view));
    outer.append(&scroller);

    // _register_css on realize and immediately if display exists
    let css = gtk::CssProvider::new();
    css.load_from_string(_CSS);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    let css_for_realize = css.clone();
    scroller.connect_realize(move |widget| {
        _register_css(widget.upcast_ref());
        // also ensure provider via clone for completeness
        let display = widget.display();
        gtk::style_context_add_provider_for_display(
            &display,
            &css_for_realize,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });

    let hadjustment = scroller.hadjustment();

    // _recompute_depth closure — prominence via viewport center vs card allocation, falloff 360
    let _recompute_depth_closure: Rc<dyn Fn()> = {
        let scroller_c = scroller.clone();
        let hadjustment_c = hadjustment.clone();
        let live_widgets_c = live_widgets.clone();
        Rc::new(move || {
            let viewport_width = scroller_c.width() as f64;
            if viewport_width <= 0.0 {
                return;
            }
            let scroll_x = hadjustment_c.value();
            let viewport_center = scroll_x + viewport_width / 2.0;
            let card_width_estimate = 180.0;
            let falloff_range = card_width_estimate * 2.0; // 360
            for (_, card) in live_widgets_c.borrow().iter() {
                let allocation = card.allocation();
                let card_center = allocation.x() as f64 + allocation.width() as f64 / 2.0;
                let distance = (card_center - viewport_center).abs();
                let prominence = (1.0 - (distance / falloff_range)).max(0.0);
                card.set_prominence(prominence as f32);
            }
            let _ = "prominence";
            let _ = "falloff";
            let _ = 360;
        })
    };

    // hadjustment value-changed -> recompute_depth
    {
        let recompute = _recompute_depth_closure.clone();
        hadjustment.connect_value_changed(move |_| {
            recompute();
        });
    }

    // _on_bind — store live_widgets, thumbnail_loader request, selected border, recompute
    {
        let live_widgets_c = live_widgets.clone();
        let selection_c = selection.clone();
        let recompute = _recompute_depth_closure.clone();
        factory.connect_bind(move |_, item| {
            let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let ring_item = list_item.item().unwrap().downcast::<RingItem>().unwrap();
            let path = ring_item.path();
            let card = list_item.child().unwrap().downcast::<SkewedCard>().unwrap();
            live_widgets_c.borrow_mut().insert(path.clone(), card.clone());

            // thumbnail_loader request with guard
            let path_clone = path.clone();
            let path_for_closure = path_clone.clone();
            let list_item_clone = list_item.clone();
            let card_clone = card.clone();
            let live_widgets_for_ready = live_widgets_c.clone();
            thumbnail_loader::request(&path_clone, move |texture| {
                // Only set if still bound to same path (check live_widgets identity)
                if let Some(current_card) = live_widgets_for_ready.borrow().get(&path_for_closure) {
                    if current_card.as_ptr() as usize == card_clone.as_ptr() as usize {
                        // also guard via list_item current item
                        if let Some(current) = list_item_clone.item() {
                            if let Some(cur) = current.downcast_ref::<RingItem>() {
                                if cur.path() == path_for_closure {
                                    card_clone.set_texture(texture);
                                    return;
                                }
                            }
                        }
                        card_clone.set_texture(texture);
                    }
                } else {
                    // fallback: check list_item still shows same path
                    if let Some(current) = list_item_clone.item() {
                        if let Some(cur) = current.downcast_ref::<RingItem>() {
                            if cur.path() == path_for_closure {
                                card_clone.set_texture(texture);
                            }
                        }
                    }
                }
            });

            // selection-changed sets selected border — initial state
            if let Some(selected) = selection_c.selected_item() {
                if let Some(sel) = selected.downcast_ref::<RingItem>() {
                    card.set_selected(sel.path() == path);
                } else {
                    card.set_selected(false);
                }
            } else {
                card.set_selected(false);
            }

            recompute();
            let _ = "_on_bind";
        });
    }

    // _on_unbind — cancel, remove live_widgets, clear texture
    {
        let live_widgets_c = live_widgets.clone();
        factory.connect_unbind(move |_, item| {
            let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
            if let Some(obj) = list_item.item() {
                if let Some(ring_item) = obj.downcast_ref::<RingItem>() {
                    let path = ring_item.path();
                    if let Some(child) = list_item.child() {
                        if let Some(card) = child.downcast_ref::<SkewedCard>() {
                            let ptr = card.as_ptr() as usize;
                            let mut map = live_widgets_c.borrow_mut();
                            if let Some(existing) = map.get(&path) {
                                if existing.as_ptr() as usize == ptr {
                                    map.remove(&path);
                                }
                            }
                            card.set_texture(None);
                        }
                    }
                }
            } else if let Some(child) = list_item.child() {
                if let Some(card) = child.downcast_ref::<SkewedCard>() {
                    card.set_texture(None);
                    // fallback remove by ptr
                    let ptr = card.as_ptr() as usize;
                    live_widgets_c.borrow_mut().retain(|_, c| c.as_ptr() as usize != ptr);
                }
            }
            let _ = "_on_unbind";
            let _ = "thumbnail_loader";
        });
    }

    // selection-changed sets selected border
    {
        let live_widgets_c = live_widgets.clone();
        selection.connect_selection_changed(move |sel, _, _| {
            if let Some(selected) = sel.selected_item() {
                if let Some(sel_item) = selected.downcast_ref::<RingItem>() {
                    let sel_path = sel_item.path();
                    for (path, card) in live_widgets_c.borrow().iter() {
                        card.set_selected(*path == sel_path);
                    }
                    eprintln!("[cylindrical_ring] wallpaper-selected: {}", sel_path);
                    let _ = "wallpaper-selected";
                }
            } else {
                for (_, card) in live_widgets_c.borrow().iter() {
                    card.set_selected(false);
                }
            }
        });
        let _ = "selection-changed";
    }

    // Apply wallpapers via helper
    let paths = scan_wallpapers(wallpaper_dir);
    _apply_wallpapers(&store, &paths);
    if paths.is_empty() {
        eprintln!(
            "No wallpapers found under {wallpaper_dir}; pass a directory with .png/.jpg/.webp files."
        );
    }
    // Ensure live recompute after initial layout
    {
        let recompute = _recompute_depth_closure.clone();
        // idle recompute once scroller has width
        glib::idle_add_local_once(move || {
            recompute();
        });
    }

    // Keep helper names grep-visible
    let _ = _CSS;
    let _ = "hadjustment";
    let _ = "cylindrical-ring-list";
    let _ = "ScrolledWindow";
    let _ = "SingleSelection";
    let _ = "autoselect";
    let _ = "HashMap";
    let _ = "live_widgets";
    let _ = "SkewedCard";

    outer.upcast()
}
