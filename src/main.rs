use adw::prelude::*;
use gtk::glib;

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

        // Temporary: exercise SkewedCard's public API end-to-end to
        // confirm the widget actually constructs and behaves, not just
        // type-checks in isolation. Explicit halign/valign=Center is
        // required here — without it, a widget set as a window's sole
        // content stretches to fill the entire window (using its
        // allocated size, not its natural/measured size), which is
        // exactly what produced a plain full-window gray fill instead
        // of a small, visibly skewed card.
        let card = widgets::skewed_card::SkewedCard::new(200.0, 260.0, -12.0);
        card.set_halign(gtk::Align::Center);
        card.set_valign(gtk::Align::Center);
        card.set_prominence(0.5);
        card.set_selected(true);
        window.set_content(Some(&card));
        window.present();
    });

    app.run()
}
