use gtk::{CssProvider, gdk::Display, glib::object::IsA, style_context_add_provider_for_display};

pub fn init(display: &impl IsA<Display>) {
    let css_provider = CssProvider::new();

    css_provider.load_from_data(
        ".label-spaced {
            line-height: 2;
        }
        
        .accent-bg {
            background-color: var(--accent-bg-color);
            color: var(--accent-fg-color);
        }

        .accent-outline {
            box-shadow: inset 0 0 0 2px var(--accent-color);
        }
        ",
    );

    style_context_add_provider_for_display(
        display,
        &css_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
