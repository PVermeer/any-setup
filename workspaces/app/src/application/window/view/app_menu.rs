use crate::application::{App, window::view::View};
use common::{
    config::{self},
    utils::OnceLockExt,
};
use libadwaita::{
    gio::{ActionEntry, Menu, MenuItem, SimpleActionGroup, prelude::ActionMapExtManual},
    gtk::{MenuButton, prelude::WidgetExt},
};
use std::rc::Rc;

pub struct AppMenu {
    pub button: MenuButton,
    menu: Menu,
    actions: SimpleActionGroup,
}
impl AppMenu {
    pub const NAME: &str = "app-menu";
    pub const ACTION_LABEL: &str = "app-menu";

    pub fn new() -> Self {
        // GTK does not let a popovermenu to be created programmatically
        // https://blog.libove.org/posts/rust-gtk--creating-a-menu-bar-programmatically-with-gtk-rs
        let button = MenuButton::builder()
            .name(AppMenu::NAME)
            .icon_name("open-menu-symbolic")
            .build();
        let menu = Menu::new();
        button.set_menu_model(Some(&menu));

        // Must use actions, there is currently no way to register a fn on click or something
        let actions = SimpleActionGroup::new();

        Self {
            button,
            menu,
            actions,
        }
    }

    pub fn init(&self, app: &Rc<App>) {
        app.window
            .adw_window
            .insert_action_group(Self::ACTION_LABEL, Some(&self.actions));

        let section_1 = Menu::new();
        let section_2 = Menu::new();

        let about = self.build_about(app);

        section_2.append_item(&about);

        self.menu.append_section(None, &section_1);
        self.menu.append_section(None, &section_2);
    }

    fn build_about(&self, app: &Rc<App>) -> MenuItem {
        let app_clone = app.clone();
        self.build_menu_item(
            &t!(
                "app_menu.about.title",
                app_name = config::APP_NAME.get_value()
            ),
            ("about", move || {
                View::show_about(&app_clone);
            }),
        )
    }

    fn build_menu_item(
        &self,
        label: &str,
        (action_name, action): (&str, impl Fn() + 'static),
    ) -> MenuItem {
        let item = MenuItem::new(
            Some(label),
            Some(&(Self::ACTION_LABEL.to_owned() + "." + action_name)),
        );
        let action = ActionEntry::builder(action_name)
            .activate(move |_: &SimpleActionGroup, _, _| {
                action();
            })
            .build();
        self.actions.add_action_entries([action]);

        item
    }
}
