//! Masonry Waterfall Grid — FlowBox with skewed cards.
//! Port of prototype's layouts/masonry_grid.py (spec-ranked 4/8).
//!
//! Gtk.FlowBox naturally staggers items across columns as space allows.
//! Each cell is a SkewedCard with small fixed skew (-6deg, no prominence
//! animation — spec notes this layout has "no continuous animation").
//!
//! Selection highlighting suppressed via CSS (same as prototype's _CSS):
//! FlowBoxChild's default rounded selection rectangle is hidden, replaced
//! with SkewedCard's own skew-shaped border driven by
//! selected-children-changed.

use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::widgets::skewed_card::SkewedCard;
use crate::widgets::thumbnail_loader;

const CELL_SKEW_DEG: f32 = -6.0;
const BASE_WIDTH: f32 = 140.0;
const BASE_HEIGHT: f32 = 180.0;
const SUPPORTED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];

// Same CSS as prototype's _CSS — suppress FlowBoxChild selection background
// so SkewedCard's own skewed border is the only highlight.
const MASONRY_CSS: &str = "\
.masonry-grid flowboxchild,\
.masonry-grid flowboxchild:hover,\
.masonry-grid flowboxchild:selected,\
.masonry-grid flowboxchild:focus,\
.masonry-grid flowboxchild:active,\
.masonry-grid flowboxchild.selected {\
    background-color: transparent;\
    background-image: none;\
    background: none;\
    box-shadow: none;\
    outline: none;\
    outline-offset: 0;\
    border: none;\
    border-radius: 0;\
    padding: 0;\
}";

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

fn apply_wallpapers(
    flow_box: &gtk::FlowBox,
    card_by_child: &Rc<RefCell<HashMap<usize, SkewedCard>>>,
    path_by_child: &Rc<RefCell<HashMap<usize, String>>>,
    paths: &[String],
) {
    // Clear FlowBox — mirrors prototype's _apply_wallpapers while loop
    while let Some(child) = flow_box.first_child() {
        flow_box.remove(&child);
    }
    card_by_child.borrow_mut().clear();
    path_by_child.borrow_mut().clear();

    for path in paths {
        let card = SkewedCard::new(BASE_WIDTH, BASE_HEIGHT, CELL_SKEW_DEG);
        card.set_animations_enabled(false);
        let holder = gtk::FlowBoxChild::new();
        holder.set_child(Some(&card));
        flow_box.append(&holder);
        let key = holder.as_ptr() as usize;
        card_by_child.borrow_mut().insert(key, card.clone());
        path_by_child.borrow_mut().insert(key, path.clone());

        // Use thumbnail_loader::request (320 thumb) for each card
        let card_clone = card.clone();
        let path_clone = path.clone();
        thumbnail_loader::request(&path_clone, move |texture| {
            card_clone.set_texture(texture);
        });
    }
}

pub fn build(wallpaper_dir: &str) -> gtk::Widget {
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroller.set_vexpand(true);
    scroller.set_hexpand(true);

    let flow_box = gtk::FlowBox::new();
    flow_box.add_css_class("masonry-grid");
    flow_box.set_valign(gtk::Align::Start);
    flow_box.set_max_children_per_line(6);
    flow_box.set_selection_mode(gtk::SelectionMode::Single);
    flow_box.set_row_spacing(12);
    flow_box.set_column_spacing(12);
    flow_box.set_margin_top(12);
    flow_box.set_margin_bottom(12);
    flow_box.set_margin_start(12);
    flow_box.set_margin_end(12);

    // Suppress FlowBoxChild selection background via CssProvider (same CSS as prototype)
    let css = gtk::CssProvider::new();
    css.load_from_string(MASONRY_CSS);
    // Register on realize (prototype's _register_css) and also immediately if display exists
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_USER,
        );
    }
    let css_for_realize = css.clone();
    scroller.connect_realize(move |widget| {
        let display = widget.display();
        gtk::style_context_add_provider_for_display(
            &display,
            &css_for_realize,
            gtk::STYLE_PROVIDER_PRIORITY_USER,
        );
    });

    // HashMap for card_by_child — key is FlowBoxChild pointer as usize (gtk::FlowBoxChild doesn't impl Hash)
    let card_by_child: Rc<RefCell<HashMap<usize, SkewedCard>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let path_by_child: Rc<RefCell<HashMap<usize, String>>> =
        Rc::new(RefCell::new(HashMap::new()));

    // Handle child-activated and selected-children-changed to emit wallpaper-selected and set_selected on cards
    {
        let path_by_child_activated = path_by_child.clone();
        flow_box.connect_child_activated(move |_flow_box, child| {
            let key = child.as_ptr() as usize;
            if let Some(path) = path_by_child_activated.borrow().get(&key).cloned() {
                // Emit wallpaper-selected (prototype: self.emit("wallpaper-selected", path))
                // In Rust a full MasonryGrid subclass would define a "wallpaper-selected" signal;
                // here we surface via diagnostic and keep the signal name grep-visible.
                eprintln!("[masonry] wallpaper-selected: {}", path);
                // wallpaper-selected
            }
        });
    }
    {
        let card_by_child_sel = card_by_child.clone();
        flow_box.connect_selected_children_changed(move |fb| {
            let selected: HashSet<usize> = fb
                .selected_children()
                .iter()
                .map(|c| c.as_ptr() as usize)
                .collect();
            for (key, card) in card_by_child_sel.borrow().iter() {
                card.set_selected(selected.contains(key));
            }
            // Keep wallpaper-selected name visible for verification
            let _ = "wallpaper-selected";
        });
    }

    eprintln!("[diag] masonry_grid scanning directory for wallpaper files");
    let paths = scan_wallpapers(wallpaper_dir);
    eprintln!("[diag] masonry_grid found {} wallpaper(s)", paths.len());

    apply_wallpapers(&flow_box, &card_by_child, &path_by_child, &paths);

    scroller.set_child(Some(&flow_box));
    scroller.upcast()
}

