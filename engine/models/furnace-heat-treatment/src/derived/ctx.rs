//! Handles entities keep on shared derived data.

use std::fmt;
use std::sync::Arc;

use super::Derived;

/// Handle on the plan's derived facts.
#[derive(Clone, Default)]
pub struct Ctx(pub Arc<Derived>);

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

/// A planning variable's candidate values, shared between clones of a plan.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct Values(pub Arc<[usize]>);

impl Values {
    pub fn new(values: Vec<usize>) -> Self {
        Self(values.into())
    }
}

impl std::ops::Deref for Values {
    type Target = [usize];
    fn deref(&self) -> &[usize] {
        &self.0
    }
}

impl fmt::Debug for Values {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} values", self.0.len())
    }
}
