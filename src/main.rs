use adw::prelude::*;
use gtk::glib;

mod layouts;
mod widgets;

fn main() -> glib::ExitCode {
    // Application ID uses reverse-DNS convention — org.example is a
    // placeholder; swap in your actual domain or io.github.<username>
    // once you know where this will live permanently.
    let app = adw::Application::builder()
        .application_id("org.example.Backdrop")
        .build();

    app.connect_activate(|app| {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Backdrop")
            .default_width(1000)
            .default_height(600)
            .build();

        let wallpaper_dir = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/usr/share/backgrounds".to_string());

        let content = layouts::split_screen::build(&wallpaper_dir);
        window.set_content(Some(&content));
        window.present();
    });

    // run_with_args(&[]) instead of run() — Adw::Application::run()
    // hands the process's real argv to GIO's own command-line handling
    // by default, which tries to interpret our wallpaper-directory
    // argument as a "file to open" (a GIO Application feature we never
    // opted into), producing a GLib-GIO-CRITICAL warning. We already
    // read the directory ourselves via std::env::args() above, so GIO's
    // own arg parsing isn't needed at all — passing an empty slice here
    // stops it from ever seeing the real argv.
    app.run_with_args::<&str>(&[])
}
