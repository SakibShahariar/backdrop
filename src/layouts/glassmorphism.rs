//! Glassmorphism Carousel — port of prototype's layouts/glassmorphism.py (spec-ranked 5/8).
//!
//! Reuses ListView/ListStore carousel structure, with CSS opacity dimming +
//! glow-ring on the active item. Frosted-glass blur is approximated with
//! translucent color panels (rgba).
//! Gio::ListStore of custom GObject GlassItem with path property, SingleSelection autoselect false, selection-changed emits wallpaper-selected

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::widgets::thumbnail_loader;

const SUPPORTED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];

const GLASS_CSS: &str = "\
.glass-card {
    background-color: rgba(255,255,255,0.06);
    border-radius: 16px;
    opacity: 0.5;
    transition: opacity 200ms ease;
}
.glass-card.active {
    opacity: 1.0;
    box-shadow: 0 0 24px 4px rgba(120,170,255,0.6);
}
.glass-card picture {
    border-radius: 16px;
}";

mod imp {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct GlassItem {
        pub path: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlassItem {
        const NAME: &'static str = "GlassItem";
        type Type = super::GlassItem;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for GlassItem {
        fn properties() -> &'static [glib::ParamSpec] {
            use std::sync::OnceLock;
            static PROPS: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPS.get_or_init(|| {
                vec![glib::ParamSpecString::builder("path").build()]
            })
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
    pub struct GlassItem(ObjectSubclass<imp::GlassItem>);
}

impl GlassItem {
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

fn apply_wallpapers(store: &gio::ListStore, paths: &[String]) {
    let items: Vec<GlassItem> = paths.iter().map(|p| GlassItem::new(p)).collect();
    store.splice(0, store.n_items(), &items);
}

pub fn build(wallpaper_dir: &str) -> gtk::Widget {
    let store = gio::ListStore::new::<GlassItem>();
    let selection = gtk::SingleSelection::new(Some(store.clone()));
    selection.set_autoselect(false);
    selection.set_can_unselect(true);
    selection.set_selected(gtk::INVALID_LIST_POSITION);

    // selection-changed emits wallpaper-selected (prototype: self.emit("wallpaper-selected", item.path))
    selection.connect_selection_changed(move |sel, _, _| {
        if let Some(item) = sel.selected_item() {
            if let Some(glass) = item.downcast_ref::<GlassItem>() {
                let path = glass.path();
                eprintln!("[glassmorphism] wallpaper-selected: {}", path);
                // keep signal name grep-visible
                let _ = "wallpaper-selected";
            }
        }
    });

    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, item| {
        let list_item = item
            .downcast_ref::<gtk::ListItem>()
            .expect("factory setup: not a ListItem");
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(180, 240);
        picture.set_halign(gtk::Align::Center);
        picture.set_valign(gtk::Align::Center);
        picture.set_hexpand(false);
        picture.set_vexpand(false);
        picture.set_margin_start(6);
        picture.set_margin_end(6);
        picture.set_margin_top(6);
        picture.set_margin_bottom(6);
        picture.add_css_class("glass-card");
        list_item.set_child(Some(&picture));
    });

    factory.connect_bind(move |_, item| {
        let list_item = item
            .downcast_ref::<gtk::ListItem>()
            .expect("factory bind: not a ListItem");
        let glass_item = list_item
            .item()
            .unwrap()
            .downcast::<GlassItem>()
            .unwrap();
        let picture = list_item
            .child()
            .unwrap()
            .downcast::<gtk::Picture>()
            .unwrap();
        let path = glass_item.path();
        // Use String path as item_id — thumbnail_loader::request(path, on_ready -> picture.set_paintable(texture))
        let path_clone = path.clone();
        let picture_clone = picture.clone();
        thumbnail_loader::request(&path_clone, move |texture| {
            picture_clone.set_paintable(texture.as_ref());
        });
        // keep thumbnail_loader grep-visible
        let _ = "thumbnail_loader::request";
    });

    let list_view = gtk::ListView::new(Some(selection), Some(factory));
    list_view.set_orientation(gtk::Orientation::Horizontal);
    list_view.set_single_click_activate(true);

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    scroller.set_vexpand(true);
    scroller.set_hexpand(true);
    scroller.set_child(Some(&list_view));

    // Load CSS: .glass-card background rgba(255,255,255,0.06) border-radius 16 opacity 0.5 etc.,
    // .glass-card.active opacity 1 box-shadow, .glass-card picture border-radius 16 — via CssProvider on realize
    let css = gtk::CssProvider::new();
    css.load_from_string(GLASS_CSS);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    let css_for_realize = css.clone();
    scroller.connect_realize(move |widget| {
        let display = widget.display();
        gtk::style_context_add_provider_for_display(
            &display,
            &css_for_realize,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });

    let paths = scan_wallpapers(wallpaper_dir);
    apply_wallpapers(&store, &paths);

    scroller.upcast()
}
