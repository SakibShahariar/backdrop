//! Split-Screen with virtualized ListView — only visible cards are realized.
//! Fixes slow launch (416× widgets + 416× thumbs → 10s) and low-res preview
//! (480px thumb upscaled to 1000px preview). Now thumb 320px, preview 1920px,
//! and ListView virtualizes (8 visible, not 416).

use gtk::gio;
use gtk::prelude::*;

use crate::widgets::desktop_preview::DesktopPreview;
use crate::widgets::skewed_card::SkewedCard;
use crate::widgets::thumbnail_loader;

const CARD_WIDTH: f32 = 130.0;
const CARD_HEIGHT: f32 = CARD_WIDTH * 1.3;
const SUPPORTED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];

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

pub fn build(wallpaper_dir: &str) -> gtk::Widget {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.set_vexpand(true);
    outer.set_hexpand(true);

    let preview = DesktopPreview::new();
    preview.set_vexpand(true);
    preview.set_hexpand(true);
    outer.append(&preview);

    eprintln!("[diag] scanning directory for wallpaper files");
    let paths = scan_wallpapers(wallpaper_dir);
    eprintln!("[diag] found {} wallpaper(s)", paths.len());
    if paths.is_empty() {
        eprintln!(
            "No wallpapers found under {wallpaper_dir}; pass a directory with .png/.jpg/.webp files."
        );
        let label = gtk::Label::new(Some("No wallpapers found"));
        outer.append(&label);
        return outer.upcast();
    }

    // Model for virtualized ListView
    let store = gio::ListStore::new::<gtk::StringObject>();
    for p in &paths {
        store.append(&gtk::StringObject::new(p));
    }
    let selection = gtk::SingleSelection::new(Some(store));
    selection.set_can_unselect(true);
    selection.set_autoselect(false);

    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, item| {
        let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let card = SkewedCard::new(CARD_WIDTH, CARD_HEIGHT, -12.0);
        card.set_halign(gtk::Align::Center);
        card.set_valign(gtk::Align::Center);
        list_item.set_child(Some(&card));
    });
    factory.connect_bind(move |_, item| {
        let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let string_obj = list_item.item().unwrap().downcast::<gtk::StringObject>().unwrap();
        let path = string_obj.string().to_string();
        let card = list_item.child().unwrap().downcast::<SkewedCard>().unwrap();
        // Reset previous texture (recycled widget)
        card.set_texture(None);
        card.set_selected(list_item.is_selected());
        let path_clone = path.clone();
        let list_item_clone = list_item.clone();
        thumbnail_loader::request(&path, {
            let card = card.clone();
            move |texture| {
                // Only set if still bound to same path (check current item)
                if let Some(current) = list_item_clone.item() {
                    if let Some(cur_obj) = current.downcast_ref::<gtk::StringObject>() {
                        if cur_obj.string() == path_clone {
                            card.set_texture(texture);
                        }
                    }
                }
            }
        });
    });
    factory.connect_unbind(move |_, item| {
        let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
        if let Some(child) = list_item.child() {
            if let Some(card) = child.downcast_ref::<SkewedCard>() {
                card.set_texture(None);
            }
        }
    });

    let list_view = gtk::ListView::new(Some(selection.clone()), Some(factory));
    list_view.set_orientation(gtk::Orientation::Horizontal);
    list_view.set_single_click_activate(true);
    list_view.add_css_class("wallpaper-list");

    // Handle selection → preview (high-res) + card selected border
    {
        let preview_sel = preview.clone();
        selection.connect_selection_changed(move |sel, _, _| {
            if let Some(selected) = sel.selected_item() {
                if let Some(obj) = selected.downcast_ref::<gtk::StringObject>() {
                    let path = obj.string().to_string();
                    let preview = preview_sel.clone();
                    thumbnail_loader::request_preview(&path, move |texture| {
                        preview.set_texture(texture)
                    });
                }
            }
        });
        // Also handle activate (click)
        let preview_act = preview.clone();
        list_view.connect_activate(move |lv, pos| {
            if let Some(item) = lv.model().unwrap().item(pos) {
                if let Some(obj) = item.downcast_ref::<gtk::StringObject>() {
                    let path = obj.string().to_string();
                    let preview = preview_act.clone();
                    thumbnail_loader::request_preview(&path, move |texture| {
                        preview.set_texture(texture)
                    });
                }
            }
        });
    }

    // Initial preview: first wallpaper high-res
    if let Some(first) = paths.first() {
        let preview_c = preview.clone();
        let first = first.clone();
        thumbnail_loader::request_preview(&first, move |texture| {
            preview_c.set_texture(texture);
        });
        // Select first item
        selection.set_selected(0);
    }

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    scroller.set_size_request(-1, 190);
    scroller.set_child(Some(&list_view));
    outer.append(&scroller);

    // Also keep SingleSelection selected border in sync via factory is_selected,
    // but need to handle that ListView recycles — bind already sets selected.

    eprintln!(
        "[diag] split_screen::build() virtualized {} wallpapers (ListView), returning",
        paths.len()
    );
    outer.upcast()
}
