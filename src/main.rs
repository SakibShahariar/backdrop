use adw::prelude::*;
use gtk::gdk;
use gtk::glib;

mod layouts;
mod widgets;

fn parse_args() -> (String, String) {
    let args: Vec<String> = std::env::args().collect();
    let mut wallpaper_dir = "/usr/share/backgrounds".to_string();
    let mut layout_id = "split_screen".to_string();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--layout" && i + 1 < args.len() {
            layout_id = args[i + 1].clone();
            i += 2;
        } else if !args[i].starts_with("--") && wallpaper_dir == "/usr/share/backgrounds" {
            wallpaper_dir = args[i].clone();
            i += 1;
        } else {
            i += 1;
        }
    }
    // Validate layout_id
    let valid = [
        "split_screen",
        "stacked_deck",
        "infinite_ribbon",
        "masonry_grid",
        "glassmorphism",
        "parallax_gallery",
        "radial_fan",
        "cylindrical_ring",
    ];
    if !valid.contains(&layout_id.as_str()) {
        eprintln!("[warn] unknown layout '{layout_id}', falling back to split_screen");
        layout_id = "split_screen".to_string();
    }
    (wallpaper_dir, layout_id)
}

fn layout_window_size(layout_id: &str) -> (i32, i32) {
    match layout_id {
        "split_screen" => (1000, 600),
        "stacked_deck" => (680, 540),
        "infinite_ribbon" => (1000, 420),
        "masonry_grid" => (1000, 700),
        "glassmorphism" => (1000, 500),
        "parallax_gallery" => (1000, 500),
        "radial_fan" => (1000, 600),
        "cylindrical_ring" => (1000, 500),
        _ => (1000, 600),
    }
}

fn build_layout(layout_id: &str, wallpaper_dir: &str) -> gtk::Widget {
    match layout_id {
        "split_screen" => layouts::split_screen::build(wallpaper_dir),
        "stacked_deck" => layouts::stacked_deck::build(wallpaper_dir),
        "infinite_ribbon" => layouts::infinite_ribbon::build(wallpaper_dir),
        "masonry_grid" => layouts::masonry_grid::build(wallpaper_dir),
        "glassmorphism" => layouts::glassmorphism::build(wallpaper_dir),
        "parallax_gallery" => layouts::parallax_gallery::build(wallpaper_dir),
        "radial_fan" => layouts::radial_fan::build(wallpaper_dir),
        "cylindrical_ring" => layouts::cylindrical_ring::build(wallpaper_dir),
        _ => layouts::split_screen::build(wallpaper_dir),
    }
}

fn main() -> glib::ExitCode {
    eprintln!("[diag] main() started");

    let (wallpaper_dir, initial_layout) = parse_args();
    let (win_w, win_h) = layout_window_size(&initial_layout);
    eprintln!("[diag] wallpaper_dir={wallpaper_dir} layout={initial_layout} size={win_w}x{win_h}");

    let app = adw::Application::builder()
        .application_id("org.example.Backdrop")
        .build();

    eprintln!("[diag] application built, connecting activate handler");

    app.connect_activate(move |app| {
        eprintln!("[diag] activate signal fired");

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Backdrop")
            .default_width(win_w)
            .default_height(win_h)
            .build();

        eprintln!("[diag] window built");

        // Stack of all 8 layouts, built lazily on first switch (like prototype's window.py)
        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_vexpand(true);
        stack.set_hexpand(true);

        // Build initial layout now, others lazily on switch
        let initial_widget = build_layout(&initial_layout, &wallpaper_dir);
        stack.add_named(&initial_widget, Some(&initial_layout));
        stack.set_visible_child_name(&initial_layout);

        // Cache for lazily-built layouts
        let stack_clone = stack.clone();
        let wallpaper_dir_clone = wallpaper_dir.clone();
        // For now, build all remaining layouts eagerly (simple, like prototype's _switch_to builds on demand)
        // To keep startup fast, we build them on idle after present()
        let initial_layout_clone = initial_layout.clone();
        glib::idle_add_local_once(move || {
            for layout_id in [
                "split_screen",
                "stacked_deck",
                "infinite_ribbon",
                "masonry_grid",
                "glassmorphism",
                "parallax_gallery",
                "radial_fan",
                "cylindrical_ring",
            ] {
                if layout_id == initial_layout_clone {
                    continue;
                }
                let widget = build_layout(layout_id, &wallpaper_dir_clone);
                stack_clone.add_named(&widget, Some(layout_id));
            }
            eprintln!("[diag] all layouts built (lazy)");
        });

        // Simple layout switcher via number keys 1-8 (since no header bar popover yet)
        // 1=split_screen, 2=stacked_deck, etc., matches prototype's --layout ids
        let stack_for_keys = stack.clone();
        let key_layout = gtk::EventControllerKey::new();
        key_layout.set_propagation_phase(gtk::PropagationPhase::Capture);
        key_layout.connect_key_pressed(move |_, key, _, _| {
            let layout_id = match key {
                gdk::Key::_1 => Some("split_screen"),
                gdk::Key::_2 => Some("stacked_deck"),
                gdk::Key::_3 => Some("infinite_ribbon"),
                gdk::Key::_4 => Some("masonry_grid"),
                gdk::Key::_5 => Some("glassmorphism"),
                gdk::Key::_6 => Some("parallax_gallery"),
                gdk::Key::_7 => Some("radial_fan"),
                gdk::Key::_8 => Some("cylindrical_ring"),
                _ => None,
            };
            if let Some(id) = layout_id {
                stack_for_keys.set_visible_child_name(id);
                eprintln!("[diag] switched to layout {id} via key");
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(key_layout);

        window.set_content(Some(&stack));

        // ESC to close (from prototype window.py: Escape closes window, no header bar)
        let key_ctl = gtk::EventControllerKey::new();
        key_ctl.set_propagation_phase(gtk::PropagationPhase::Capture);
        let win_clone = window.clone();
        key_ctl.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                win_clone.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(key_ctl);

        eprintln!("[diag] calling window.present()");
        window.present();
        eprintln!("[diag] window.present() returned");
    });

    eprintln!("[diag] calling app.run()");
    let program_name = std::env::args().next().unwrap_or_default();
    let exit_code = app.run_with_args(&[program_name]);
    eprintln!("[diag] app.run_with_args() returned: {exit_code:?}");
    exit_code
}
