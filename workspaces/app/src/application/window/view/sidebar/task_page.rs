use super::NavPage;
use crate::application::{App, pages::NavPageBuild};
use libadwaita::NavigationPage;
use std::rc::Rc;

pub struct TaskPage {
    pub nav_page: NavigationPage,
}
impl NavPage for TaskPage {
    fn get_navpage(&self) -> &NavigationPage {
        &self.nav_page
    }

    fn get_section(&self) -> Option<&str> {
        None
    }

    fn get_icon(&self) -> Option<&str> {
        None
    }
}
impl TaskPage {
    pub fn new() -> Self {
        let NavPageBuild { nav_page, .. } = Self::build_nav_page(&t!("pages.tasks.title"));

        Self { nav_page }
    }

    pub fn init(&self, app: &Rc<App>) {}
}
