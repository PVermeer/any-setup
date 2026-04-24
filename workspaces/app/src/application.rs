mod css_provider;
mod error_dialog;
mod pages;
mod window;

use anyhow::{Error, Result};
use common::{
    app_dirs::AppDirs,
    assets::{self},
    cache_settings::CacheSettings,
    config::{self},
    utils::OnceLockExt,
};
use error_dialog::ErrorDialog;
use gtk::{IconTheme, Image, Settings, gdk};
use pages::{Pages, page_config::Page};
use std::{cell::RefCell, rc::Rc};
use tracing::{debug, error};
use window::AppWindow;

pub struct App {
    pub cache_settings: RefCell<CacheSettings>,
    pub dirs: Rc<AppDirs>,
    pub error_dialog: ErrorDialog,
    icon_theme: Rc<IconTheme>,
    window: AppWindow,
    pages: Pages,
}
impl App {
    pub fn new(adw_application: &libadwaita::Application) -> Rc<Self> {
        Rc::new({
            let display = gdk::Display::default().expect("Failed to connect to display");
            let icon_theme = Rc::new(IconTheme::for_display(&display));
            let app_dirs = AppDirs::new().expect("Failed to get all needed directories");
            let settings = Settings::default().expect("Failed to load gtk settings");
            let cache_settings = RefCell::new(
                CacheSettings::new(&app_dirs).expect("Failed to load cached settings"),
            );
            let window = AppWindow::new(adw_application);
            let pages = Pages::new(&app_dirs);
            let error_dialog = ErrorDialog::new();

            Self::set_theme_settings(&settings);
            css_provider::init(&display);

            Self {
                cache_settings,
                dirs: app_dirs,
                error_dialog,
                icon_theme,
                window,
                pages,
            }
        })
    }

    pub fn init(self: &Rc<Self>) {
        if let Err(error) = (|| -> Result<()> {
            debug!("Using icon theme: {}", self.icon_theme.theme_name());

            // Order matters!
            self.window.init(self);
            self.error_dialog.init(self);

            assets::init(&self.dirs)?;

            // Last
            self.pages.init(self);
            if let Some(page) = self.pages.get_first() {
                self.navigate(page);
            }

            Ok(())
        })() {
            self.show_error(&error);
        }
    }

    #[allow(clippy::unused_self, unused)]
    pub fn get_icon(self: &Rc<Self>) -> Image {
        Image::from_icon_name(config::APP_ID.get_value())
    }

    pub fn navigate(self: &Rc<Self>, page: &Page) {
        self.window.view.navigate(self, page);
    }

    pub fn show_error(self: &Rc<Self>, error: &Error) {
        error!("{error:?}");
        self.error_dialog.show(self, error);
    }

    pub fn close(self: &Rc<Self>) {
        self.window.close();
    }

    fn set_theme_settings(settings: &Settings) {
        settings.set_gtk_icon_theme_name(Some("Adwaita"));
    }
}
