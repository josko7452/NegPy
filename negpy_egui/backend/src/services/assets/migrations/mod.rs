//! Startup migrations for persisted edits, rolls and asset stores.
//!
//! Mirrors `negpy/services/assets/migrations/`: every legacy row rewrite that depends on
//! the repository lives here, one module per migration group.

pub mod bounds;
pub mod cast_removal;
mod common;
pub mod hash;
pub mod normalization_roll;
pub mod positive_auto;
pub mod roll_fields;
pub mod source_color;
