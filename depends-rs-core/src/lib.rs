#![forbid(unsafe_code)]
//! Pure syntax transformations for the depends-rs procedural macros.
//!
//! Expansion state travels in tokens. This crate does not read source files,
//! resolve Rust names, inspect function bodies, or keep a global registry.

mod elaborate;
mod expand;
mod protocol;
mod syntax;

pub use expand::{continue_expansion, expand_depends, expand_lifetimes};
pub use syntax::{DependencyPath, GenericSlot, LifetimeShape, LifetimeSlot, OpaqueSlot, Relation};
