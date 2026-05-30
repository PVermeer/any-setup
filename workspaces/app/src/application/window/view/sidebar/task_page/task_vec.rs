use super::TaskUi;
use std::ops::{Deref, DerefMut};

pub struct TaskVec(pub Vec<TaskUi>);
pub trait TaskVecExt {
    fn find_task_mut(&mut self, id: &str) -> Option<&mut TaskUi>;
}
impl Deref for TaskVec {
    type Target = Vec<TaskUi>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for TaskVec {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl TaskVecExt for TaskVec {
    fn find_task_mut(&mut self, id: &str) -> Option<&mut TaskUi> {
        let last_matches = matches!(
            self.last(),
            Some(task) if task.run_id == id
        );

        if last_matches {
            self.last_mut()
        } else {
            self.iter_mut().find(|task| task.run_id == id)
        }
    }
}