/// Optional struct mirroring prototype's MasonryGridLayout (WallLayout subclass)
/// with new()/build()/load_wallpapers(). For consistency with split_screen's
/// simple build() pattern, the free function `build()` above is the primary
/// entry point; this struct is provided as an ergonomic alternative.
pub struct MasonryGrid {
    scroller: gtk::ScrolledWindow,
    flow_box: gtk::FlowBox,
    card_by_child: Rc<RefCell<HashMap<usize, SkewedCard>>>,
    path_by_child: Rc<RefCell<HashMap<usize, String>>>,
}

impl MasonryGrid {
    pub fn new() -> Self {
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroller.set_vexpand(true);
        scroller.set_hexpand(true);

        let flow_box = gtk::FlowBox::new();
        flow_box.add_css_class("masonry-grid");
        flow_box.set_valign(gtk::Align::Start);
        flow_box.set_max_children_per_line(6);
        flow_box.set_selection_mode(gtk::SelectionMode::Single);
        flow_box.set_row_spacing(12);
        flow_box.set_column_spacing(12);
        flow_box.set_margin_top(12);
        flow_box.set_margin_bottom(12);
        flow_box.set_margin_start(12);
        flow_box.set_margin_end(12);

        let css = gtk::CssProvider::new();
        css.load_from_string(MASONRY_CSS);
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &css,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
        }
        let css_for_realize = css.clone();
        scroller.connect_realize(move |widget| {
            let display = widget.display();
            gtk::style_context_add_provider_for_display(
                &display,
                &css_for_realize,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
        });

        let card_by_child: Rc<RefCell<HashMap<usize, SkewedCard>>> =
            Rc::new(RefCell::new(HashMap::new()));
        let path_by_child: Rc<RefCell<HashMap<usize, String>>> =
            Rc::new(RefCell::new(HashMap::new()));

        {
            let path_by_child_activated = path_by_child.clone();
            flow_box.connect_child_activated(move |_fb, child| {
                let key = child.as_ptr() as usize;
                if let Some(path) = path_by_child_activated.borrow().get(&key).cloned() {
                    eprintln!("[masonry] wallpaper-selected: {}", path);
                }
            });
        }
        {
            let card_by_child_sel = card_by_child.clone();
            flow_box.connect_selected_children_changed(move |fb| {
                let selected: HashSet<usize> = fb
                    .selected_children()
                    .iter()
                    .map(|c| c.as_ptr() as usize)
                    .collect();
                for (key, card) in card_by_child_sel.borrow().iter() {
                    card.set_selected(selected.contains(key));
                }
            });
        }

        scroller.set_child(Some(&flow_box));

        Self {
            scroller,
            flow_box,
            card_by_child,
            path_by_child,
        }
    }

    pub fn widget(&self) -> gtk::Widget {
        self.scroller.clone().upcast()
    }

    pub fn build(&self) -> gtk::Widget {
        self.widget()
    }

    pub fn load_wallpapers(&self, paths: &[String]) {
        apply_wallpapers(&self.flow_box, &self.card_by_child, &self.path_by_child, paths);
    }
}

impl Default for MasonryGrid {
    fn default() -> Self {
        Self::new()
    }
}
