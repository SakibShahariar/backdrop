use adw::prelude::*;
use gtk::glib;

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

        let label = gtk::Label::new(Some("Rust scaffold — pipeline check"));
        window.set_content(Some(&label));
        window.present();
    });

    app.run()
}
