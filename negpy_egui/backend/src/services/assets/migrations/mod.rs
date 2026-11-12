//! Startup migrations for persisted edits, rolls and asset stores.
//!
//! Mirrors `negpy/services/assets/migrations/`: every legacy row rewrite that depends on
//! the repository lives here, one module per migration group.

pub mod bounds;
pub mod cast_removal;
mod common;
pub mod hash;
pub mod source_color;
