use adw::prelude::*;
use gtk::gdk;
use gtk::glib;

mod layouts;
mod widgets;

fn main() -> glib::ExitCode {
    eprintln!("[diag] main() started");

    // Application ID uses reverse-DNS convention — org.example is a
    // placeholder; swap in your actual domain or io.github.<username>
    // once you know where this will live permanently.
    let app = adw::Application::builder()
        .application_id("org.example.Backdrop")
        .build();

    eprintln!("[diag] application built, connecting activate handler");

    app.connect_activate(|app| {
        eprintln!("[diag] activate signal fired");

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Backdrop")
            .default_width(1000)
            .default_height(600)
            .build();

        eprintln!("[diag] window built");

        let wallpaper_dir = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/usr/share/backgrounds".to_string());

        eprintln!("[diag] scanning wallpaper dir: {wallpaper_dir}");

        let content = layouts::split_screen::build(&wallpaper_dir);
        window.set_content(Some(&content));

        // ESC to close
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
    // Passing only argv[0] (program name), not the full real argv —
    // avoids GIO's file-open argument interpretation (which produced
    // the GLib-GIO-CRITICAL warning) while still being a well-formed,
    // non-empty args list, unlike the fully-empty array I tried before
    // (which may itself have been malformed and silently broken
    // activation — untested, hence the diagnostics above to find out
    // for certain rather than guess again).
    let program_name = std::env::args().next().unwrap_or_default();
    let exit_code = app.run_with_args(&[program_name]);
    eprintln!("[diag] app.run_with_args() returned: {exit_code:?}");
    exit_code
}
