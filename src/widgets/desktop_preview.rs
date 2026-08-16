//! DesktopPreview: shows the currently selected wallpaper, full-bleed,
//! cover-fit. Port of the Python prototype's widgets/desktop_preview.py
//! (the later, simplified version with the mock top bar/dock removed —
//! that was cut in the Python version after repeated complaints that it
//! read as a rendering artifact rather than helpful chrome).

use gtk::gdk;
use gtk::glib;
use gtk::graphene;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::RefCell;

use super::gsk_utils::draw_texture_cover;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct DesktopPreview {
        pub texture: RefCell<Option<gdk::Texture>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DesktopPreview {
        const NAME: &'static str = "DesktopPreview";
        type Type = super::DesktopPreview;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for DesktopPreview {}

    impl WidgetImpl for DesktopPreview {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            if orientation == gtk::Orientation::Horizontal {
                (320, 480, -1, -1)
            } else {
                (200, 300, -1, -1)
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let width = widget.width() as f32;
            let height = widget.height() as f32;
            if width <= 0.0 || height <= 0.0 {
                return;
            }

            if let Some(texture) = self.texture.borrow().as_ref() {
                draw_texture_cover(snapshot, texture, 0.0, 0.0, width, height);
            } else {
                let rect = graphene::Rect::new(0.0, 0.0, width, height);
                let placeholder = gdk::RGBA::new(0x1e as f32 / 255.0, 0x1e as f32 / 255.0, 0x1e as f32 / 255.0, 1.0);
                snapshot.append_color(&placeholder, &rect);
            }
        }
    }
}

glib::wrapper! {
    pub struct DesktopPreview(ObjectSubclass<imp::DesktopPreview>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl DesktopPreview {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn set_texture(&self, texture: Option<gdk::Texture>) {
        self.imp().texture.replace(texture);
        self.queue_draw();
    }
}

impl Default for DesktopPreview {
    fn default() -> Self {
        Self::new()
    }
}
