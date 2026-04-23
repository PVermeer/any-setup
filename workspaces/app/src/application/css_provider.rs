use gtk::{CssProvider, gdk::Display, glib::object::IsA, style_context_add_provider_for_display};

pub fn init(display: &impl IsA<Display>) {
    let css_provider = CssProvider::new();

    css_provider.load_from_data(
        ".label-spaced {
            line-height: 2;
        }",
    );

    style_context_add_provider_for_display(
        display,
        &css_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
