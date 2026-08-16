//! Layout: Split-Screen Preview — port of the Python prototype's
//! layouts/split_screen.py (the later Gtk::Box version, not the
//! original Gtk::Paned — Paned's fixed-pixel divider position left dead
//! space whenever the window was a different size than one specific
//! calibration point; a Box where the preview vexpands automatically
//! claims all leftover space with no such gap).
//!
//! This first pass is NOT virtualized — every wallpaper in the folder
//! gets its own SkewedCard immediately, in a plain horizontal Gtk::Box
//! inside a ScrolledWindow. That's fine for a folder of dozens of
//! images; it's the same class of scaling limit the Python prototype's
//! Radial Fan-Out layout had (flagged there as a deeper architectural
//! issue, not something this pass fixes either) — virtualizing this
//! properly is follow-up work, not blocking a first working version.

use gtk::gdk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

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

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    scroller.set_size_request(-1, 190);

    let card_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    card_row.set_margin_top(12);
    card_row.set_margin_bottom(12);
    card_row.set_margin_start(12);
    card_row.set_margin_end(12);
    scroller.set_child(Some(&card_row));
    outer.append(&scroller);

    let paths = scan_wallpapers(wallpaper_dir);
    if paths.is_empty() {
        eprintln!(
            "No wallpapers found under {wallpaper_dir}; pass a directory with .png/.jpg/.webp files."
        );
    }

    // Track the currently-selected card so clicking a new one can
    // deselect the old — Rc<RefCell<>> since the click closures below
    // need shared mutable access to this across multiple cards.
    let selected_card: Rc<RefCell<Option<SkewedCard>>> = Rc::new(RefCell::new(None));

    for path in &paths {
        let card = SkewedCard::new(CARD_WIDTH, CARD_HEIGHT, -12.0);
        card.set_halign(gtk::Align::Center);
        card.set_valign(gtk::Align::Center);
        card_row.append(&card);

        thumbnail_loader::request(path, {
            let card = card.clone();
            move |texture| card.set_texture(texture)
        });

        let click = gtk::GestureClick::new();
        {
            let preview = preview.clone();
            let card = card.clone();
            let selected_card = selected_card.clone();
            let path = path.clone();
            click.connect_released(move |_, _, _, _| {
                if let Some(old) = selected_card.borrow_mut().replace(card.clone()) {
                    if old != card {
                        old.set_selected(false);
                    }
                }
                card.set_selected(true);

                // Reuse the thumbnail texture for the preview rather
                // than a separate full-res load — good enough for this
                // first pass; a real full-resolution load for the
                // preview specifically is follow-up work.
                if let Ok(pixbuf) = gtk::gdk_pixbuf::Pixbuf::from_file(Path::new(&path)) {
                    preview.set_texture(Some(gdk::Texture::for_pixbuf(&pixbuf)));
                }
            });
        }
        card.add_controller(click);
    }

    // Select the first wallpaper by default, matching the Python
    // version's initial-state behavior.
    if let (Some(first_path), Some(first_card)) = (paths.first(), card_row.first_child()) {
        if let Some(first_card) = first_card.downcast_ref::<SkewedCard>() {
            first_card.set_selected(true);
            *selected_card.borrow_mut() = Some(first_card.clone());
        }
        if let Ok(pixbuf) = gtk::gdk_pixbuf::Pixbuf::from_file(Path::new(first_path)) {
            preview.set_texture(Some(gdk::Texture::for_pixbuf(&pixbuf)));
        }
    }

    outer.upcast()
}
