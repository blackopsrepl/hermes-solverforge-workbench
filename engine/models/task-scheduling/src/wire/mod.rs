//! Records of the dataset format that are not planning entities or facts.

mod parameters;
mod records;

pub use parameters::Parameters;
pub use records::{
    Anchor, Assignment, Changeover, Dependency, DurationOn, ResourceKind, ResourceTransition, Span,
    TagDef, TagRef, TypeDef,
};
