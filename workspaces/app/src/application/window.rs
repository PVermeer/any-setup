mod view;

use crate::application::App;
use common::{
    config::{self},
    utils::OnceLockExt,
};
use gtk::{glib::Propagation, prelude::WidgetExt};
use libadwaita::{
    AlertDialog, ApplicationWindow, ResponseAppearance,
    gtk::prelude::GtkWindowExt,
    prelude::{AdwApplicationWindowExt, AdwDialogExt, AlertDialogExt},
};
use std::{cell::Cell, rc::Rc};
use view::View;

pub struct AppWindow {
    pub adw_window: ApplicationWindow,
    pub view: Rc<View>,
    pub confirmed_close: Rc<Cell<bool>>,
}
impl AppWindow {
    const DEFAULT_WIDTH: i32 = 950;
    const DEFAULT_HEIGHT: i32 = 850;
    const MIN_WIDTH: i32 = 600;
    const MIN_HEIGHT: i32 = 500;

    const RUNNING_TASKS_DIALOG_ID_CANCEL: &str = "cancel";
    const RUNNING_TASKS_DIALOG_ID_TASKS: &str = "tasks";
    const RUNNING_TASKS_DIALOG_ID_EXIT: &str = "exit";

    pub fn new(adw_application: &libadwaita::Application) -> Rc<Self> {
        let view = View::new();
        let window = ApplicationWindow::builder()
            .application(adw_application)
            .title(config::APP_NAME.get_value())
            .icon_name(config::APP_ID.get_value())
            .content(&view.nav_split)
            .build();

        Rc::new(Self {
            adw_window: window,
            view,
            confirmed_close: Rc::new(Cell::new(false)),
        })
    }

    pub fn init(self: &Rc<Self>, app: &Rc<App>) {
        self.set_cached_window_size(app);
        self.view.init(app);
        self.on_close(app);

        self.adw_window.add_breakpoint(self.view.breakpoint.clone());
        self.adw_window.present();
    }

    pub fn close(self: &Rc<Self>) {
        self.adw_window.close();
    }

    pub fn on_close(self: &Rc<Self>, app: &Rc<App>) {
        let self_clone = self.clone();
        let app_clone = app.clone();

        self.adw_window.connect_close_request(move |_window| {
            if !self_clone.confirmed_close.get()
                && let Some(running_tasks_dialog) = self_clone.get_running_tasks_dialog(&app_clone)
            {
                let self_clone_2 = self_clone.clone();
                let app_clone_2 = app_clone.clone();

                running_tasks_dialog.connect_response(None, move |_dialog, id| match id {
                    Self::RUNNING_TASKS_DIALOG_ID_EXIT => {
                        self_clone_2.confirmed_close.set(true);
                        self_clone_2.close();
                    }

                    Self::RUNNING_TASKS_DIALOG_ID_TASKS => {
                        app_clone_2.window.view.sidebar.load_task_page(&app_clone_2);
                    }

                    _ => {}
                });

                let self_clone = self_clone.clone();

                running_tasks_dialog.present(Some(&self_clone.adw_window));
                return Propagation::Stop;
            }

            self_clone.save_cached_window_size(&app_clone);
            Propagation::Proceed
        });
    }

    fn save_cached_window_size(self: &Rc<Self>, app: &Rc<App>) {
        let self_clone = self.clone();
        let app_clone = app.clone();

        let mut cache_settings_borrow = app_clone.cache_settings.borrow_mut();
        cache_settings_borrow.set_window_size(
            self_clone.adw_window.width(),
            self_clone.adw_window.height(),
            self_clone.adw_window.is_maximized(),
        );

        let _ = cache_settings_borrow.save();
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
    }

    fn get_running_tasks_dialog(&self, app: &Rc<App>) -> Option<AlertDialog> {
        if !app.task_manager.is_running() {
            return None;
        }

        let dialog = AlertDialog::builder()
            .heading(t!("window.close.dialog.title"))
            .body(t!("window.close.dialog.running_tasks"))
            .build();

        dialog.add_response(
            Self::RUNNING_TASKS_DIALOG_ID_CANCEL,
            &t!("window.close.dialog.cancel"),
        );
        dialog.add_response(
            Self::RUNNING_TASKS_DIALOG_ID_TASKS,
            &t!("window.close.dialog.tasks"),
        );
        dialog.add_response(
            Self::RUNNING_TASKS_DIALOG_ID_EXIT,
            &t!("window.close.dialog.exit"),
        );

        dialog.set_response_appearance(
            Self::RUNNING_TASKS_DIALOG_ID_TASKS,
            ResponseAppearance::Suggested,
        );
        dialog.set_response_appearance(
            Self::RUNNING_TASKS_DIALOG_ID_EXIT,
            ResponseAppearance::Destructive,
        );

        dialog.set_default_response(Some(Self::RUNNING_TASKS_DIALOG_ID_CANCEL));
        dialog.set_close_response(Self::RUNNING_TASKS_DIALOG_ID_TASKS);

        Some(dialog)
    }
}
