mod view;

use crate::application::App;
use common::{
    config::{self},
    utils::OnceLockExt,
};
use gtk::{glib::Propagation, prelude::WidgetExt};
use libadwaita::{ApplicationWindow, gtk::prelude::GtkWindowExt, prelude::AdwApplicationWindowExt};
use std::rc::Rc;
use view::View;

pub struct AppWindow {
    pub adw_window: ApplicationWindow,
    pub view: Rc<View>,
}
impl AppWindow {
    const DEFAULT_WIDTH: i32 = 950;
    const DEFAULT_HEIGHT: i32 = 850;
    const MIN_WIDTH: i32 = 600;
    const MIN_HEIGHT: i32 = 500;

    pub fn new(adw_application: &libadwaita::Application) -> Self {
        let view = View::new();
        let window = ApplicationWindow::builder()
            .application(adw_application)
            .title(config::APP_NAME.get_value())
            .icon_name(config::APP_ID.get_value())
            .content(&view.nav_split)
            .build();

        Self {
            adw_window: window,
            view,
        }
    }

    pub fn init(&self, app: &Rc<App>) {
        self.set_cached_window_size(app);
        self.view.init(app);

        self.adw_window.add_breakpoint(self.view.breakpoint.clone());
        self.adw_window.present();
    }

    pub fn close(&self) {
        self.adw_window.close();
    }

    fn set_cached_window_size(&self, app: &Rc<App>) {
        let window_settings = &app.cache_settings.borrow().settings.window;

        let width = if window_settings.width == 0 {
            Self::DEFAULT_WIDTH
        } else if window_settings.width < Self::MIN_WIDTH {
            Self::MIN_WIDTH
        } else {
            window_settings.width
        };

        let height = if window_settings.height == 0 {
            Self::DEFAULT_HEIGHT
        } else if window_settings.height < Self::MIN_HEIGHT {
            Self::MIN_HEIGHT
        } else {
            window_settings.height
        };

        let is_maximized = window_settings.maximized;

        self.adw_window.set_default_width(width);
        self.adw_window.set_default_height(height);
        self.adw_window.set_maximized(is_maximized);

        let app_clone = app.clone();

        self.adw_window.connect_close_request(move |window| {
            let mut cache_settings_borrow = app_clone.cache_settings.borrow_mut();
            cache_settings_borrow.set_window_size(
                window.width(),
                window.height(),
                window.is_maximized(),
            );

            let _ = cache_settings_borrow.save();

            Propagation::Proceed
        });
    }
}
