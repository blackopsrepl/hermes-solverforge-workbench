//! Facts derived from a dataset by `prepare`: ids resolved to indexes, dates
//! to day numbers. Shared read-only by every fact and entity of one plan.

use std::fmt;
use std::sync::Arc;

use super::{Derived, TaskInfo};

/// Handle on the plan's derived facts.
#[derive(Clone, Default)]
pub struct Ctx(pub Arc<Derived>);

impl Ctx {
    pub fn task(&self, index: usize) -> &TaskInfo {
        &self.0.tasks[index]
    }
}

impl std::ops::Deref for Ctx {
    type Target = Derived;
    fn deref(&self) -> &Derived {
        &self.0
    }
}

/// Two handles are equal when they share the same derived facts.
impl PartialEq for Ctx {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Ctx {}

impl std::hash::Hash for Ctx {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::ptr::hash(Arc::as_ptr(&self.0), state);
    }
}

impl fmt::Debug for Ctx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Ctx")
    }
}
